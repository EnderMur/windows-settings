#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;


fn main() -> eframe::Result<()> {
    let logger = Arc::new(Logger::new());
    logger.log(LogLevel::Normal, "=== Application started ===");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 650.0])
            .with_min_inner_size([700.0, 450.0])
            .with_title("Windows Settings"),
        ..Default::default()
    };

    let logger_for_app = logger.clone();
    eframe::run_native(
        "Windows Settings",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(dark_visuals());
            cc.egui_ctx.set_pixels_per_point(1.15);
            let mut app = App::new(logger_for_app);
            app.spawn_initial_status_check(cc.egui_ctx.clone());
            Ok(Box::new(app))
        }),
    )
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Segoe UI — основной текст (полная поддержка Cyrillic + Win-специфика)
    if let Ok(bytes) = fs::read("C:/Windows/Fonts/segoeui.ttf") {
        fonts.font_data.insert(
            "segoe_ui".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.insert(0, "segoe_ui".to_owned());
        }
    }

    // Segoe UI Symbol — для редких символов вроде шестерёнки и т.п.
    if let Ok(bytes) = fs::read("C:/Windows/Fonts/seguisym.ttf") {
        fonts.font_data.insert(
            "segoe_sym".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.push("segoe_sym".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(24, 24, 28);
    v.window_fill = egui::Color32::from_rgb(24, 24, 28);
    v.extreme_bg_color = egui::Color32::from_rgb(18, 18, 22);
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(32, 32, 38);
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 48);
    v.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(40, 40, 48);
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(56, 56, 68);
    v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(56, 56, 68);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(72, 72, 88);
    v.widgets.active.weak_bg_fill = egui::Color32::from_rgb(72, 72, 88);
    v.selection.bg_fill = egui::Color32::from_rgb(80, 110, 200);
    v
}

// =================== Logger ===================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LogLevel {
    Normal,
    Debug,
}

struct Logger {
    file: Mutex<Option<File>>,
    path: PathBuf,
    level: AtomicU8, // 0 = Normal, 1 = Debug
}

impl Logger {
    fn new() -> Self {
        let dir = appdata_logs_dir();
        let _ = fs::create_dir_all(&dir);
        let filename = format!("run_{}.log", local_timestamp_filename());
        let path = dir.join(filename);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
        Self {
            file: Mutex::new(file),
            path,
            level: AtomicU8::new(0),
        }
    }

    fn current_level(&self) -> LogLevel {
        match self.level.load(Ordering::Relaxed) {
            1 => LogLevel::Debug,
            _ => LogLevel::Normal,
        }
    }

    fn set_level(&self, level: LogLevel) {
        let new = match level {
            LogLevel::Normal => 0,
            LogLevel::Debug => 1,
        };
        let old = self.level.swap(new, Ordering::Relaxed);
        if old != new {
            self.log(
                LogLevel::Normal,
                &format!("Log level changed to {level:?}"),
            );
        }
    }

    fn log(&self, msg_level: LogLevel, msg: &str) {
        // Debug-сообщения пишем только если включён Debug
        if msg_level == LogLevel::Debug && self.current_level() != LogLevel::Debug {
            return;
        }
        let stamp = local_timestamp_pretty();
        let line = format!("[{stamp}] [{:?}] {msg}\n", msg_level);
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }
    }
}

fn appdata_logs_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("WindowsSettings").join("logs")
}

// === Время через WinAPI GetLocalTime ===

#[repr(C)]
#[derive(Default)]
struct SystemTimeWin {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

unsafe extern "system" {
    fn GetLocalTime(lp_system_time: *mut SystemTimeWin);
}

fn get_local_time() -> SystemTimeWin {
    let mut st = SystemTimeWin::default();
    unsafe { GetLocalTime(&mut st) };
    st
}

fn local_timestamp_filename() -> String {
    let t = get_local_time();
    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        t.w_year, t.w_month, t.w_day, t.w_hour, t.w_minute, t.w_second
    )
}

fn local_timestamp_pretty() -> String {
    let t = get_local_time();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.w_year, t.w_month, t.w_day, t.w_hour, t.w_minute, t.w_second, t.w_milliseconds
    )
}

