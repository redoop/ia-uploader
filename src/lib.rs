pub mod i18n;
use anyhow::{Context, Result, bail, ensure};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
pub use i18n::Language;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{
    Url,
    blocking::{Body, Client},
    header::{AUTHORIZATION, CONTENT_LENGTH, HeaderValue, LOCATION},
};
use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(version, about = "Internet Archive large-file uploader")]
pub struct Cli {
    /// Proxy URL, e.g. http://127.0.0.1:7890 or socks5h://127.0.0.1:7890
    #[arg(long, global = true, conflicts_with = "direct")]
    pub proxy: Option<String>,
    /// Ignore system/environment proxies and connect directly
    #[arg(long, global = true)]
    pub direct: bool,
    /// Interface language: auto, zh, or en
    #[arg(long, global = true, default_value = "auto", env = "IA_UPLOADER_LANG", value_parser = ["auto", "zh", "en"])]
    pub lang: String,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Verify IA_ACCESS_KEY and IA_SECRET_KEY without uploading
    Auth,
    /// Verify credentials, then upload files sequentially; preserve old versions
    Upload {
        /// Internet Archive item identifier
        identifier: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value = "data")]
        mediatype: String,
        #[arg(long, default_value = "opensource")]
        collection: String,
        /// Additional retry attempts for each file
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(0..=10))]
        retries: u32,
        /// Skip server-side derivative generation
        #[arg(long)]
        no_derive: bool,
    },
}

impl Cli {
    pub fn language(&self) -> Language {
        match self.lang.as_str() {
            "zh" => Language::Zh,
            "en" => Language::En,
            _ => Language::system(),
        }
    }
    pub fn parse_localized() -> Self {
        let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
        let language = cli_language(&args);
        let matches = localized_cli(language).get_matches_from(args);
        Self::from_arg_matches(&matches).expect("validated command-line arguments")
    }
}
fn cli_language(args: &[std::ffi::OsString]) -> Language {
    let mut choice = std::env::var("IA_UPLOADER_LANG").unwrap_or_else(|_| "auto".into());
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if arg == "--lang" {
            if let Some(value) = args.next() {
                choice = value.to_string_lossy().into();
            }
        } else if let Some(value) = arg.to_str().and_then(|s| s.strip_prefix("--lang=")) {
            choice = value.into();
        }
    }
    match choice.as_str() {
        "zh" => Language::Zh,
        "en" => Language::En,
        _ => Language::system(),
    }
}
fn localized_cli(lang: Language) -> clap::Command {
    let options = lang.text("选项", "Options");
    let arguments = lang.text("参数", "Arguments");
    let mut command = Cli::command()
        .about(lang.text(
            "Internet Archive 大文件上传工具（S3 API 凭据认证）",
            "Internet Archive large-file uploader (S3 credential authentication)",
        ))
        .disable_help_subcommand(true)
        .subcommand_help_heading(lang.text("命令", "Commands"))
        .mut_arg("proxy", |arg| {
            arg.help(lang.text(
                "手动代理地址，例如 http://127.0.0.1:7890",
                "Proxy URL, e.g. http://127.0.0.1:7890",
            ))
        })
        .mut_arg("direct", |arg| {
            arg.help(lang.text(
                "忽略系统及环境变量代理，直接连接",
                "Ignore system/environment proxies and connect directly",
            ))
        })
        .mut_arg("lang", |arg| {
            arg.help(lang.text(
                "界面语言：auto 跟随系统，zh 中文，en 英文",
                "Language: auto follows the OS, zh for Chinese, en for English",
            ))
        })
        .mut_args(|arg| arg.help_heading(options))
        .mut_subcommand("auth", |cmd| {
            cmd.about(lang.text(
                "验证 IA_ACCESS_KEY 和 IA_SECRET_KEY，不上传文件",
                "Verify IA_ACCESS_KEY and IA_SECRET_KEY without uploading",
            ))
        })
        .mut_subcommand("upload", |cmd| {
            cmd.about(lang.text(
                "验证凭据后依次上传文件；同名远端文件会被替换并保留旧版本",
                "Verify credentials and upload files sequentially; preserve old versions",
            ))
            .mut_arg("identifier", |a| {
                a.help(lang.text(
                    "Internet Archive 归档唯一标识",
                    "Internet Archive item identifier",
                ))
            })
            .mut_arg("files", |a| {
                a.help(lang.text("待上传文件路径", "Paths of files to upload"))
            })
            .mut_arg("title", |a| a.help(lang.text("归档标题", "Item title")))
            .mut_arg("mediatype", |a| a.help(lang.text("媒体类型", "Media type")))
            .mut_arg("collection", |a| {
                a.help(lang.text("目标 collection", "Destination collection"))
            })
            .mut_arg("retries", |a| {
                a.help(lang.text(
                    "每个文件的额外重试次数",
                    "Additional retry attempts per file",
                ))
            })
            .mut_arg("no_derive", |a| {
                a.help(lang.text(
                    "禁止服务端生成派生文件",
                    "Skip server-side derivative generation",
                ))
            })
            .mut_args(|arg| {
                let heading = if arg.is_positional() {
                    arguments
                } else {
                    options
                };
                arg.help_heading(heading)
            })
        });
    // Build once so Clap's generated help/version flags can share the same
    // localized heading as the application flags, avoiding duplicate groups.
    command.build();
    let template = lang.text(
        "{about-with-newline}\n用法：{usage}\n\n{all-args}{after-help}",
        "{about-with-newline}\nUsage: {usage}\n\n{all-args}{after-help}",
    );
    command
        .help_template(template)
        .mut_arg("help", |arg| {
            arg.help(lang.text("显示帮助", "Print help"))
                .help_heading(options)
        })
        .mut_arg("version", |arg| {
            arg.help(lang.text("显示版本", "Print version"))
                .help_heading(options)
        })
        .mut_subcommands(|cmd| {
            cmd.help_template(template).mut_arg("help", |arg| {
                arg.help(lang.text("显示帮助", "Print help"))
                    .help_heading(options)
            })
        })
}

