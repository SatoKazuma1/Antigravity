#!/bin/sh
# Antigravity Unlocker — установка в один клик (Linux x86-64):
#
#   curl -fsSL https://raw.githubusercontent.com/SatoKazuma1/Antigravity/main/install.sh | sh
#
# Скачивает последний релиз с GitHub, устанавливает в ~/.local/share/agunlocker/,
# создаёт ярлык в меню приложений (~/.local/share/applications/ag_unlocker.desktop),
# добавляет команду в ~/.local/bin/ag_unlocker и автоматически запускает GUI (или TUI, если нет монитора).
set -eu

REPO="SatoKazuma1/Antigravity"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
DIR="$DATA_DIR/agunlocker"
BIN="$DIR/ag_unlocker"
LOCAL_BIN="${HOME}/.local/bin"
DESKTOP_DIR="$DATA_DIR/applications"

case "$(uname -m)" in
    x86_64 | amd64) ;;
    *)
        echo "Antigravity Unlocker собран только для x86-64, а здесь $(uname -m)." >&2
        exit 1
        ;;
esac

TAG=""
if URL=$(curl -fsSLo /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest"); then
    TAG="${URL##*/}"
fi
VER="${TAG#v}"

case "$VER" in
    "" | *[!0-9._]*)
        if [ -x "$BIN" ]; then
            echo "GitHub не ответил — используется уже скачанная версия." >&2
        else
            echo "Не удалось узнать последнюю версию на github.com/$REPO." >&2
            exit 1
        fi
        ;;
    *)
        if [ ! -x "$BIN" ] || [ "$(cat "$BIN.version" 2>/dev/null)" != "$VER" ]; then
            echo "Скачиваю Antigravity Unlocker $VER…" >&2
            mkdir -p "$DIR"
            TMP=$(mktemp -d)
            trap 'rm -rf "$TMP"' EXIT
            curl -fsSL "https://github.com/$REPO/releases/download/$TAG/antigravity-unlocker-linux-x86_64.tar.gz" |
                tar xz -C "$TMP"
            mv -f "$TMP/antigravity-unlocker-linux-x86_64/ag_unlocker" "$BIN"
            chmod +x "$BIN"
            echo "$VER" >"$BIN.version"
            rm -rf "$TMP"
            trap - EXIT
        fi
        ;;
esac

# 1. Создание симлинка в ~/.local/bin/ag_unlocker
mkdir -p "$LOCAL_BIN"
ln -sf "$BIN" "$LOCAL_BIN/ag_unlocker"

# 2. Создание ярлыка в меню приложений (desktop launcher)
mkdir -p "$DESKTOP_DIR"
cat <<EOF >"$DESKTOP_DIR/ag_unlocker.desktop"
[Desktop Entry]
Name=Antigravity Unlocker
Comment=Antigravity Bypass and Configuration Tool
Exec=$BIN
Terminal=false
Type=Application
Categories=Network;Utility;Development;
StartupNotify=true
EOF
chmod +x "$DESKTOP_DIR/ag_unlocker.desktop"

echo "Установка завершена!"
echo "• Бинарник: $BIN"
echo "• Команда терминала: ag_unlocker (в $LOCAL_BIN)"
echo "• Ярлык создан в меню приложений"

# 3. Автоматический запуск: если доступен графический дисплей — GUI, иначе — TUI
if [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
    echo "Запускаю графический интерфейс (GUI)…"
    "$BIN" >/dev/null 2>&1 &
else
    echo "Графический дисплей не обнаружен, запускаю в терминале (TUI)…"
    exec "$BIN" --tui </dev/tty
fi