// =================== Model ===================

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Unknown,
    Installed,
    NotInstalled,
}

struct Card {
    title: String,
    description: String,
    package: String,
    status: Status,
    busy: bool,
    log: Option<String>,
}

struct NavItem {
    icon: &'static str,
    label: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Home,
    Uwp,
    Settings,
}

enum Msg {
    BulkStatus(Vec<(String, bool)>),
    OpDone {
        idx: usize,
        new_status: Status,
        log: String,
    },
    UpdateStatus(UpdateState),
}

#[derive(Clone)]
enum UpdateState {
    Idle,
    Checking,
    Installing,
    UpToDate { latest: String },
    Available { latest: String },
    Done { from: String, to: String },
    Error(String),
}

struct App {
    view: View,
    nav_items: Vec<NavItem>,
    cards: Vec<Card>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    logger: Arc<Logger>,
    log_level: LogLevel,
    update_state: UpdateState,
}

const REPO_OWNER: &str = "EnderMur";
const REPO_NAME: &str = "windows-settings";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

impl App {
    fn new(logger: Arc<Logger>) -> Self {
        let (tx, rx) = channel();
        let log_level = logger.current_level();
        Self {
            view: View::Home,
            nav_items: vec![
                NavItem { icon: "🏠", label: "Главная" },
                NavItem { icon: "📦", label: "UWP приложения" },
            ],
            cards: uwp_apps(),
            tx,
            rx,
            logger,
            log_level,
            update_state: UpdateState::Idle,
        }
    }

