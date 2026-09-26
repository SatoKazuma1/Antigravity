//! Cross-platform autostart management.
//!
//! On Linux: uses the FreeDesktop.org (XDG) Autostart specification:
//! `$XDG_CONFIG_HOME/autostart/ag_unlocker.desktop` (default `~/.config/autostart/ag_unlocker.desktop`).
//! This is completely agnostic of the init system (works on systemd, OpenRC, runit, dinit, s6, sysvinit)
//! and is supported by all desktop environments (GNOME, KDE Plasma, XFCE, Sway, Hyprland, Niri, i3, etc.).
//!
//! On Windows: uses the standard per-user registry run key:
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

use std::path::PathBuf;

#[cfg(not(target_os = "windows"))]
pub fn autostart_desktop_path() -> PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    config.join("autostart").join("ag_unlocker.desktop")
}

#[cfg(not(target_os = "windows"))]
pub fn is_enabled() -> bool {
    autostart_desktop_path().exists()
}

#[cfg(not(target_os = "windows"))]
pub fn set_enabled(on: bool) -> Result<(), String> {
    use std::fs;
    let path = autostart_desktop_path();

    if !on {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("не удалось удалить автозапуск: {}", e))?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("не удалось создать папку autostart: {}", e))?;
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let installed_bin = PathBuf::from(&home).join(".local/share/agunlocker/ag_unlocker");
    let alt_bin = PathBuf::from(&home).join(".local/bin/ag_unlocker");

    let exe = if installed_bin.exists() {
        installed_bin
    } else if alt_bin.exists() {
        alt_bin
    } else {
        std::env::current_exe().map_err(|e| format!("путь к exe не определён: {}", e))?
    };

    let icon_path = PathBuf::from(&home).join(".local/share/ag-unlocker/icon.png");
    let icon_line = if icon_path.exists() {
        format!("Icon={}\n", icon_path.display())
    } else {
        String::new()
    };

    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=Antigravity Unlocker\n\
         Comment=Antigravity Bypass and Configuration Tool\n\
         Exec=\"{}\" --minimized\n\
         {}Terminal=false\n\
         Categories=Network;Utility;\n\
         X-GNOME-Autostart-enabled=true\n",
        exe.display(),
        icon_line
    );

    fs::write(&path, content).map_err(|e| format!("не удалось записать файл автозапуска: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_enabled() -> bool {
    use std::process::Command;
    let mut cmd = Command::new("reg");
    cmd.args([
        "query",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "AntigravityUnlocker",
    ]);
    crate::utils::bounded_output(
        crate::utils::no_window(&mut cmd),
        std::time::Duration::from_secs(5),
    )
    .is_some_and(|o| o.status.success())
}

#[cfg(target_os = "windows")]
pub fn set_enabled(on: bool) -> Result<(), String> {
    use std::process::Command;
    if !on {
        let mut cmd = Command::new("reg");
        cmd.args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "AntigravityUnlocker",
            "/f",
        ]);
        let _ = crate::utils::bounded_output(
            crate::utils::no_window(&mut cmd),
            std::time::Duration::from_secs(5),
        );
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| format!("путь к exe не определён: {}", e))?;
    let val = format!("\"{}\" --minimized", exe.display());

    let mut cmd = Command::new("reg");
    cmd.args([
        "add",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "AntigravityUnlocker",
        "/t",
        "REG_SZ",
        "/d",
        &val,
        "/f",
    ]);

    let out = crate::utils::bounded_output(
        crate::utils::no_window(&mut cmd),
        std::time::Duration::from_secs(5),
    )
    .ok_or_else(|| "Превышен таймаут добавления автозапуска в реестр".to_string())?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_autostart_toggle_linux() {
        let prev = is_enabled();
        let path = autostart_desktop_path();

        assert!(set_enabled(true).is_ok());
        assert!(path.exists());
        assert!(is_enabled());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[Desktop Entry]"));
        assert!(content.contains("--minimized"));

        assert!(set_enabled(false).is_ok());
        assert!(!path.exists());
        assert!(!is_enabled());

        if prev {
            let _ = set_enabled(true);
        }
    }
}
