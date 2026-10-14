#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
    exec target/debug/examples/smoke
fi
xvfb_bin="${STEM_XVFB:-/tmp/stem-gui/usr/bin/Xvfb}"
"$xvfb_bin" :99 -screen 0 1280x1100x24 -nolisten tcp > /tmp/stem-xvfb.log 2>&1 &
display_pid=$!
trap 'kill "$display_pid" 2>/dev/null || true' EXIT
sleep 1
DISPLAY=:99 LIBGL_ALWAYS_SOFTWARE=1 target/debug/examples/smoke