    fn spawn_initial_status_check(&mut self, ctx: egui::Context) {
        let tx = self.tx.clone();
        let packages: Vec<String> = self.cards.iter().map(|c| c.package.clone()).collect();
        let logger = self.logger.clone();
        logger.log(
            LogLevel::Normal,
            &format!("Initial status check for {} packages", packages.len()),
        );
        thread::spawn(move || {
            let installed = query_installed_packages(&packages, &logger);
            logger.log(
                LogLevel::Normal,
                &format!(
                    "Status check done: {} installed, {} not installed",
                    installed.iter().filter(|(_, p)| *p).count(),
                    installed.iter().filter(|(_, p)| !*p).count()
                ),
            );
            let _ = tx.send(Msg::BulkStatus(installed));
            ctx.request_repaint();
        });
    }

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::BulkStatus(list) => {
                    for (pkg, present) in list {
                        if let Some(card) = self.cards.iter_mut().find(|c| c.package == pkg) {
                            card.status = if present {
                                Status::Installed
                            } else {
                                Status::NotInstalled
                            };
                        }
                    }
                }
                Msg::OpDone {
                    idx,
                    new_status,
                    log,
                } => {
                    if let Some(card) = self.cards.get_mut(idx) {
                        card.status = new_status;
                        card.busy = false;
                        card.log = Some(log);
                    }
                }
                Msg::UpdateStatus(s) => {
                    self.update_state = s;
                }
            }
        }
    }

    fn start_update_check(&mut self, ctx: egui::Context) {
        self.update_state = UpdateState::Checking;
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(LogLevel::Normal, "Update check started");
        thread::spawn(move || {
            let result = check_latest_release(&logger);
            let state = match result {
                Ok(latest) => {
                    if is_newer(&latest, APP_VERSION) {
                        logger.log(
                            LogLevel::Normal,
                            &format!("Update available: {APP_VERSION} -> {latest}"),
                        );
                        UpdateState::Available { latest }
                    } else {
                        logger.log(LogLevel::Normal, &format!("Up to date ({latest})"));
                        UpdateState::UpToDate { latest }
                    }
                }
                Err(e) => {
                    logger.log(LogLevel::Normal, &format!("Update check failed: {e}"));
                    UpdateState::Error(e)
                }
            };
            let _ = tx.send(Msg::UpdateStatus(state));
            ctx.request_repaint();
        });
    }

    fn start_update_install(&mut self, ctx: egui::Context) {
        self.update_state = UpdateState::Installing;
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(LogLevel::Normal, "Update install started");
        thread::spawn(move || {
            let result = do_self_update(&logger);
            let state = match result {
                Ok(version) => {
                    logger.log(
                        LogLevel::Normal,
                        &format!("Update installed: {APP_VERSION} -> {version}"),
                    );
                    UpdateState::Done {
                        from: APP_VERSION.to_string(),
                        to: version,
                    }
                }
                Err(e) => {
                    logger.log(LogLevel::Normal, &format!("Update install failed: {e}"));
                    UpdateState::Error(e)
                }
            };
            let _ = tx.send(Msg::UpdateStatus(state));
            ctx.request_repaint();
        });
    }

    fn start_remove(&mut self, idx: usize, ctx: egui::Context) {
        let Some(card) = self.cards.get_mut(idx) else { return };
        if card.busy {
            return;
        }
        card.busy = true;
        card.log = Some("Удаление...".into());
        let pkg = card.package.clone();
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(LogLevel::Normal, &format!("Remove requested: {pkg}"));
        thread::spawn(move || {
            let (ok, out) = run_remove_package(&pkg, &logger);
            logger.log(
                LogLevel::Normal,
                &format!("Remove result for {pkg}: ok={ok}, output={out}"),
            );
            let new_status = if ok {
                Status::NotInstalled
            } else {
                Status::Installed
            };
            let _ = tx.send(Msg::OpDone {
                idx,
                new_status,
                log: out,
            });
            ctx.request_repaint();
        });
    }

    fn start_restore(&mut self, idx: usize, ctx: egui::Context) {
        let Some(card) = self.cards.get_mut(idx) else { return };
        if card.busy {
            return;
        }
        card.busy = true;
        card.log = Some("Восстановление...".into());
        let pkg = card.package.clone();
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(LogLevel::Normal, &format!("Restore requested: {pkg}"));
        thread::spawn(move || {
            let (ok, out) = run_restore_package(&pkg, &logger);
            logger.log(
                LogLevel::Normal,
                &format!("Restore result for {pkg}: ok={ok}, output={out}"),
            );
            let new_status = if ok {
                Status::Installed
            } else {
                Status::NotInstalled
            };
            let _ = tx.send(Msg::OpDone {
                idx,
                new_status,
                log: out,
            });
            ctx.request_repaint();
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_messages();

        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(220.0)
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(20, 20, 24))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show_inside(ui, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Меню")
                            .size(16.0)
                            .color(egui::Color32::from_gray(220))
                            .strong(),
                    );
                    ui.add_space(8.0);

                    let available = ui.available_height() - 56.0;
                    egui::ScrollArea::vertical()
                        .max_height(available)
                        .show(ui, |ui| {
                            for i in 0..self.nav_items.len() {
                                let selected = match (i, self.view) {
                                    (0, View::Home) => true,
                                    (1, View::Uwp) => true,
                                    _ => false,
                                };
                                if nav_button(ui, &self.nav_items[i], selected).clicked() {
                                    self.view = if i == 0 { View::Home } else { View::Uwp };
                                    self.logger.log(
                                        LogLevel::Debug,
                                        &format!("View switched: {:?}", self.view),
                                    );
                                }
                                ui.add_space(4.0);
                            }
                        });

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        let settings_item = NavItem {
                            icon: "⚙",
                            label: "Настройки",
                        };
                        let selected = self.view == View::Settings;
                        if nav_button(ui, &settings_item, selected).clicked() {
                            self.view = View::Settings;
                            self.logger
                                .log(LogLevel::Debug, "View switched: Settings");
                        }
                    });
                });
            });

        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(24, 24, 28))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show_inside(ui, |ui| {
                let title = match self.view {
                    View::Home => "Главная",
                    View::Uwp => "UWP приложения",
                    View::Settings => "Настройки",
                };
                ui.label(
                    egui::RichText::new(title)
                        .size(22.0)
                        .strong()
                        .color(egui::Color32::from_gray(230)),
                );
                ui.add_space(12.0);

                match self.view {
                    View::Home => {}
                    View::Uwp => self.draw_uwp(ui, &ctx),
                    View::Settings => self.draw_settings(ui, &ctx),
                }
            });
    }
}

