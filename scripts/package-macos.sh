#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked --bins
sh scripts/build-icon.sh
version=$(awk -F '"' '/^version = / {print $2; exit}' Cargo.toml)
app='dist/Internet Archive Uploader.app'
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp assets/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
cp target/release/ia-uploader "$app/Contents/MacOS/ia-uploader"
cp target/release/ia-upload-cli dist/ia-upload-cli
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Internet Archive Uploader</string>
<key>CFBundleDisplayName</key><string>Internet Archive 上传器</string>
<key>CFBundleIdentifier</key><string>org.local.ia-uploader</string>
<key>CFBundleExecutable</key><string>ia-uploader</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>CFBundleIconFile</key><string>AppIcon.icns</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
printf '%s\n' "已生成 $app 和 dist/ia-upload-cli"