pub fn credentials() -> Result<HeaderValue> {
    credentials_in(Language::detect())
}

pub fn credentials_in(lang: Language) -> Result<HeaderValue> {
    let access = std::env::var("IA_ACCESS_KEY").context(lang.text(
        "请设置 IA_ACCESS_KEY；在 https://archive.org/account/s3.php 获取凭据",
        "Set IA_ACCESS_KEY; get credentials at https://archive.org/account/s3.php",
    ))?;
    let secret = std::env::var("IA_SECRET_KEY")
        .context(lang.text("请设置 IA_SECRET_KEY", "Set IA_SECRET_KEY"))?;
    ensure!(
        !access.trim().is_empty() && !secret.trim().is_empty(),
        lang.text("凭据不能为空", "Credentials must not be empty")
    );
    auth_header_in(&access, &secret, lang)
}

pub fn auth_header(access: &str, secret: &str) -> Result<HeaderValue> {
    auth_header_in(access, secret, Language::detect())
}

pub fn auth_header_in(access: &str, secret: &str, lang: Language) -> Result<HeaderValue> {
    ensure!(
        !access.trim().is_empty() && !secret.trim().is_empty(),
        lang.text("凭据不能为空", "Credentials must not be empty")
    );
    let mut value = HeaderValue::from_str(&format!("LOW {access}:{secret}"))
        .map_err(|_| anyhow::anyhow!(lang.text("凭据格式无效", "Invalid credential format")))?;
    value.set_sensitive(true);
    Ok(value)
}

fn trusted(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .host_str()
            .is_some_and(|h| h == "archive.org" || h.ends_with(".archive.org"))
}

fn identifier_valid(s: &str) -> bool {
    (5..=100).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && s.as_bytes()[0].is_ascii_alphanumeric()
}