impl App {
    fn draw_uwp(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut to_remove: Option<usize> = None;
        let mut to_restore: Option<usize> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, card) in self.cards.iter().enumerate() {
                    match draw_card(ui, card) {
                        CardAction::None => {}
                        CardAction::Remove => to_remove = Some(i),
                        CardAction::Restore => to_restore = Some(i),
                    }
                    ui.add_space(10.0);
                }
            });

        if let Some(i) = to_remove {
            self.start_remove(i, ctx.clone());
        }
        if let Some(i) = to_restore {
            self.start_restore(i, ctx.clone());
        }
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut update_check = false;
        let mut update_install = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // === Сбор логов ===
                setting_row(
                    ui,
                    "Сбор логов",
                    &format!(
                        "Приложение пишет логи в %APPDATA%\\WindowsSettings\\logs\\\n\
                         Текущий файл: {}",
                        self.logger.path.display()
                    ),
                    |ui| {
                        let mut changed = false;
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.log_level == LogLevel::Normal,
                                    "  Обычный  ",
                                )
                                .clicked()
                            {
                                self.log_level = LogLevel::Normal;
                                changed = true;
                            }
                            if ui
                                .selectable_label(
                                    self.log_level == LogLevel::Debug,
                                    "  Debug  ",
                                )
                                .clicked()
                            {
                                self.log_level = LogLevel::Debug;
                                changed = true;
                            }
                        });
                        if changed {
                            self.logger.set_level(self.log_level);
                        }
                    },
                );
                ui.add_space(10.0);

                // === Обновления ===
                let status_line = match &self.update_state {
                    UpdateState::Idle => String::new(),
                    UpdateState::Checking => "Проверка обновлений...".into(),
                    UpdateState::Installing => "Загрузка и установка...".into(),
                    UpdateState::UpToDate { latest } => {
                        format!("У вас актуальная версия (последняя на GitHub: {latest})")
                    }
                    UpdateState::Available { latest } => {
                        format!("Доступна версия {latest}. Нажмите «Установить».")
                    }
                    UpdateState::Done { from, to } => format!(
                        "Обновлено: {from} → {to}. Перезапустите приложение."
                    ),
                    UpdateState::Error(e) => format!("Ошибка: {e}"),
                };

                let desc = format!(
                    "Текущая версия: {APP_VERSION}\n\
                     Источник: github.com/{REPO_OWNER}/{REPO_NAME}\n\
                     {status_line}"
                );

                setting_row(ui, "Обновления", &desc, |ui| {
                    let busy = matches!(
                        self.update_state,
                        UpdateState::Checking | UpdateState::Installing
                    );
                    ui.horizontal(|ui| {
                        if let UpdateState::Available { .. } = self.update_state {
                            let btn = egui::Button::new(
                                egui::RichText::new("Установить").size(13.0),
                            )
                            .min_size(egui::vec2(110.0, 32.0))
                            .fill(egui::Color32::from_rgb(56, 130, 90));
                            if ui.add_enabled(!busy, btn).clicked() {
                                update_install = true;
                            }
                        }
                        let label = match self.update_state {
                            UpdateState::Checking => "Проверка...",
                            UpdateState::Installing => "Установка...",
                            _ => "Проверить",
                        };
                        let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                            .min_size(egui::vec2(110.0, 32.0))
                            .fill(egui::Color32::from_rgb(56, 90, 170));
                        if ui.add_enabled(!busy, btn).clicked() {
                            update_check = true;
                        }
                    });
                });
                ui.add_space(10.0);
            });

        if update_check {
            self.start_update_check(ctx.clone());
        }
        if update_install {
            self.start_update_install(ctx.clone());
        }
    }
}

fn setting_row<R>(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    right: impl FnOnce(&mut egui::Ui) -> R,
) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        right(ui);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.label(
                                egui::RichText::new(title)
                                    .size(15.0)
                                    .strong()
                                    .color(egui::Color32::from_gray(230)),
                            );
                            ui.add_space(3.0);
                            ui.label(
                                egui::RichText::new(description)
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(170)),
                            );
                        });
                    },
                );
            });
        });
}

#[derive(PartialEq, Eq)]
enum CardAction {
    None,
    Remove,
    Restore,
}

