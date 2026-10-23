# Internet Archive Uploader

<p align="center"><img src="assets/logo-v1.png" width="160" alt="Internet Archive Uploader logo"></p>

English · [简体中文](README.zh-CN.md)

**Upload large files to Internet Archive from a desktop app or your terminal.**

Internet Archive Uploader is an independent, MIT-licensed Rust application with a desktop interface and a command-line tool. Both use the same upload engine to authenticate with Internet Archive S3 credentials, stream files without loading them entirely into memory, and retry interrupted requests.

Use the desktop app to select files and track progress, or use the CLI for scripts and repeatable upload workflows.

[Download v0.1.1 for Apple Silicon Mac](https://github.com/redoop/ia-uploader/releases/download/v0.1.1/Internet-Archive-Uploader-0.1.1-macOS-arm64.dmg) · [All releases](https://github.com/redoop/ia-uploader/releases)

## Features

- **Desktop and CLI:** File selection, drag-and-drop, an upload queue, progress reporting, transfer rates, and scriptable commands.
- **Streaming uploads:** File sizes use 64-bit values; memory usage does not grow with the full file size.
- **Account verification:** Authenticate with an Internet Archive S3 Access Key and Secret Key before uploading.
- **Retries and redirects:** Exponential backoff for network failures and HTTP 429/5xx responses; replayable uploads for HTTP 307/308 redirects.
- **Proxy support:** Automatic system/environment proxy discovery, direct connections, or an explicit HTTP, HTTPS, or SOCKS5 proxy.
- **Unicode support:** Unicode filenames and titles, including Chinese text.
- **macOS installer:** A DMG containing the desktop app, an Applications shortcut, and the CLI, with matching application and mounted-volume icons.

Credentials are kept in memory for the running session and are not saved by the application. Authentication is sent only to HTTPS `archive.org` endpoints and their subdomains, including after redirects. TLS certificate verification remains enabled.

## Install on macOS

1. Download the [Apple Silicon DMG](https://github.com/redoop/ia-uploader/releases/download/v0.1.1/Internet-Archive-Uploader-0.1.1-macOS-arm64.dmg).
2. Open it and drag **Internet Archive Uploader.app** into **Applications**.
3. Launch the app and enter your Internet Archive S3 credentials.

The current release is for **Apple Silicon Macs (arm64)**. The application has an ad-hoc signature and has not been signed with an Apple Developer identity or notarized. Builds and runtime checks have been performed on macOS; Windows and Linux have not been validated. The current desktop interface and CLI messages are in Chinese; this README documents them in English.

The DMG also includes `ia-upload-cli`, which you can copy to a directory of your choice.

### Verify the download

Download the `.dmg` and `.sha256` files from the [release page](https://github.com/redoop/ia-uploader/releases/tag/v0.1.1) into the same directory, then run:

```sh
shasum -a 256 -c Internet-Archive-Uploader-0.1.1-macOS-arm64.dmg.sha256
```

## Desktop workflow

1. Leave the connection mode on **自动代理** (automatic proxy), or choose **直连** (direct) or **手动代理** (manual proxy).
2. Enter your Access Key and Secret Key, then click **验证账号** (verify account).
3. Enter an item identifier and optionally set the title, collection, and media type.
4. Click **添加文件** (add files), or drag files into the window.
5. Click **开始上传** (start upload) and monitor the current file's progress, transfer rate, and completed-file count.

Network requests run in a background thread. **取消任务** (cancel task) requests cancellation during file reads or retry waits; a request already connecting or waiting for a server response may take longer to stop. Closing the window terminates the task.

## Build from source

Requires Rust 1.92 or newer.

```sh
git clone git@github.com:redoop/ia-uploader.git
cd ia-uploader
cargo build --release --locked
```

This produces two executables:

| Executable | Purpose |
| --- | --- |
| `target/release/ia-uploader` | Desktop app |
| `target/release/ia-upload-cli` | Command-line uploader |

To build only the CLI without GUI dependencies:

```sh
cargo build --release --locked --no-default-features --bin ia-upload-cli
```

Run the desktop app during development with `cargo run --bin ia-uploader`. Chinese font rendering uses system fonts: STHeiti on macOS, Microsoft YaHei on Windows, or Noto Sans CJK at the configured system path on Linux.

## CLI quick start

Get your **Internet Archive** S3 Access Key and Secret Key from the [account credentials page](https://archive.org/account/s3.php). These are different from your website password and AWS credentials.

In macOS zsh, use hidden input to avoid putting the key values in shell history:

```zsh
read -rs 'IA_ACCESS_KEY?Access Key: '; echo
read -rs 'IA_SECRET_KEY?Secret Key: '; echo
export IA_ACCESS_KEY IA_SECRET_KEY
./target/release/ia-upload-cli auth
```

Upload a file:

```sh
./target/release/ia-upload-cli upload my-archive-20261024 \
  /absolute/path/backup.7z \
  --title 'My Archive' --collection opensource --mediatype data
```

Upload multiple files to the same item:

```sh
./target/release/ia-upload-cli upload my-video-20261024 \
  /absolute/path/part1.mp4 /absolute/path/part2.mp4 \
  --title 'Video Collection' --collection opensource_movies --mediatype movies \
  --retries 5 --no-derive
```

Files are uploaded sequentially using their base filenames. Duplicate filenames in a queue are rejected. An identifier selects the item at `https://archive.org/details/<identifier>`; choose an unused identifier to create an item, or an item you have permission to modify. The collection must accept uploads from your account. Metadata headers apply when creating an item; they do not update an existing item's metadata.

Existing remote files with the same name are replaced. Requests include `x-archive-keep-old-version:1` to ask Internet Archive to preserve older versions.

### Proxy options

Automatic mode uses system proxy settings and proxy environment variables. You can override it with a global CLI option before or after the subcommand:

```sh
./target/release/ia-upload-cli auth --proxy http://127.0.0.1:7890
./target/release/ia-upload-cli upload my-archive-20261024 /path/file.zip \
  --proxy socks5h://127.0.0.1:7890
./target/release/ia-upload-cli auth --direct
```

Use the address configured in your proxy application. `--proxy` and `--direct` are mutually exclusive. These options do not modify system settings. A connection timeout means the client did not receive an authentication result; it does not establish that the credentials are invalid.

## Upload behavior and limitations

- **No multipart or byte-level resume:** Each file uses a single PUT request. A retry starts that file from the beginning; rerunning the command retransmits the listed files.
- **No fixed client-side file-size ceiling:** Actual upload limits depend on Internet Archive, your permissions, and available server resources.
- **Timeouts:** Connections time out after 30 seconds; authentication requests after 60 seconds. Upload requests have no total deadline. A stalled request may require cancellation or restarting the application.
- **Retry policy:** Three additional attempts by default, configurable up to ten. HTTP 401/403 and other non-retryable responses stop the upload.
- **Progress is not archival confirmation:** Progress measures bytes read and sent by the client. Success is reported after a successful HTTP response; ingestion and derivative processing may continue afterward.
- **Keep source files unchanged:** Size and modification time are checked before each transmission. The client does not perform an additional remote read-back checksum verification.
- **Uploads are generally public:** Valid credentials do not grant write access to every item or collection.

## Development checks

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

The v0.1.1 test suite includes 14 tests covering authentication rejection, redirect boundaries and full-body replay, transient failures, retry exhaustion, proxy routing, Unicode encoding, identifier validation, 5 GiB sparse-file reads, cancellation during reads, GUI state, and logo decoding. HTTP tests bind local loopback ports and use dummy credentials; they do not upload to Internet Archive.

## macOS packaging

```sh
sh scripts/package-macos.sh
sh scripts/package-dmg.sh
```

The DMG script builds for the host architecture and creates an installer and a SHA-256 file in `dist/`. It requires macOS command-line developer tools and Python 3. The installer contains the app, an Applications shortcut, the CLI, and Chinese installation instructions.

The original logo is in `assets/logo-v1.png`; its generation prompt is in `assets/logo-prompt.txt`. Packaging generates the multi-size `assets/AppIcon.icns` and embeds the volume icon inside the DMG. The external DMG's Finder icon uses macOS extended metadata, which some transfer tools strip; the mounted-volume icon remains part of the image.

## License and references

Released under the [MIT License](LICENSE). This project is independently developed and is not an official Internet Archive application.

- [Internet Archive S3-like API documentation](https://archive.org/developers/ias3.html)
- [Internet Archive S3 credentials](https://archive.org/account/s3.php)
