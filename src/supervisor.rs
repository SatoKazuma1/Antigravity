//! The resident half of the program on Linux: the tray icon, and nothing else.
//!
//! The window is a process of its own (`gui::CHILD_FLAG`), started from here
//! when the user asks for it and gone when they close it. That is the point of
//! the split. A window that is only *hidden* keeps its whole renderer — the
//! Vulkan/GL context, the swap chain, the font atlas — which came to a couple of
//! hundred MB of video memory for a program that sits in the tray all day. A
//! process that exits gives all of it back, and it never has to answer a
//! session manager's "close" with a refusal either: at logout the window is
//! simply closed, and this process has no window to be asked about.
//!
//! What stays out of here on purpose: the relay and the auto-patch watcher are
//! services of their own (`background`), the update check lives in the window,
//! and no renderer is ever created in this process.
//!
//! One instance per login session. It is held by an advisory lock on a file in
//! the runtime directory (released by the kernel when the process dies, so a
//! crash leaves nothing stale), and a second start finds the first through a
//! Unix socket next to it and asks it to open the window.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

use crate::gui;
use crate::ops::{self, Cmd};
use crate::update;

/// What `run` came to.
pub enum Outcome {
    /// Finished: the resident process ran and was quit, or another one was
    /// already running and has been told what to do.
    Done,
    /// Could not be the resident process here (no tray, no runtime directory).
    /// The caller falls back to the single-process window, which is what every
    /// start did before the split.
    Unavailable(String),
}

enum Msg {
    Open,
    Enable,
    Flush,
    Quit,
    /// The window process with this pid has ended.
    GuiExited(u32),
}

struct Gui {
    pid: u32,
    /// The pipe the window listens on (`gui::watch_parent`).
    stdin: ChildStdin,
}

fn runtime_dir() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            let dir = PathBuf::from(format!("/tmp/ag_unlocker-{}", unsafe { libc::getuid() }));
            let _ = std::fs::create_dir_all(&dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
            }
            dir
        }
    }
}

fn lock_path() -> PathBuf {
    runtime_dir().join("ag_unlocker.lock")
}

fn socket_path() -> PathBuf {
    runtime_dir().join("ag_unlocker.sock")
}

/// `Ok(Some(file))` — ours, keep it open for as long as the process lives.
/// `Ok(None)` — somebody else's.
fn acquire() -> std::io::Result<Option<File>> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path())?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

/// Asks the running resident process to open the window. It may have taken the
/// lock a moment ago and not bound its socket yet, hence the retries.
fn ask_to_open() -> bool {
    for _ in 0..30 {
        if let Ok(mut s) = UnixStream::connect(socket_path()) {
            return s.write_all(b"open\n").is_ok();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Runs the resident process, or hands the start to the one that already is.
///
/// `open_window` is false for the autostart entry (`--minimized`): the icon, and
/// no window until it is asked for.
pub fn run(open_window: bool) -> Outcome {
    let lock = match acquire() {
        Ok(Some(f)) => f,
        Ok(None) => {
            if open_window && !ask_to_open() {
                return Outcome::Unavailable(
                    "уже запущенная копия не отвечает — окно откроется отдельно".into(),
                );
            }
            return Outcome::Done;
        }
        Err(e) => return Outcome::Unavailable(format!("не удалось занять блокировку: {e}")),
    };

    let Some(handler) = gui::tray::create_tray() else {
        return Outcome::Unavailable("системный трей не создался".into());
    };

    // Ours by the lock, so whatever is at that path is a corpse.
    let sock = socket_path();
    let _ = std::fs::remove_file(&sock);
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => return Outcome::Unavailable(format!("не удалось открыть сокет: {e}")),
    };

    let (tx, rx) = channel::<Msg>();

    // The tray reports from its own GTK thread, through process-wide handlers.
    {
        let t = tx.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                let _ = t.send(Msg::Open);
            }
        }));
        let t = tx.clone();
        let (open_id, enable_id, flush_id, quit_id) = (
            handler.open_id,
            handler.enable_id,
            handler.flush_id,
            handler.quit_id,
        );
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
            let msg = if event.id == open_id {
                Msg::Open
            } else if event.id == enable_id {
                Msg::Enable
            } else if event.id == flush_id {
                Msg::Flush
            } else if event.id == quit_id {
                Msg::Quit
            } else {
                return;
            };
            let _ = t.send(msg);
        }));
    }

    // A second start of the program, asking for the window.
    {
        let t = tx.clone();
        let _ = std::thread::Builder::new().name("ipc".into()).spawn(move || {
            for conn in listener.incoming() {
                let Ok(conn) = conn else { continue };
                // A client that connects and says nothing must not stall the
                // next one.
                let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
                let mut line = String::new();
                if BufReader::new(conn).read_line(&mut line).is_ok() && line.trim() == "open" {
                    let _ = t.send(Msg::Open);
                }
            }
        });
    }

    if open_window {
        let _ = tx.send(Msg::Open);
    }

    let mut window: Option<Gui> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Open => open(&mut window, &tx),
            Msg::GuiExited(pid) => {
                if window.as_ref().is_some_and(|g| g.pid == pid) {
                    window = None;
                }
            }
            Msg::Enable => enable_all(),
            Msg::Flush => flush_dns(),
            Msg::Quit => break,
        }
    }

    if let Some(g) = window.take() {
        // Default disposition: it ends on the spot, and the pipe closing would
        // end it anyway.
        unsafe { libc::kill(g.pid as libc::pid_t, libc::SIGTERM) };
    }
    let _ = std::fs::remove_file(&sock);
    drop(lock);
    Outcome::Done
}

/// Brings up the window: the one that is already open, or a new process.
fn open(window: &mut Option<Gui>, tx: &Sender<Msg>) {
    if let Some(g) = window.as_mut() {
        // A write to a dead window fails (SIGPIPE is ignored by the runtime),
        // which is how a window that has just closed is told from one that has
        // not.
        if g.stdin.write_all(b"focus\n").and_then(|()| g.stdin.flush()).is_ok() {
            return;
        }
        *window = None;
    }

    let exe = match update::current_executable_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ag_unlocker: {e}");
            return;
        }
    };
    let mut child = match Command::new(exe)
        .arg(gui::CHILD_FLAG)
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ag_unlocker: не удалось открыть окно: {e}");
            return;
        }
    };
    let pid = child.id();
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return;
    };
    *window = Some(Gui { pid, stdin });

    // The only owner of the `Child`: it has to be waited on, or the window
    // process stays a zombie after it closes.
    let t = tx.clone();
    let _ = std::thread::Builder::new().name("window-wait".into()).spawn(move || {
        let _ = child.wait();
        let _ = t.send(Msg::GuiExited(pid));
    });
}

/// «Включить всё» from the menu, with no window to show the journal in.
///
/// A worker of its own for the length of the job: the same one the window
/// drives, started the way the window starts it (unlocked, then the command),
/// told to stop when the queue is empty, and its events read and dropped.
fn enable_all() {
    let (ev_tx, ev_rx) = channel();
    let worker = ops::spawn(ev_tx, Box::new(|| {}));
    worker.send(Cmd::Unlocked);
    worker.send(Cmd::EnableAll);
    worker.send(Cmd::Stop);
    let _ = std::thread::Builder::new()
        .name("enable-all-events".into())
        .spawn(move || for _ in ev_rx {});
}

fn flush_dns() {
    let _ = std::thread::Builder::new().name("flush-dns".into()).spawn(|| {
        crate::dns::flush_client_cache();
        crate::dns::refresh_pinned_hosts();
    });
}
