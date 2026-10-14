#!/usr/bin/env bash
set -euo pipefail
app_dir="$(cd -- "$(dirname -- "$0")" && pwd)"

session_runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
if [[ -d "$session_runtime" && -O "$session_runtime" ]]; then
    export XDG_RUNTIME_DIR="$session_runtime"
    if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" && -S "$session_runtime/bus" && -O "$session_runtime/bus" ]]; then
        export DBUS_SESSION_BUS_ADDRESS="unix:path=$session_runtime/bus"
    fi
fi

if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${WAYLAND_SOCKET:-}" && -z "${DISPLAY:-}" ]]; then
    wayland_sockets=()
    if [[ -d "$session_runtime" && -O "$session_runtime" ]]; then
        for candidate in "$session_runtime"/wayland-*; do
            if [[ -S "$candidate" && -O "$candidate" ]]; then
                wayland_sockets+=("${candidate##*/}")
            fi
        done
    fi
    case "${#wayland_sockets[@]}" in
        1) export WAYLAND_DISPLAY="${wayland_sockets[0]}" ;;
        0)
            echo "No desktop display connection found. Run this launcher from a desktop terminal, or set WAYLAND_DISPLAY (Wayland) / DISPLAY (X11) for your session." >&2
            exit 1
            ;;
        *)
            echo "Multiple Wayland sessions found. Set WAYLAND_DISPLAY to the session you want, then run this launcher again." >&2
            exit 1
            ;;
    esac
fi
exec "$app_dir/stem-desktop" "$@"