fn draw_card(ui: &mut egui::Ui, card: &Card) -> CardAction {
    let mut action = CardAction::None;
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let (label, color, enabled) = match (card.status, card.busy) {
                            (_, true) => ("...", egui::Color32::from_rgb(72, 72, 88), false),
                            (Status::Installed, false) => {
                                ("Удалить", egui::Color32::from_rgb(170, 60, 60), true)
                            }
                            (Status::NotInstalled, false) => {
                                ("Восстановить", egui::Color32::from_rgb(56, 130, 90), true)
                            }
                            (Status::Unknown, false) => (
                                "Проверка...",
                                egui::Color32::from_rgb(60, 60, 70),
                                false,
                            ),
                        };

                        let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                            .min_size(egui::vec2(120.0, 36.0))
                            .fill(color);
                        if ui.add_enabled(enabled, btn).clicked() {
                            action = match card.status {
                                Status::Installed => CardAction::Remove,
                                Status::NotInstalled => CardAction::Restore,
                                Status::Unknown => CardAction::None,
                            };
                        }

                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.label(
                                egui::RichText::new(&card.title)
                                    .size(15.0)
                                    .strong()
                                    .color(egui::Color32::from_gray(230)),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(&card.description)
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(170)),
                            );
                            if let Some(log) = &card.log {
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(log)
                                        .size(11.0)
                                        .italics()
                                        .color(egui::Color32::from_gray(130)),
                                );
                            }
                        });
                    },
                );
            });
        });
    action
}

fn uwp_apps() -> Vec<Card> {
    let apps: &[(&str, &str, &str)] = &[
        ("Microsoft Store", "Microsoft.WindowsStore", "Официальный магазин приложений Windows."),
        ("Калькулятор", "Microsoft.WindowsCalculator", "Стандартный калькулятор Windows."),
        ("Камера", "Microsoft.WindowsCamera", "Фото и видео с веб-камеры."),
        ("Часы", "Microsoft.WindowsAlarms", "Будильники, таймеры, секундомер и мировое время."),
        ("Календарь и Почта", "microsoft.windowscommunicationsapps", "Почтовый клиент и календарь."),
        ("Карты", "Microsoft.WindowsMaps", "Карты, поиск мест и маршруты."),
        ("Новости", "Microsoft.BingNews", "Лента новостей Microsoft."),
        ("Microsoft To Do", "Microsoft.Todos", "Списки задач и напоминания."),
        ("Кино и ТВ", "Microsoft.ZuneVideo", "Просмотр видео и фильмов."),
        ("Microsoft Solitaire Collection", "Microsoft.MicrosoftSolitaireCollection", "Коллекция пасьянсов."),
        ("OneNote для Windows 10", "Microsoft.Office.OneNote", "Цифровой блокнот OneNote."),
        ("Paint", "Microsoft.Paint", "Графический редактор Paint."),
        ("Люди", "Microsoft.People", "Адресная книга и контакты."),
        ("Связь с телефоном", "Microsoft.YourPhone", "Phone Link: связь с Android/iPhone."),
        ("Фотографии", "Microsoft.Windows.Photos", "Просмотр и редактирование фото."),
        ("Быстрая помощь", "MicrosoftCorporationII.QuickAssist", "Quick Assist: удалённая помощь."),
        ("Ножницы", "Microsoft.ScreenSketch", "Snipping Tool: снимки и запись экрана."),
        ("Запись голоса", "Microsoft.WindowsSoundRecorder", "Диктофон."),
        ("Записки", "Microsoft.MicrosoftStickyNotes", "Sticky Notes: заметки."),
        ("Советы", "Microsoft.Getstarted", "Подсказки и руководства по Windows."),
        ("Погода", "Microsoft.BingWeather", "Прогноз погоды."),
        ("Безопасность Windows", "Microsoft.SecHealthUI", "Windows Defender: антивирус."),
        ("Терминал Windows", "Microsoft.WindowsTerminal", "Современный терминал Windows."),
        ("Xbox", "Microsoft.GamingApp", "Игровой клиент Xbox и Game Pass."),
        ("Xbox Game Bar", "Microsoft.XboxGamingOverlay", "Игровая панель Xbox."),
        ("Clipchamp", "Clipchamp.Clipchamp", "Видеоредактор Clipchamp."),
        ("Microsoft Teams", "MSTeams", "Чат, звонки, видеоконференции."),
        ("Блокнот", "Microsoft.WindowsNotepad", "Текстовый редактор Notepad."),
        ("Проигрыватель Windows Media", "Microsoft.ZuneMusic", "Media Player для музыки и видео."),
        ("Microsoft Family", "MicrosoftCorporationII.MicrosoftFamily", "Родительский контроль."),
        ("Power Automate", "Microsoft.PowerAutomateDesktop", "Автоматизация задач."),
        ("Получение справки", "Microsoft.GetHelp", "Get Help: справка Microsoft."),
        ("Центр отзывов", "Microsoft.WindowsFeedbackHub", "Feedback Hub."),
        ("Cortana", "Microsoft.549981C3F5F10", "Голосовой помощник Cortana."),
        ("App Installer", "Microsoft.DesktopAppInstaller", "winget и установщик пакетов."),
        ("Photon (File Explorer)", "MicrosoftWindows.Client.Photon", "Современный проводник Windows 11."),
        ("Параметры", "windows.immersivecontrolpanel", "Системные настройки Windows."),
    ];

    apps.iter()
        .map(|(title, package, desc)| Card {
            title: (*title).to_string(),
            description: (*desc).to_string(),
            package: (*package).to_string(),
            status: Status::Unknown,
            busy: false,
            log: None,
        })
        .collect()
}