#[cfg(test)]
fn object_url(identifier: &str, name: &str) -> Result<Url> {
    let mut url = Url::parse("https://s3.us.archive.org/")?;
    url.path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(identifier)
        .push(name);
    Ok(url)
}

fn metadata(s: &str) -> String {
    format!("uri({})", utf8_percent_encode(s, NON_ALPHANUMERIC))
}

fn authenticate(client: &Client, auth: &HeaderValue, base: &Url, lang: Language) -> Result<()> {
    let response = client
        .get(base.join("?check_auth=1")?)
        .header(AUTHORIZATION, auth.clone())
        .timeout(Duration::from_secs(60))
        .send()
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                anyhow::anyhow!(lang.text("无法连接 Internet Archive（连接失败或超时）。请检查网络及代理设置；如使用 Clash，请选自动代理或填写 http://127.0.0.1:7890。这不代表密钥无效。", "Cannot connect to Internet Archive (connection failed or timed out). Check your network and proxy settings. For Clash, use automatic proxy or http://127.0.0.1:7890. This does not mean your keys are invalid."))
            } else {
                anyhow::anyhow!(tr!(lang, "验证请求失败：{}", "Authentication request failed: {}", e.without_url()))
            }
        })?;
    ensure!(
        response.status().is_success(),
        tr!(
            lang,
            "认证失败，HTTP {}",
            "Authentication failed: HTTP {}",
            response.status()
        )
    );
    let value: serde_json::Value = response
        .json()
        .context(lang.text("认证响应不是 JSON", "Authentication response is not JSON"))?;
    ensure!(
        value
            .get("error")
            .is_none_or(|v| v.is_null() || v == false || v == ""),
        lang.text("凭据验证失败", "Credential verification failed")
    );
    // Fail closed: a random successful JSON response is not proof of authentication.
    ensure!(
        value
            .get("username")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty()),
        lang.text(
            "认证响应缺少 username，无法确认身份",
            "Authentication response has no username; identity could not be verified"
        )
    );
    println!("{}", lang.text("账号验证成功", "Account verified"));
    Ok(())
}

#[derive(Clone, Debug)]
pub enum Event {
    Verified,
    Started(String, u64),
    Progress(u64, u64, f64),
    Accepted(String),
    Retry(u32),
}
#[derive(Clone, Default, PartialEq, Eq)]
pub enum Network {
    #[default]
    Auto,
    Direct,
    Proxy(String),
}
#[cfg(test)]
fn proxy_url(raw: &str) -> Result<Url> {
    proxy_url_in(raw, Language::En)
}

fn proxy_url_in(raw: &str, lang: Language) -> Result<Url> {
    let url = Url::parse(raw).map_err(|_| {
        anyhow::anyhow!(lang.text(
            "代理地址无效，示例：http://127.0.0.1:7890",
            "Invalid proxy URL; example: http://127.0.0.1:7890"
        ))
    })?;
    ensure!(
        matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
            && url.host_str().is_some()
            && url.query().is_none()
            && url.fragment().is_none()
            && matches!(url.path(), "" | "/"),
        lang.text(
            "代理地址须使用 HTTP、HTTPS 或 SOCKS5 协议，不应包含路径、查询参数或片段",
            "Proxy URL must use HTTP, HTTPS or SOCKS5, without a path, query or fragment"
        )
    );
    Ok(url)
}

#[derive(Clone, Default)]
pub struct Control {
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub events: Option<std::sync::mpsc::Sender<Event>>,
    pub network: Network,
    pub language: Language,
}
impl Control {
    fn emit(&self, event: Event) {
        if let Some(tx) = &self.events {
            let _ = tx.send(event);
        }
    }
    fn check(&self) -> Result<()> {
        let lang = self.language;
        ensure!(
            !self.cancel.load(std::sync::atomic::Ordering::Relaxed),
            lang.text("上传已取消", "Upload cancelled")
        );
        Ok(())
    }
}

