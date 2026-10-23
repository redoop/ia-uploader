#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use eframe::egui;
use ia_uploader::{Command, Control, Event, Network};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};

enum Message {
    Finished(Result<(), String>, bool),
}
struct App {
    access: String,
    secret: String,
    verified: bool,
    network_mode: u8,
    proxy: String,
    identifier: String,
    title: String,
    collection: String,
    media: String,
    files: Vec<PathBuf>,
    retries: u32,
    no_derive: bool,
    busy: bool,
    control: Control,
    rx: mpsc::Receiver<Event>,
    result_tx: mpsc::Sender<Message>,
    result_rx: mpsc::Receiver<Message>,
    status: String,
    current: String,
    progress: f32,
    speed: f64,
    completed: usize,
    error: bool,
}
impl Default for App {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            access: String::new(),
            secret: String::new(),
            verified: false,
            network_mode: 0,
            proxy: "http://127.0.0.1:7890".into(),
            identifier: String::new(),
            title: String::new(),
            collection: "opensource".into(),
            media: "data".into(),
            files: vec![],
            retries: 3,
            no_derive: false,
            busy: false,
            control: Control {
                events: Some(tx),
                ..Default::default()
            },
            rx,
            result_tx,
            result_rx,
            status: "请先输入凭据并验证账号".into(),
            current: String::new(),
            progress: 0.,
            speed: 0.,
            completed: 0,
            error: false,
        }
    }
}
impl App {
    fn add_files(&mut self, files: Vec<PathBuf>) {
        for path in files {
            if !self.files.contains(&path) {
                self.files.push(path);
            }
        }
    }
    fn start(&mut self, auth_only: bool) {
        let auth = match ia_uploader::auth_header(&self.access, &self.secret) {
            Ok(auth) => auth,
            Err(e) => {
                self.status = e.to_string();
                self.error = true;
                return;
            }
        };
        self.control.network = match self.network_mode {
            1 => Network::Direct,
            2 => Network::Proxy(self.proxy.trim().into()),
            _ => Network::Auto,
        };
        self.control.cancel.store(false, Ordering::Relaxed);
        let command = if auth_only {
            Command::Auth
        } else {
            Command::Upload {
                identifier: self.identifier.trim().into(),
                files: self.files.clone(),
                title: if self.title.trim().is_empty() {
                    None
                } else {
                    Some(self.title.clone())
                },
                mediatype: self.media.clone(),
                collection: self.collection.trim().into(),
                retries: self.retries,
                no_derive: self.no_derive,
            }
        };
        self.busy = true;
        self.error = false;
        self.progress = 0.;
        self.speed = 0.;
        self.completed = 0;
        self.current.clear();
        self.status = "正在验证账号…".into();
        let control = self.control.clone();
        let tx = self.result_tx.clone();
        std::thread::spawn(move || {
            let result = ia_uploader::execute(command, auth, control).map_err(|e| format!("{e:#}"));
            let _ = tx.send(Message::Finished(result, auth_only));
        });
    }
    fn messages(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Verified => {
                    self.verified = true;
                    self.status = "账号验证成功".into();
                }
                Event::Started(name, _) => {
                    self.current = name;
                    self.progress = 0.;
                    self.status = "正在上传".into();
                }
                Event::Progress(sent, total, speed) => {
                    self.progress = if total == 0 {
                        1.
                    } else {
                        (sent as f64 / total as f64).clamp(0., 1.) as f32
                    };
                    self.speed = speed;
                }
                Event::Accepted(_) => {
                    self.completed += 1;
                    self.progress = 1.;
                    self.status = "服务器已接受文件".into();
                }
                Event::Retry(n) => {
                    self.progress = 0.;
                    self.speed = 0.;
                    self.status = format!("等待重试（第 {n} 次，将从文件开头发送）");
                }
            }
        }
        while let Ok(Message::Finished(result, auth_only)) = self.result_rx.try_recv() {
            self.busy = false;
            match result {
                Ok(()) => {
                    self.status = if auth_only {
                        "账号验证成功，可以添加文件并上传".into()
                    } else {
                        "上传完成，Internet Archive 正在处理归档".into()
                    };
                }
                Err(e) => {
                    if auth_only {
                        self.verified = false;
                    }
                    self.error = true;
                    self.status = e;
                }
            }
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.messages();
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if !self.busy {
            let dropped = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect()
            });
            self.add_files(dropped);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.);
            ui.heading(egui::RichText::new("Internet Archive 上传器").size(27.));
            ui.label(egui::RichText::new("将大文件上传到 archive.org · Rust 桌面客户端").color(egui::Color32::GRAY));
            ui.add_space(16.);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("1. 账号验证");
                    ui.add_enabled_ui(!self.busy, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("连接方式");
                            let previous = self.network_mode;
                            egui::ComboBox::from_id_salt("network").selected_text(match self.network_mode { 1 => "直连", 2 => "手动代理", _ => "自动代理（系统 / 环境变量）" }).show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.network_mode, 0, "自动代理（系统 / 环境变量）");
                                ui.selectable_value(&mut self.network_mode, 1, "直连");
                                ui.selectable_value(&mut self.network_mode, 2, "手动代理");
                            });
                            if previous != self.network_mode { self.verified = false; }
                        });
                        if self.network_mode == 2 {
                            ui.horizontal(|ui| { ui.label("代理地址"); if ui.add(egui::TextEdit::singleline(&mut self.proxy).password(true).hint_text("http://127.0.0.1:7890").desired_width(420.)).changed() { self.verified = false; } });
                            ui.small("支持 HTTP / HTTPS / SOCKS5；代理仅在本次运行中生效。");
                        }
                    });
                    ui.hyperlink_to("获取 Internet Archive S3 凭据 ↗", "https://archive.org/account/s3.php");
                    ui.add_enabled_ui(!self.busy, |ui| {
                        egui::Grid::new("credentials").num_columns(2).spacing([16., 9.]).show(ui, |ui| {
                            ui.label("Access Key");
                            if ui.add(egui::TextEdit::singleline(&mut self.access).desired_width(450.)).changed() { self.verified = false; }
                            ui.end_row(); ui.label("Secret Key");
                            if ui.add(egui::TextEdit::singleline(&mut self.secret).password(true).desired_width(450.)).changed() { self.verified = false; }
                            ui.end_row();
                        });
                        if ui.button(if self.verified { "✓ 重新验证账号" } else { "验证账号" }).clicked() { self.start(true); }
                    });
                    ui.small("密钥仅保留在本次运行的内存中，不保存到磁盘。");
                });
                ui.add_space(10.);
                ui.group(|ui| {
                    ui.set_width(ui.available_width()); ui.strong("2. 归档信息");
                    ui.add_enabled_ui(!self.busy, |ui| {
                        egui::Grid::new("metadata").num_columns(2).spacing([16., 9.]).show(ui, |ui| {
                            ui.label("唯一标识 Identifier"); ui.add(egui::TextEdit::singleline(&mut self.identifier).hint_text("my-archive-20261023").desired_width(450.)); ui.end_row();
                            ui.label("标题"); ui.add(egui::TextEdit::singleline(&mut self.title).hint_text("可选，支持中文").desired_width(450.)); ui.end_row();
                            ui.label("Collection"); ui.text_edit_singleline(&mut self.collection); ui.end_row();
                            ui.label("媒体类型");
                            egui::ComboBox::from_id_salt("media").selected_text(&self.media).show_ui(ui, |ui| {
                                for value in ["data", "movies", "audio", "texts", "image", "software"] { ui.selectable_value(&mut self.media, value.into(), value); }
                            }); ui.end_row();
                        });
                        ui.horizontal(|ui| { ui.checkbox(&mut self.no_derive, "不生成派生文件"); ui.label("重试次数"); ui.add(egui::DragValue::new(&mut self.retries).range(0..=10)); });
                    });
                });
                ui.add_space(10.);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.strong(format!("3. 上传队列 · {} 个文件", self.files.len()));
                        ui.add_enabled_ui(!self.busy, |ui| {
                            if ui.button("＋ 添加文件").clicked() && let Some(files) = rfd::FileDialog::new().pick_files() { self.add_files(files); }
                            if ui.button("清空").clicked() { self.files.clear(); }
                        });
                    });
                    if self.files.is_empty() { ui.add_space(12.); ui.label("点击添加文件，或将文件拖入窗口"); ui.add_space(12.); }
                    let mut remove = None;
                    for (index, path) in self.files.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!self.busy, egui::Button::new("移除")).clicked() { remove = Some(index); }
                            ui.label(path.file_name().unwrap_or_default().to_string_lossy()).on_hover_text(path.display().to_string());
                            match path.metadata() { Ok(m) => { ui.weak(format!("{:.2} GiB", m.len() as f64 / 1073741824.)); }, Err(_) => { ui.colored_label(egui::Color32::RED, "文件不可访问"); } }
                        });
                    }
                    if let Some(index) = remove { self.files.remove(index); }
                });
                ui.add_space(12.);
                ui.label("上传内容通常公开可读；同名远端文件将替换，服务端保留旧版本。重试会重传整个文件。");
                ui.horizontal(|ui| {
                    let ready = !self.busy && self.verified && !self.files.is_empty() && !self.identifier.trim().is_empty();
                    if ui.add_enabled(ready, egui::Button::new(egui::RichText::new("开始上传").size(18.)).min_size(egui::vec2(140., 40.))).clicked() { self.start(false); }
                    if ui.add_enabled(self.busy, egui::Button::new("取消任务")).clicked() { self.control.cancel.store(true, Ordering::Relaxed); self.status = "正在取消，等待网络操作返回…".into(); }
                    if self.completed > 0 && !self.busy { ui.hyperlink_to("打开归档页面 ↗", format!("https://archive.org/details/{}", self.identifier.trim())); }
                });
                ui.add_space(10.);
                if !self.current.is_empty() { ui.label(format!("{} · 已完成 {}/{} · {:.2} MiB/s", self.current, self.completed, self.files.len(), self.speed / 1048576.)); }
                ui.add(egui::ProgressBar::new(self.progress).show_percentage().animate(self.busy));
                ui.colored_label(if self.error { egui::Color32::from_rgb(210, 65, 65) } else { egui::Color32::from_rgb(40, 140, 110) }, &self.status);
            });
        });
    }
}
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo-v1.png"))
                    .expect("packaged application icon must be a valid PNG"),
            )
            .with_inner_size([820., 850.])
            .with_min_inner_size([680., 600.]),
        ..Default::default()
    };
    eframe::run_native(
        "Internet Archive 上传器",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            let mut fonts = egui::FontDefinitions::default();
            // Use system CJK fonts without redistributing proprietary font files.
            for path in [
                "/System/Library/Fonts/STHeiti Light.ttc",
                "C:/Windows/Fonts/msyh.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            ] {
                if let Ok(bytes) = std::fs::read(path) {
                    fonts
                        .font_data
                        .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .insert(0, "cjk".into());
                    cc.egui_ctx.set_fonts(fonts);
                    break;
                }
            }
            Ok(Box::new(App::default()))
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_deduplicates_and_worker_results_unlock_ui() {
        let mut app = App::default();
        app.add_files(vec!["a.zip".into(), "a.zip".into(), "b.zip".into()]);
        assert_eq!(app.files.len(), 2);
        app.busy = true;
        app.result_tx
            .send(Message::Finished(Err("认证失败".into()), true))
            .unwrap();
        app.messages();
        assert!(!app.busy);
        assert!(!app.verified);
        assert!(app.error);
    }
    #[test]
    fn application_logo_decodes_with_transparency() {
        let icon =
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo-v1.png")).unwrap();
        assert_eq!(icon.width, icon.height);
        assert!(icon.width >= 1024);
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4
        );
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
    #[test]
    fn zero_byte_progress_is_finite() {
        let mut app = App::default();
        app.control
            .events
            .as_ref()
            .unwrap()
            .send(Event::Progress(0, 0, 0.))
            .unwrap();
        app.messages();
        assert_eq!(app.progress, 1.);
    }
}