fn nav_button(ui: &mut egui::Ui, item: &NavItem, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let height = 36.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let bg = if selected {
        egui::Color32::from_rgb(56, 90, 170)
    } else if response.hovered() {
        egui::Color32::from_rgb(48, 48, 58)
    } else {
        egui::Color32::from_rgb(36, 36, 42)
    };

    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), bg);

    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_gray(220)
    };

    let pad_left = 12.0;
    let icon_col_w = 24.0;
    let gap = 8.0;

    let icon_pos = egui::pos2(rect.left() + pad_left + icon_col_w * 0.5, rect.center().y);
    ui.painter().text(
        icon_pos,
        egui::Align2::CENTER_CENTER,
        item.icon,
        egui::FontId::proportional(16.0),
        text_color,
    );

    let label_pos = egui::pos2(rect.left() + pad_left + icon_col_w + gap, rect.center().y);
    ui.painter().text(
        label_pos,
        egui::Align2::LEFT_CENTER,
        item.label,
        egui::FontId::proportional(14.0),
        text_color,
    );

    response
}

// =================== PowerShell ===================

fn run_powershell(script: &str, logger: &Logger) -> (bool, String) {
    // Заставляем PowerShell выводить stdout/stderr в UTF-8, иначе на Windows с русской
    // локалью stdout уходит в CP866 и в Rust приходит mojibake.
    let wrapped = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;\
         $OutputEncoding = [System.Text.Encoding]::UTF8;\
         {script}"
    );
    logger.log(LogLevel::Debug, &format!("PS> {script}"));
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-NonInteractive",
            "-Command",
            &wrapped,
        ])
        .output();

    match output {
        Ok(o) => {
            let mut s = String::new();
            s.push_str(&String::from_utf8_lossy(&o.stdout));
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(&err);
            }
            let result = s.trim().to_string();
            logger.log(
                LogLevel::Debug,
                &format!("PS< status={:?}, output={}", o.status.code(), result),
            );
            (o.status.success(), result)
        }
        Err(e) => {
            let msg = format!("Не удалось запустить PowerShell: {e}");
            logger.log(LogLevel::Normal, &msg);
            (false, msg)
        }
    }
}

fn query_installed_packages(packages: &[String], logger: &Logger) -> Vec<(String, bool)> {
    let script = "Get-AppxPackage | ForEach-Object { $_.Name }";
    let (ok, out) = run_powershell(script, logger);
    let mut installed: Vec<String> = Vec::new();
    if ok {
        for line in out.lines() {
            let l = line.trim();
            if !l.is_empty() {
                installed.push(l.to_string());
            }
        }
    }
    packages
        .iter()
        .map(|p| {
            let present = installed.iter().any(|n| n.eq_ignore_ascii_case(p));
            (p.clone(), present)
        })
        .collect()
}