struct Progress {
    control: Control,
    file: File,
    sent: u64,
    total: u64,
    started: Instant,
    last: Instant,
}
impl Read for Progress {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let lang = self.control.language;
        if self
            .control
            .cancel
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(io::Error::other(
                lang.text("上传已取消", "Upload cancelled"),
            ));
        }
        let n = self.file.read(buf)?;
        self.sent += n as u64;
        if self.last.elapsed() >= Duration::from_millis(500) || n == 0 {
            eprint!(
                "{}",
                tr!(
                    lang,
                    "\r已发送 {:.2}/{:.2} GiB  {:.2} MiB/s（等待服务端确认）",
                    "\rSent {:.2}/{:.2} GiB  {:.2} MiB/s (awaiting server confirmation)",
                    self.sent as f64 / 1073741824.,
                    self.total as f64 / 1073741824.,
                    self.sent as f64 / 1048576. / self.started.elapsed().as_secs_f64().max(0.001)
                )
            );
            self.control.emit(Event::Progress(
                self.sent,
                self.total,
                self.sent as f64 / self.started.elapsed().as_secs_f64().max(0.001),
            ));
            self.last = Instant::now();
        }
        Ok(n)
    }
}

pub fn run(cli: Cli) -> Result<()> {
    let lang = cli.language();
    let network = if cli.direct {
        Network::Direct
    } else if let Some(proxy) = cli.proxy {
        Network::Proxy(proxy)
    } else {
        Network::Auto
    };
    execute(
        cli.command,
        credentials_in(lang)?,
        Control {
            network,
            language: lang,
            ..Default::default()
        },
    )
}

pub fn execute(command: Command, auth: HeaderValue, control: Control) -> Result<()> {
    execute_at(
        command,
        auth,
        control,
        Url::parse("https://s3.us.archive.org/")?,
        trusted,
    )
}

