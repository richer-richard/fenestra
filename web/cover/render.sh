#!/usr/bin/env sh
# Renders the project cover to gallery/cover.png.
#
# Two passes through headless Chrome: light.html becomes light.png, and
# cover.html paints that image through the letters of the word over the
# dark render. Run from anywhere; output paths are relative to this file.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
out="$here/../../gallery/cover.png"

if [ -n "${CHROME:-}" ]; then
  chrome="$CHROME"
elif [ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]; then
  chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
elif command -v google-chrome >/dev/null 2>&1; then
  chrome=google-chrome
elif command -v chromium >/dev/null 2>&1; then
  chrome=chromium
else
  echo "render.sh: no Chrome found; set CHROME=/path/to/chrome" >&2
  exit 1
fi

shoot() {
  "$chrome" --headless=new --disable-gpu --hide-scrollbars \
    --force-device-scale-factor=1 --window-size=2400,1200 \
    --virtual-time-budget=8000 --screenshot="$2" "file://$1" >/dev/null 2>&1
}

mkdir -p "$(dirname "$out")"
shoot "$here/light.html" "$here/light.png"
shoot "$here/cover.html" "$out"
echo "wrote $(cd "$(dirname "$out")" && pwd)/cover.png"