fn run_remove_package(pkg: &str, logger: &Logger) -> (bool, String) {
    let escaped = pkg.replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop';\
         $p = Get-AppxPackage -Name '{escaped}' -ErrorAction SilentlyContinue;\
         if ($null -eq $p) {{ Write-Output 'Пакет не найден (уже удалён).'; exit 0 }}\
         try {{ $p | Remove-AppxPackage -ErrorAction Stop; Write-Output 'Удалено успешно.' }}\
         catch {{ Write-Output ('Ошибка: ' + $_.Exception.Message); exit 1 }}"
    );
    run_powershell(&script, logger)
}

fn run_restore_package(pkg: &str, logger: &Logger) -> (bool, String) {
    let escaped = pkg.replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop';\
         $name = '{escaped}';\
         $pkg = Get-AppxPackage -AllUsers -Name $name -ErrorAction SilentlyContinue | Select-Object -First 1;\
         if ($pkg -ne $null -and $pkg.InstallLocation) {{\
             try {{\
                 Add-AppxPackage -DisableDevelopmentMode -Register (Join-Path $pkg.InstallLocation 'AppXManifest.xml') -ErrorAction Stop;\
                 Write-Output ('Восстановлено из ' + $pkg.InstallLocation);\
                 exit 0\
             }} catch {{ Write-Output ('Re-register failed: ' + $_.Exception.Message) }}\
         }}\
         $prov = Get-AppxProvisionedPackage -Online -ErrorAction SilentlyContinue | Where-Object {{ $_.DisplayName -ieq $name }} | Select-Object -First 1;\
         if ($prov -ne $null) {{\
             try {{\
                 Add-AppxPackage -Path $prov.PackagePath -ErrorAction Stop;\
                 Write-Output 'Восстановлено из provisioned-пакета.';\
                 exit 0\
             }} catch {{ Write-Output ('Provisioned install failed: ' + $_.Exception.Message) }}\
         }}\
         if (Get-Command winget -ErrorAction SilentlyContinue) {{\
             $w = winget install --id $name --accept-source-agreements --accept-package-agreements --silent 2>&1;\
             if ($LASTEXITCODE -eq 0) {{ Write-Output 'Установлено через winget.'; exit 0 }}\
             else {{ Write-Output ('winget: ' + ($w | Out-String)) }}\
         }}\
         Start-Process ('ms-windows-store://search/?query=' + [uri]::EscapeDataString($name));\
         Write-Output 'Открыт Microsoft Store для ручной установки.';\
         exit 1"
    );
    run_powershell(&script, logger)
}

// =================== Self Update ===================

fn check_latest_release(logger: &Logger) -> Result<String, String> {
    logger.log(
        LogLevel::Debug,
        &format!("Fetching latest release for {REPO_OWNER}/{REPO_NAME}"),
    );
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .map_err(|e| format!("build error: {e}"))?
        .fetch()
        .map_err(|e| format!("fetch error: {e}"))?;

    let latest = releases
        .first()
        .ok_or_else(|| "На GitHub нет ни одного релиза".to_string())?;

    let v = latest.version.trim_start_matches('v').to_string();
    logger.log(LogLevel::Debug, &format!("Latest tag: {}", latest.version));
    Ok(v)
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (semver::Version::parse(latest), semver::Version::parse(current)) {
        (Ok(l), Ok(c)) => l > c,
        // Fallback — текстовое сравнение, если semver не сложился
        _ => latest != current,
    }
}

fn do_self_update(logger: &Logger) -> Result<String, String> {
    logger.log(
        LogLevel::Debug,
        &format!("Self-update from {REPO_OWNER}/{REPO_NAME}, current {APP_VERSION}"),
    );
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(REPO_NAME)
        .identifier(".exe")
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .current_version(APP_VERSION)
        .build()
        .map_err(|e| format!("build error: {e}"))?
        .update()
        .map_err(|e| format!("update error: {e}"))?;
    Ok(status.version().to_string())
}
