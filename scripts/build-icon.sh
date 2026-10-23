#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
icon_work=$(mktemp -d "${TMPDIR:-/tmp}/ia-uploader-icon.XXXXXX")
trap 'rm -rf "$icon_work"' EXIT HUP INT TERM
iconset="$icon_work/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/logo-v1.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    retina=$((size * 2))
    sips -z "$retina" "$retina" assets/logo-v1.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o assets/AppIcon.icns