fn execute_at(
    command: Command,
    auth: HeaderValue,
    control: Control,
    base: Url,
    allowed: fn(&Url) -> bool,
) -> Result<()> {
    let lang = control.language;
    let builder = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(30))
        // A total request deadline would interrupt slow, multi-hour uploads.
        .timeout(None)
        .http1_only()
        .user_agent(concat!("ia-uploader/", env!("CARGO_PKG_VERSION")));
    let builder = match &control.network {
        Network::Auto => builder,
        Network::Direct => builder.no_proxy(),
        Network::Proxy(raw) => {
            builder
                .no_proxy()
                .proxy(reqwest::Proxy::all(proxy_url_in(raw, lang)?).map_err(|_| {
                    anyhow::anyhow!(lang.text("无法创建代理配置", "Could not configure proxy"))
                })?)
        }
    };
    let client = builder.build()?;
    match command {
        Command::Auth => {
            authenticate(&client, &auth, &base, lang)?;
            control.emit(Event::Verified);
            Ok(())
        }
        Command::Upload {
            identifier,
            files,
            title,
            mediatype,
            collection,
            retries,
            no_derive,
        } => {
            ensure!(
                identifier_valid(&identifier),
                lang.text("identifier 必须为 5–100 个 ASCII 字母、数字、点、下划线或横线，且以字母或数字开头", "Identifier must contain 5–100 ASCII letters, digits, dots, underscores or hyphens, starting with a letter or digit")
            );
            // Validate the entire queue before making any changes remotely.
            let mut names = std::collections::HashSet::new();
            let mut queue = Vec::new();
            let mut total = 0u64;
            for path in files {
                let file = File::open(&path)
                    .with_context(|| tr!(lang, "无法打开 {}", "Cannot open {}", path.display()))?;
                let info = file.metadata()?;
                ensure!(
                    info.is_file(),
                    tr!(
                        lang,
                        "{} 不是普通文件",
                        "{} is not a regular file",
                        path.display()
                    )
                );
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context(lang.text("文件名须为 UTF-8", "Filename must be valid UTF-8"))?
                    .to_owned();
                ensure!(
                    name != "." && name != ".." && !name.contains('\\'),
                    lang.text("无效文件名", "Invalid filename")
                );
                ensure!(
                    names.insert(name.clone()),
                    tr!(
                        lang,
                        "队列包含同名文件：{name}",
                        "Duplicate filename in queue: {name}"
                    )
                );
                total = total
                    .checked_add(info.len())
                    .context(lang.text("文件总大小溢出", "Total file size overflow"))?;
                queue.push((path, name, info.len(), info.modified()?));
            }
            control.check()?;
            authenticate(&client, &auth, &base, lang)?;
            control.emit(Event::Verified);
            for (path, name, size, modified) in queue {
                println!(
                    "{}",
                    tr!(
                        lang,
                        "上传 {}（{} 字节）",
                        "Uploading {} ({} bytes)",
                        path.display(),
                        size
                    )
                );
                control.emit(Event::Started(name.clone(), size));
                let mut original = base.clone();
                original
                    .path_segments_mut()
                    .map_err(|_| anyhow::anyhow!(lang.text("无效端点", "Invalid endpoint")))?
                    .pop_if_empty()
                    .push(&identifier)
                    .push(&name);
                let mut completed = false;
                for attempt in 0..=retries {
                    let mut url = original.clone();
                    let mut retryable = false;
                    for redirects in 0..=5 {
                        control.check()?;
                        ensure!(
                            allowed(&url),
                            lang.text(
                                "拒绝向非可信地址发送凭据",
                                "Refusing to send credentials to an untrusted address"
                            )
                        );
                        let file = File::open(&path)?;
                        let info = file.metadata()?;
                        ensure!(
                            info.len() == size && info.modified()? == modified,
                            lang.text(
                                "源文件已改变，请重新运行",
                                "Source file changed; restart the upload"
                            )
                        );
                        let stream = Progress {
                            control: control.clone(),
                            file,
                            sent: 0,
                            total: size,
                            started: Instant::now(),
                            last: Instant::now(),
                        };
                        let request = client
                            .put(url.clone())
                            .header(AUTHORIZATION, auth.clone())
                            .header(CONTENT_LENGTH, size)
                            .header("Content-Type", "application/octet-stream")
                            .header("x-archive-auto-make-bucket", "1")
                            .header("x-archive-size-hint", total)
                            .header("x-archive-keep-old-version", "1")
                            .header(
                                "x-archive-meta-title",
                                metadata(title.as_deref().unwrap_or(&identifier)),
                            )
                            .header("x-archive-meta-mediatype", metadata(&mediatype))
                            .header("x-archive-meta-collection", metadata(&collection))
                            .header("x-archive-queue-derive", if no_derive { "0" } else { "1" })
                            .body(Body::sized(stream, size));
                        match request.send() {
                            Err(e) => {
                                eprintln!(
                                    "{}",
                                    tr!(
                                        lang,
                                        "\n网络请求失败：{}",
                                        "\nNetwork request failed: {}",
                                        e.without_url()
                                    )
                                );
                                retryable = true;
                                break;
                            }
                            Ok(response) => {
                                let status = response.status();
                                if status.is_success() {
                                    completed = true;
                                    break;
                                }
                                if status.as_u16() == 307 || status.as_u16() == 308 {
                                    ensure!(
                                        redirects < 5,
                                        lang.text("重定向次数过多", "Too many redirects")
                                    );
                                    let location = response
                                        .headers()
                                        .get(LOCATION)
                                        .context(lang.text(
                                            "重定向缺少 Location",
                                            "Redirect has no Location header",
                                        ))?
                                        .to_str()?;
                                    url = url
                                        .join(location)
                                        .context(lang.text("无效重定向", "Invalid redirect"))?;
                                    continue;
                                }
                                if status.as_u16() == 429 || status.is_server_error() {
                                    eprintln!(
                                        "{}",
                                        tr!(
                                            lang,
                                            "\n服务器暂不可用：HTTP {status}",
                                            "\nServer temporarily unavailable: HTTP {status}"
                                        )
                                    );
                                    retryable = true;
                                    break;
                                }
                                bail!(tr!(
                                    lang,
                                    "上传被拒绝：HTTP {status}（检查 item 所有权、collection 权限及凭据）",
                                    "Upload rejected: HTTP {status} (check item ownership, collection permissions and credentials)"
                                ));
                            }
                        }
                    }
                    control.check()?;
                    if completed {
                        break;
                    }
                    if !retryable || attempt == retries {
                        bail!(tr!(
                            lang,
                            "上传失败：{name}；再次运行将从文件开头重传",
                            "Upload failed: {name}; running again will restart the file from the beginning"
                        ));
                    }
                    let delay = Duration::from_secs((2u64.pow(attempt + 1)).min(60));
                    eprintln!(
                        "{}",
                        tr!(
                            lang,
                            "{} 秒后重试 {}/{}（从文件开头重传）",
                            "Retrying in {} seconds, attempt {}/{} (restarting the file)",
                            delay.as_secs(),
                            attempt + 1,
                            retries
                        )
                    );
                    control.emit(Event::Retry(attempt + 1));
                    for _ in 0..delay.as_millis() / 100 {
                        control.check()?;
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                control.emit(Event::Accepted(name.clone()));
                println!(
                    "{}",
                    tr!(lang, "\n服务端已接受：{name}", "\nServer accepted: {name}")
                );
            }
            println!(
                "{}",
                tr!(
                    lang,
                    "上传完成：https://archive.org/details/{identifier}（归档处理可能需要等待）",
                    "Upload complete: https://archive.org/details/{identifier} (archival processing may take time)"
                )
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirect_credentials_are_confined() {
        for url in [
            "http://s3.us.archive.org/a",
            "https://archive.org.evil.com/a",
            "https://evilarchive.org/a",
            "https://user@archive.org/a",
            "https://archive.org:444/a",
        ] {
            assert!(!trusted(&Url::parse(url).unwrap()), "{url}");
        }
        assert!(trusted(&Url::parse("https://ia801.archive.org/a").unwrap()));
    }
    #[test]
    fn paths_and_unicode_metadata_are_encoded() {
        let url = object_url("test-item", "中文 #?.zip").unwrap();
        assert!(url.query().is_none());
        assert!(url.fragment().is_none());
        assert_eq!(url.path_segments().unwrap().count(), 2);
        assert_eq!(metadata("标题"), "uri(%E6%A0%87%E9%A2%98)");
    }
    #[test]
    fn cli_help_and_errors_use_selected_language() {
        for lang in [Language::En, Language::Zh] {
            let mut command = localized_cli(lang);
            let help = command.render_long_help().to_string();
            assert!(help.contains(lang.text("界面语言", "Language:")), "{help}");
            assert!(
                help.contains(lang.text("手动代理地址", "Proxy URL")),
                "{help}"
            );
            let upload = command
                .find_subcommand_mut("upload")
                .unwrap()
                .render_long_help()
                .to_string();
            assert!(
                upload.contains(lang.text(
                    "每个文件的额外重试次数",
                    "Additional retry attempts per file"
                )),
                "{upload}"
            );
            let error = auth_header_in("", "", lang).unwrap_err().to_string();
            assert_eq!(
                error,
                lang.text("凭据不能为空", "Credentials must not be empty")
            );
            if lang == Language::En {
                assert!(!help.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)));
            }
            let args: Vec<std::ffi::OsString> = [
                "ia-upload-cli",
                "auth",
                "--lang",
                if lang == Language::Zh { "zh" } else { "en" },
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            assert_eq!(cli_language(&args), lang);
        }
    }
    #[test]
    fn identifiers_reject_paths() {
        assert!(identifier_valid("test-item_2026"));
        for id in ["../oops", "a/bcd", "中文名称", ".hidden", "a"] {
            assert!(!identifier_valid(id));
        }
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use std::{
        io::Write,
        net::TcpListener,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
    };

    type Requests = Arc<Mutex<Vec<Vec<u8>>>>;
    fn server(responses: Vec<String>) -> (Url, Requests, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let capture = requests.clone();
        let handle = std::thread::spawn(move || {
            for reply in responses {
                let deadline = Instant::now() + Duration::from_secs(15);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "expected request did not arrive");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut data = vec![];
                let mut buffer = [0; 8192];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0, "unexpected EOF");
                    data.extend_from_slice(&buffer[..count]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]).to_lowercase();
                        let size: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .unwrap_or("0")
                            .parse()
                            .unwrap();
                        if data.len() >= end + 4 + size {
                            break;
                        }
                    }
                }
                capture.lock().unwrap().push(data);
                if !reply.is_empty() {
                    socket.write_all(reply.as_bytes()).unwrap();
                }
            }
        });
        (url, requests, handle)
    }
    fn reply(status: &str, extra: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n{extra}\r\n{body}",
            body.len()
        )
    }
    fn good_auth() -> String {
        reply(
            "200 OK",
            "Content-Type: application/json\r\n",
            r#"{"username":"test-user"}"#,
        )
    }
    fn local(url: &Url) -> bool {
        url.scheme() == "http" && url.host_str() == Some("127.0.0.1")
    }
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "ia-upload-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(&path, b"payload\0with\xffbinary").unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn upload(file: &Temp, retries: u32) -> Command {
        Command::Upload {
            identifier: "test-item".into(),
            files: vec![file.0.clone()],
            title: Some("中文标题".into()),
            mediatype: "data".into(),
            collection: "opensource".into(),
            retries,
            no_derive: true,
        }
    }
    fn execute_test(command: Command, base: Url, mut control: Control) -> Result<()> {
        control.network = Network::Direct;
        execute_at(
            command,
            auth_header("fake-access", "fake-secret").unwrap(),
            control,
            base,
            local,
        )
    }
    fn payload(request: &[u8]) -> &[u8] {
        let index = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        &request[index + 4..]
    }
    #[test]
    fn authentication_rejection_never_uploads() {
        for response in [
            reply("403 Forbidden", "", ""),
            reply(
                "200 OK",
                "",
                r#"{"error":"bad credentials","username":"user"}"#,
            ),
            reply("200 OK", "", "{}"),
            reply("200 OK", "", "not json"),
        ] {
            let file = Temp::new();
            let (base, requests, worker) = server(vec![response]);
            assert!(execute_test(upload(&file, 0), base, Control::default()).is_err());
            worker.join().unwrap();
            assert_eq!(requests.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn redirect_replays_full_body_and_headers() {
        let file = Temp::new();
        let (base, requests, worker) = server(vec![
            good_auth(),
            reply("307 Temporary Redirect", "Location: /redirected\r\n", ""),
            reply("200 OK", "", ""),
        ]);
        let (tx, rx) = std::sync::mpsc::channel();
        execute_test(
            upload(&file, 0),
            base,
            Control {
                events: Some(tx),
                ..Default::default()
            },
        )
        .unwrap();
        worker.join().unwrap();
        let requests = requests.lock().unwrap();
        assert_eq!(payload(&requests[1]), payload(&requests[2]));
        assert_eq!(payload(&requests[1]), std::fs::read(&file.0).unwrap());
        let headers = String::from_utf8_lossy(&requests[2]).to_lowercase();
        assert!(headers.starts_with("put /redirected "));
        assert!(headers.contains("authorization: low fake-access:fake-secret"));
        assert!(headers.contains("x-archive-queue-derive: 0"));
        assert!(headers.contains("x-archive-meta-title: uri(%e4%b8%ad"));
        assert_eq!(
            rx.try_iter()
                .filter(|e| matches!(e, Event::Accepted(_)))
                .count(),
            1
        );
    }
    #[test]
    fn transient_http_and_disconnect_retry_full_file() {
        for failure in [reply("503 Service Unavailable", "", ""), String::new()] {
            let file = Temp::new();
            let (base, requests, worker) =
                server(vec![good_auth(), failure, reply("200 OK", "", "")]);
            execute_test(upload(&file, 1), base, Control::default()).unwrap();
            worker.join().unwrap();
            let requests = requests.lock().unwrap();
            assert_eq!(requests.len(), 3);
            assert_eq!(payload(&requests[1]), payload(&requests[2]));
        }
    }
    #[test]
    fn forbidden_and_untrusted_redirect_are_terminal() {
        for response in [
            reply("403 Forbidden", "", ""),
            reply(
                "307 Temporary Redirect",
                "Location: https://example.com/leak\r\n",
                "",
            ),
        ] {
            let file = Temp::new();
            let (base, requests, worker) = server(vec![good_auth(), response]);
            assert!(execute_test(upload(&file, 3), base, Control::default()).is_err());
            worker.join().unwrap();
            assert_eq!(requests.lock().unwrap().len(), 2);
        }
    }
    #[test]
    fn retry_exhaustion_does_not_report_accepted() {
        let file = Temp::new();
        let (base, _, worker) = server(vec![good_auth(), reply("429 Too Many Requests", "", "")]);
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(
            execute_test(
                upload(&file, 0),
                base,
                Control {
                    events: Some(tx),
                    ..Default::default()
                }
            )
            .is_err()
        );
        worker.join().unwrap();
        assert!(!rx.try_iter().any(|e| matches!(e, Event::Accepted(_))));
    }
    #[test]
    fn explicit_proxy_routes_both_authentication_and_upload() {
        let file = Temp::new();
        let (proxy, requests, worker) = server(vec![good_auth(), reply("200 OK", "", "")]);
        let control = Control {
            network: Network::Proxy(proxy.to_string()),
            ..Default::default()
        };
        execute_at(
            upload(&file, 0),
            auth_header("fake-access", "fake-secret").unwrap(),
            control,
            Url::parse("http://not-a-real-host.invalid/").unwrap(),
            |_| true,
        )
        .unwrap();
        worker.join().unwrap();
        let requests = requests.lock().unwrap();
        assert!(
            String::from_utf8_lossy(&requests[0])
                .starts_with("GET http://not-a-real-host.invalid/?check_auth=1 ")
        );
        assert!(
            String::from_utf8_lossy(&requests[1])
                .starts_with("PUT http://not-a-real-host.invalid/test-item/")
        );
        assert_eq!(payload(&requests[1]), std::fs::read(&file.0).unwrap());
    }
    #[test]
    fn proxy_validation_and_cli_flags() {
        for value in [
            "http://127.0.0.1:7890",
            "socks5h://localhost:1080",
            "https://proxy.example:443",
        ] {
            assert!(proxy_url(value).is_ok());
        }
        for value in [
            "127.0.0.1:7890",
            "file:///tmp/proxy",
            "http://proxy/path",
            "http://proxy/?password=secret",
        ] {
            assert!(proxy_url(value).is_err());
        }
        assert!(
            Cli::try_parse_from(["ia-upload-cli", "auth", "--proxy", "http://localhost:7890"])
                .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "ia-upload-cli",
                "auth",
                "--direct",
                "--proxy",
                "http://localhost:7890"
            ])
            .is_err()
        );
    }
    #[test]
    fn sparse_large_file_uses_u64_and_cancels_reads() {
        let file = Temp::new();
        let size = 5u64 * 1024 * 1024 * 1024;
        File::options()
            .write(true)
            .open(&file.0)
            .unwrap()
            .set_len(size)
            .unwrap();
        let control = Control::default();
        let mut reader = Progress {
            file: File::open(&file.0).unwrap(),
            control: control.clone(),
            sent: 0,
            total: size,
            started: Instant::now(),
            last: Instant::now(),
        };
        let mut buffer = [0; 65536];
        assert_eq!(reader.read(&mut buffer).unwrap(), buffer.len());
        assert_eq!(reader.total, size);
        assert_eq!(reader.sent, 65536);
        control.cancel.store(true, Ordering::Relaxed);
        assert!(reader.read(&mut buffer).is_err());
    }
}
