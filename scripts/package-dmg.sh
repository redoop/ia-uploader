#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

sh scripts/package-macos.sh
app='Internet Archive Uploader.app'
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "dist/$app/Contents/Info.plist")
architecture=$(uname -m)
output="$(pwd)/dist/Internet-Archive-Uploader-${version}-macOS-${architecture}.dmg"
work=$(mktemp -d "${TMPDIR:-/tmp}/ia-uploader-dmg.XXXXXX")
staging="$work/payload"
mkdir "$staging" "$work/mount"
mounted=0
cleanup() {
    if [ "$mounted" = 1 ]; then hdiutil detach "$work/mount" >/dev/null 2>&1 || true; fi
    rm -rf "$work"
}
trap cleanup EXIT HUP INT TERM

ditto "dist/$app" "$staging/$app"
# Local ad-hoc signature; distribution notarization requires an Apple developer identity.
codesign --force --deep --sign - "$staging/$app"
codesign --verify --deep --strict "$staging/$app"
ln -s /Applications "$staging/Applications"
mkdir "$staging/Command Line Tools"
cp dist/ia-upload-cli "$staging/Command Line Tools/ia-upload-cli"
cat > "$staging/安装说明.txt" <<'TEXT'
Internet Archive 上传器

安装桌面应用：
将 Internet Archive Uploader.app 拖入 Applications 文件夹。
安装后，在“应用程序”文件夹打开 Internet Archive 上传器。
使用完成后，可以推出此安装磁盘。

账号验证：
前往 https://archive.org/account/s3.php 获取 Internet Archive S3 凭据。
在软件中填写 Access Key、Secret Key，保持“自动代理”，点击“验证账号”。
密钥不保存在磁盘。上传内容通常公开可读。

Command Line Tools：
“Command Line Tools”文件夹内的 ia-upload-cli 可复制到你选择的位置。
运行 ia-upload-cli --help 查看命令；使用 IA_ACCESS_KEY、IA_SECRET_KEY 环境变量提供凭据。

此安装包是本机架构版本，使用本地临时签名，尚未进行 Apple 开发者签名及公证。
TEXT

cat > "$staging/Installation.txt" <<'TEXT'
Internet Archive Uploader

Drag Internet Archive Uploader.app into Applications, then open the installed app.
The interface automatically follows the operating system's preferred language:
Chinese for zh locales (Simplified Chinese text), English for all other locales.
Restart the app after changing the OS language.

Get Internet Archive S3 credentials at https://archive.org/account/s3.php.
Enter the Access Key and Secret Key and click Verify account.
Keys are not saved to disk. Uploads are generally publicly readable.

Command Line Tools contains ia-upload-cli. Run it with --help.
Set IA_ACCESS_KEY and IA_SECRET_KEY to provide credentials.
Use --lang auto|zh|en or IA_UPLOADER_LANG to override the language.

This is a host-architecture build with an ad-hoc signature, without Apple notarization.
TEXT

# Embed the mounted volume icon inside the filesystem so it travels with the image.
hdiutil create -ov -volname 'IA Uploader' -srcfolder "$staging" -fs HFS+ -format UDRW "$work/writable.dmg"
hdiutil attach -nobrowse -mountpoint "$work/mount" "$work/writable.dmg"
mounted=1
cp assets/AppIcon.icns "$work/mount/.VolumeIcon.icns"
xcrun SetFile -a C "$work/mount"
hdiutil detach "$work/mount"
mounted=0
hdiutil convert "$work/writable.dmg" -format UDZO -ov -o "$output"
# Also set the DMG file's Finder icon (macOS resource fork + custom-icon flag).
python3 - "$work/icon.r" <<'PYICON'
import sys
from pathlib import Path
raw = Path('assets/AppIcon.icns').read_bytes()
with open(sys.argv[1], 'w') as resource:
    resource.write("data 'icns' (-16455) {\n")
    for offset in range(0, len(raw), 32):
        resource.write('    $"' + raw[offset:offset + 32].hex() + '"\n')
    resource.write('};\n')
PYICON
xcrun Rez -append "$work/icon.r" -o "$output"
xcrun SetFile -a C "$output"
xcrun DeRez -only icns "$output" > "$work/icon-check.r"
test -s "$work/icon-check.r"
hdiutil verify "$output"
(cd "$(dirname "$output")" && shasum -a 256 "$(basename "$output")") > "$output.sha256"
printf '%s\n' "已生成：$output"
