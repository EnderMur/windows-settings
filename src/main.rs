#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::os::windows::process::CommandExt;
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

    if let Ok(bytes) = fs::read("C:/Windows/Fonts/segoeui.ttf") {
        fonts.font_data.insert(
            "segoe_ui".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.insert(0, "segoe_ui".to_owned());
        }
    }

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LogLevel {
    Normal,
    Debug,
}

struct Logger {
    file: Mutex<Option<File>>,
    path: PathBuf,
    level: AtomicU8,
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
    appdata_dir().join("logs")
}

fn appdata_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("WindowsSettings")
}

fn appdata_config_path() -> PathBuf {
    appdata_dir().join("config.json")
}

fn appdata_settings_path() -> PathBuf {
    appdata_dir().join("settings.conf")
}

#[derive(Clone, Default)]
struct Config {
    github_token: Option<String>,
}

fn load_config(logger: &Logger) -> Config {
    let path = appdata_config_path();
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            logger.log(LogLevel::Debug, "Config file not found, using defaults");
            return Config::default();
        }
        Err(e) => {
            logger.log(LogLevel::Normal, &format!("Failed to read config: {e}"));
            return Config::default();
        }
    };
    let cfg = parse_config(&content);
    logger.log(
        LogLevel::Debug,
        &format!(
            "Config loaded from {}: github_token_present={}",
            path.display(),
            cfg.github_token.is_some()
        ),
    );
    cfg
}

fn save_config(cfg: &Config, logger: &Logger) -> Result<(), String> {
    let path = appdata_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("создать каталог: {e}"))?;
    }
    let content = match &cfg.github_token {
        Some(t) => format!(
            "{{\n  \"github_token\": \"{}\"\n}}\n",
            json_escape(t)
        ),
        None => "{}\n".to_string(),
    };
    fs::write(&path, content).map_err(|e| format!("записать файл: {e}"))?;
    logger.log(
        LogLevel::Normal,
        &format!(
            "Config saved to {}: github_token_present={}",
            path.display(),
            cfg.github_token.is_some()
        ),
    );
    Ok(())
}

fn parse_config(content: &str) -> Config {
    let mut cfg = Config::default();
    if let Some(token) = extract_json_string(content, "github_token") {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            cfg.github_token = Some(trimmed.to_string());
        }
    }
    cfg
}

fn extract_json_string(content: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let pos = content.find(&pattern)?;
    let after = &content[pos + pattern.len()..];
    let colon = after.find(':')?;
    let after_colon = &after[colon + 1..];
    let quote_pos = after_colon.find('"')?;
    let value_start = &after_colon[quote_pos + 1..];

    let mut out = String::new();
    let mut chars = value_start.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000C}'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 {
                        return None;
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    if let Some(c) = char::from_u32(code) {
                        out.push(c);
                    }
                }
                other => out.push(other),
            },
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[derive(Clone)]
struct AppSettings {
    log_level: LogLevel,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            log_level: LogLevel::Normal,
        }
    }
}

fn log_level_to_str(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Normal => "normal",
        LogLevel::Debug => "debug",
    }
}

fn log_level_from_str(s: &str) -> Option<LogLevel> {
    match s.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(LogLevel::Normal),
        "debug" => Some(LogLevel::Debug),
        _ => None,
    }
}

fn load_settings(logger: &Logger) -> AppSettings {
    let path = appdata_settings_path();
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            logger.log(LogLevel::Debug, "Settings file not found, using defaults");
            return AppSettings::default();
        }
        Err(e) => {
            logger.log(LogLevel::Normal, &format!("Failed to read settings: {e}"));
            return AppSettings::default();
        }
    };
    let settings = parse_settings(&content);
    logger.log(
        LogLevel::Debug,
        &format!(
            "Settings loaded from {}: log_level={}",
            path.display(),
            log_level_to_str(settings.log_level)
        ),
    );
    settings
}

fn save_settings(settings: &AppSettings, logger: &Logger) -> Result<(), String> {
    let path = appdata_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("создать каталог: {e}"))?;
    }
    let content = format!(
        "# Windows Settings — пользовательские настройки\n\
         # Формат: key=value, одна пара на строку.\n\
         log_level={}\n",
        log_level_to_str(settings.log_level)
    );
    fs::write(&path, content).map_err(|e| format!("записать файл: {e}"))?;
    logger.log(
        LogLevel::Normal,
        &format!(
            "Settings saved to {}: log_level={}",
            path.display(),
            log_level_to_str(settings.log_level)
        ),
    );
    Ok(())
}

fn parse_settings(content: &str) -> AppSettings {
    let mut settings = AppSettings::default();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "log_level" => {
                if let Some(level) = log_level_from_str(value) {
                    settings.log_level = level;
                }
            }
            _ => {}
        }
    }
    settings
}

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
    beta: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Home,
    Uwp,
    Telemetry,
    Memory,
    Cleanup,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum CleanupId {
    RecycleBin,
    UserTemp,
    SystemTemp,
    CrashDumps,
    WerReports,
    MinidumpAndLkr,
    SoftwareDistribution,
    Catroot2,
    DeliveryOptimization,
    WindowsOld,
    UpgradeLeftovers,
    LastGood,
    Prefetch,
    FontCache,
    IconCache,
    ThumbnailCache,
    DnsCache,
    StoreCache,
    SearchCache,
    CbsDismLogs,
    PrintQueue,
    RecentFiles,
    EdgeCache,
    ChromeCache,
    FirefoxCache,
    WinSxSComponentCleanup,
    OldRestorePoints,
    HiberfilOff,
}

impl CleanupId {
    fn key(self) -> &'static str {
        match self {
            CleanupId::RecycleBin => "recyclebin",
            CleanupId::UserTemp => "usertemp",
            CleanupId::SystemTemp => "systemtemp",
            CleanupId::CrashDumps => "crashdumps",
            CleanupId::WerReports => "wer",
            CleanupId::MinidumpAndLkr => "minidump",
            CleanupId::SoftwareDistribution => "wuadl",
            CleanupId::Catroot2 => "catroot2",
            CleanupId::DeliveryOptimization => "deliveryopt",
            CleanupId::WindowsOld => "windowsold",
            CleanupId::UpgradeLeftovers => "upgradeleft",
            CleanupId::LastGood => "lastgood",
            CleanupId::Prefetch => "prefetch",
            CleanupId::FontCache => "fontcache",
            CleanupId::IconCache => "iconcache",
            CleanupId::ThumbnailCache => "thumbcache",
            CleanupId::DnsCache => "dnscache",
            CleanupId::StoreCache => "storecache",
            CleanupId::SearchCache => "searchcache",
            CleanupId::CbsDismLogs => "cbsdism",
            CleanupId::PrintQueue => "printq",
            CleanupId::RecentFiles => "recent",
            CleanupId::EdgeCache => "edgecache",
            CleanupId::ChromeCache => "chromecache",
            CleanupId::FirefoxCache => "firefoxcache",
            CleanupId::WinSxSComponentCleanup => "winsxs",
            CleanupId::OldRestorePoints => "restorepts",
            CleanupId::HiberfilOff => "hiberfil",
        }
    }

    fn from_key(s: &str) -> Option<Self> {
        let all = [
            CleanupId::RecycleBin,
            CleanupId::UserTemp,
            CleanupId::SystemTemp,
            CleanupId::CrashDumps,
            CleanupId::WerReports,
            CleanupId::MinidumpAndLkr,
            CleanupId::SoftwareDistribution,
            CleanupId::Catroot2,
            CleanupId::DeliveryOptimization,
            CleanupId::WindowsOld,
            CleanupId::UpgradeLeftovers,
            CleanupId::LastGood,
            CleanupId::Prefetch,
            CleanupId::FontCache,
            CleanupId::IconCache,
            CleanupId::ThumbnailCache,
            CleanupId::DnsCache,
            CleanupId::StoreCache,
            CleanupId::SearchCache,
            CleanupId::CbsDismLogs,
            CleanupId::PrintQueue,
            CleanupId::RecentFiles,
            CleanupId::EdgeCache,
            CleanupId::ChromeCache,
            CleanupId::FirefoxCache,
            CleanupId::WinSxSComponentCleanup,
            CleanupId::OldRestorePoints,
            CleanupId::HiberfilOff,
        ];
        all.into_iter().find(|c| c.key() == s)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CleanupSize {
    Unknown,
    NotApplicable,
    Bytes(u64),
}

struct CleanupItem {
    id: CleanupId,
    title: String,
    description: String,
    size: CleanupSize,
    danger: bool,
    busy: bool,
    log: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TelemetryStatus {
    Unknown,
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum TelemetryId {
    Office,
    Firefox,
    Chrome,
    Nvidia,
    VisualStudio,
    Windows,
}

impl TelemetryId {
    fn from_key(s: &str) -> Option<Self> {
        match s {
            "office" => Some(TelemetryId::Office),
            "firefox" => Some(TelemetryId::Firefox),
            "chrome" => Some(TelemetryId::Chrome),
            "nvidia" => Some(TelemetryId::Nvidia),
            "vs" => Some(TelemetryId::VisualStudio),
            "windows" => Some(TelemetryId::Windows),
            _ => None,
        }
    }
}

struct TelemetryItem {
    id: TelemetryId,
    title: String,
    description: String,
    status: TelemetryStatus,
    busy: bool,
    log: Option<String>,
}

#[derive(Clone, Default)]
struct SysInfo {
    os: String,
    build: String,
    arch: String,
    hostname: String,
    user: String,
    is_admin: Option<bool>,
    cpu: String,
    gpu: String,
    ram_gb: String,
}

enum Msg {
    BulkStatus(Vec<(String, bool)>),
    OpDone {
        idx: usize,
        new_status: Status,
        log: String,
    },
    TelemetryBulkStatus(Vec<(TelemetryId, TelemetryStatus)>),
    TelemetryOpDone {
        id: TelemetryId,
        new_status: TelemetryStatus,
        log: String,
    },
    UpdateStatus(UpdateState),
    SysInfoReady(SysInfo),
    MemInfoReady(MemInfo),
    MemOpDone { id: MemOp, log: String },
    CleanupSizesReady(Vec<(CleanupId, CleanupSize)>),
    CleanupOpDone {
        id: CleanupId,
        new_size: CleanupSize,
        log: String,
    },
}

#[derive(Clone, Copy, Default, Debug)]
struct MemInfo {
    total_bytes: u64,
    avail_bytes: u64,
    standby_bytes: u64,
    modified_bytes: u64,
    free_bytes: u64,
    memory_load: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MemOp {
    PurgeStandby,
    PurgeLowPriorityStandby,
    EmptyWorkingSets,
    FlushModified,
}

impl MemOp {
    fn command(self) -> i32 {

        match self {
            MemOp::EmptyWorkingSets => 2,
            MemOp::FlushModified => 3,
            MemOp::PurgeStandby => 4,
            MemOp::PurgeLowPriorityStandby => 5,
        }
    }

    fn title(self) -> &'static str {
        match self {
            MemOp::PurgeStandby => "Очистить ожидающую память",
            MemOp::PurgeLowPriorityStandby => "Очистить low-priority standby",
            MemOp::EmptyWorkingSets => "Сбросить рабочие наборы",
            MemOp::FlushModified => "Сбросить изменённые страницы",
        }
    }

    fn description(self) -> &'static str {
        match self {
            MemOp::PurgeStandby => {
                "Полная очистка standby-кеша Windows. Аналог Empty Standby List."
            }
            MemOp::PurgeLowPriorityStandby => {
                "Очищает только низкоприоритетную часть standby-кеша. \
                 Меньшее влияние на производительность."
            }
            MemOp::EmptyWorkingSets => {
                "Сбрасывает рабочие наборы всех процессов. Свободная память возрастёт, \
                 но запущенные программы могут на короткое время «подтормозить»."
            }
            MemOp::FlushModified => {
                "Сбрасывает изменённые страницы на диск/в standby-список."
            }
        }
    }
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
    telemetry: Vec<TelemetryItem>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    logger: Arc<Logger>,
    settings: AppSettings,
    update_state: UpdateState,
    sys_info: Option<SysInfo>,
    config: Config,
    show_token_dialog: bool,
    token_input: String,
    token_dialog_error: Option<String>,
    mem_info: Option<MemInfo>,
    mem_busy: bool,
    mem_log: Option<String>,
    mem_refresh_in_flight: bool,
    mem_last_refresh: Option<std::time::Instant>,
    cleanup_items: Vec<CleanupItem>,
    cleanup_refresh_in_flight: bool,
    cleanup_sizes_loaded: bool,
}

const REPO_OWNER: &str = "EnderMur";
const REPO_NAME: &str = "windows-settings";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

impl App {
    fn new(logger: Arc<Logger>) -> Self {
        let (tx, rx) = channel();
        let config = load_config(&logger);
        let settings = load_settings(&logger);
        logger.set_level(settings.log_level);
        Self {
            view: View::Home,
            nav_items: vec![
                NavItem { icon: "🏠", label: "Главная", beta: false },
                NavItem { icon: "📦", label: "UWP приложения", beta: false },
                NavItem { icon: "🛡", label: "Телеметрия", beta: false },
                NavItem { icon: "🧠", label: "ОЗУ", beta: true },
                NavItem { icon: "🧹", label: "Очистка", beta: true },
            ],
            cards: uwp_apps(),
            telemetry: telemetry_items(),
            cleanup_items: cleanup_items(),
            cleanup_refresh_in_flight: false,
            cleanup_sizes_loaded: false,
            tx,
            rx,
            logger,
            settings,
            update_state: UpdateState::Idle,
            sys_info: None,
            config,
            show_token_dialog: false,
            token_input: String::new(),
            token_dialog_error: None,
            mem_info: None,
            mem_busy: false,
            mem_log: None,
            mem_refresh_in_flight: false,
            mem_last_refresh: None,
        }
    }

    fn spawn_initial_status_check(&mut self, ctx: egui::Context) {
        let tx = self.tx.clone();
        let packages: Vec<String> = self.cards.iter().map(|c| c.package.clone()).collect();
        let logger = self.logger.clone();
        let ctx1 = ctx.clone();
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
            ctx1.request_repaint();
        });

        let tx = self.tx.clone();
        let logger = self.logger.clone();
        let ctx2 = ctx.clone();
        logger.log(LogLevel::Normal, "Initial telemetry status check");
        thread::spawn(move || {
            let statuses = query_telemetry_status(&logger);
            logger.log(
                LogLevel::Normal,
                &format!("Telemetry status check done: {} entries", statuses.len()),
            );
            let _ = tx.send(Msg::TelemetryBulkStatus(statuses));
            ctx2.request_repaint();
        });

        let tx = self.tx.clone();
        let logger = self.logger.clone();
        let ctx3 = ctx.clone();
        logger.log(LogLevel::Normal, "Initial system info collection");
        thread::spawn(move || {
            let info = collect_sys_info(&logger);
            logger.log(
                LogLevel::Normal,
                &format!(
                    "System info collected: os='{}', build='{}', cpu='{}', gpu='{}'",
                    info.os, info.build, info.cpu, info.gpu
                ),
            );
            let _ = tx.send(Msg::SysInfoReady(info));
            ctx3.request_repaint();
        });

        self.start_update_check(ctx);
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
                Msg::TelemetryBulkStatus(list) => {
                    for (id, status) in list {
                        if let Some(item) =
                            self.telemetry.iter_mut().find(|t| t.id == id)
                        {
                            item.status = status;
                        }
                    }
                }
                Msg::TelemetryOpDone {
                    id,
                    new_status,
                    log,
                } => {
                    if let Some(item) = self.telemetry.iter_mut().find(|t| t.id == id) {
                        item.status = new_status;
                        item.busy = false;
                        item.log = Some(log);
                    }
                }
                Msg::UpdateStatus(s) => {
                    self.update_state = s;
                }
                Msg::SysInfoReady(info) => {
                    self.sys_info = Some(info);
                }
                Msg::MemInfoReady(info) => {
                    self.mem_info = Some(info);
                    self.mem_refresh_in_flight = false;
                    self.mem_last_refresh = Some(std::time::Instant::now());
                }
                Msg::MemOpDone { id, log } => {
                    self.mem_busy = false;
                    self.mem_log = Some(format!("{}: {}", id.title(), log));
                }
                Msg::CleanupSizesReady(list) => {
                    for (id, size) in list {
                        if let Some(item) =
                            self.cleanup_items.iter_mut().find(|c| c.id == id)
                        {
                            item.size = size;
                        }
                    }
                    self.cleanup_refresh_in_flight = false;
                    self.cleanup_sizes_loaded = true;
                }
                Msg::CleanupOpDone { id, new_size, log } => {
                    if let Some(item) =
                        self.cleanup_items.iter_mut().find(|c| c.id == id)
                    {
                        item.busy = false;
                        item.log = Some(log);
                        item.size = new_size;
                    }
                }
            }
        }
    }

    fn start_update_check(&mut self, ctx: egui::Context) {
        self.update_state = UpdateState::Checking;
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        let token = self.config.github_token.clone();
        logger.log(
            LogLevel::Normal,
            &format!(
                "Update check started (auth={})",
                if token.is_some() { "token" } else { "anonymous" }
            ),
        );
        thread::spawn(move || {
            let result = check_latest_release(&logger, token.as_deref());
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
        let token = self.config.github_token.clone();
        logger.log(
            LogLevel::Normal,
            &format!(
                "Update install started (auth={})",
                if token.is_some() { "token" } else { "anonymous" }
            ),
        );
        thread::spawn(move || {
            let result = do_self_update(&logger, token.as_deref());
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

    fn start_mem_refresh(&mut self, ctx: egui::Context) {
        if self.mem_refresh_in_flight {
            return;
        }
        self.mem_refresh_in_flight = true;
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        thread::spawn(move || {
            let info = collect_mem_info(&logger);
            let _ = tx.send(Msg::MemInfoReady(info));
            ctx.request_repaint();
        });
    }

    fn start_mem_op(&mut self, id: MemOp, ctx: egui::Context) {
        if self.mem_busy {
            return;
        }
        self.mem_busy = true;
        self.mem_log = Some(format!("{}: выполняется...", id.title()));
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(
            LogLevel::Normal,
            &format!("Memory op requested: {:?}", id),
        );
        thread::spawn(move || {
            let (ok, out) = run_mem_op(id, &logger);
            logger.log(
                LogLevel::Normal,
                &format!("Memory op {:?} result: ok={ok}, output={out}", id),
            );
            let log = if ok {
                format!("успешно. {out}")
            } else {
                format!("ошибка. {out}")
            };
            let _ = tx.send(Msg::MemOpDone { id, log });

            let info = collect_mem_info(&logger);
            let _ = tx.send(Msg::MemInfoReady(info));
            ctx.request_repaint();
        });
    }

    fn start_telemetry_op(&mut self, id: TelemetryId, disable: bool, ctx: egui::Context) {
        let Some(item) = self.telemetry.iter_mut().find(|t| t.id == id) else { return };
        if item.busy {
            return;
        }
        item.busy = true;
        item.log = Some(if disable {
            "Отключение...".into()
        } else {
            "Включение...".into()
        });
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(
            LogLevel::Normal,
            &format!(
                "Telemetry {} requested: {:?}",
                if disable { "disable" } else { "enable" },
                id
            ),
        );
        thread::spawn(move || {
            let (ok, out) = run_telemetry_op(id, disable, &logger);
            logger.log(
                LogLevel::Normal,
                &format!(
                    "Telemetry {} result for {:?}: ok={ok}, output={out}",
                    if disable { "disable" } else { "enable" },
                    id
                ),
            );
            let new_status = if ok {
                if disable {
                    TelemetryStatus::Disabled
                } else {
                    TelemetryStatus::Enabled
                }
            } else if disable {
                TelemetryStatus::Enabled
            } else {
                TelemetryStatus::Disabled
            };
            let _ = tx.send(Msg::TelemetryOpDone {
                id,
                new_status,
                log: out,
            });
            ctx.request_repaint();
        });
    }

    fn start_cleanup_refresh(&mut self, ctx: egui::Context) {
        if self.cleanup_refresh_in_flight {
            return;
        }
        self.cleanup_refresh_in_flight = true;
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(LogLevel::Normal, "Cleanup sizes refresh started");
        thread::spawn(move || {
            let sizes = query_cleanup_sizes(&logger);
            logger.log(
                LogLevel::Normal,
                &format!("Cleanup sizes refresh done: {} entries", sizes.len()),
            );
            let _ = tx.send(Msg::CleanupSizesReady(sizes));
            ctx.request_repaint();
        });
    }

    fn start_cleanup_op(&mut self, id: CleanupId, ctx: egui::Context) {
        let Some(item) = self.cleanup_items.iter_mut().find(|c| c.id == id) else {
            return;
        };
        if item.busy {
            return;
        }
        item.busy = true;
        item.log = Some("Очистка...".into());
        let tx = self.tx.clone();
        let logger = self.logger.clone();
        logger.log(
            LogLevel::Normal,
            &format!("Cleanup op requested: {:?}", id),
        );
        thread::spawn(move || {
            let (ok, out) = run_cleanup_op(id, &logger);
            logger.log(
                LogLevel::Normal,
                &format!("Cleanup op {:?} result: ok={ok}, output={out}", id),
            );
            // пересчитать размер именно этой категории
            let sizes = query_cleanup_sizes(&logger);
            let new_size = sizes
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, s)| *s)
                .unwrap_or(CleanupSize::Unknown);
            let log = if ok {
                if out.trim().is_empty() {
                    "Готово.".to_string()
                } else {
                    out
                }
            } else {
                format!("Ошибка: {out}")
            };
            let _ = tx.send(Msg::CleanupOpDone {
                id,
                new_size,
                log,
            });
            // также шлём обновлённые размеры всех категорий, чтобы UI был актуален
            let _ = tx.send(Msg::CleanupSizesReady(sizes));
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
                                let target = match i {
                                    0 => View::Home,
                                    1 => View::Uwp,
                                    2 => View::Telemetry,
                                    3 => View::Memory,
                                    4 => View::Cleanup,
                                    _ => View::Home,
                                };
                                let selected = self.view == target;
                                if nav_button(ui, &self.nav_items[i], selected).clicked() {
                                    self.view = target;
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
                            beta: false,
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
                    View::Telemetry => "Телеметрия",
                    View::Memory => "ОЗУ",
                    View::Cleanup => "Очистка",
                    View::Settings => "Настройки",
                };
                let beta_view = matches!(self.view, View::Memory | View::Cleanup);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .size(22.0)
                            .strong()
                            .color(egui::Color32::from_gray(230)),
                    );
                    if beta_view {
                        draw_beta_badge(ui, 12.0);
                    }
                });
                ui.add_space(12.0);

                match self.view {
                    View::Home => self.draw_home(ui, &ctx),
                    View::Uwp => self.draw_uwp(ui, &ctx),
                    View::Telemetry => self.draw_telemetry(ui, &ctx),
                    View::Memory => self.draw_memory(ui, &ctx),
                    View::Cleanup => self.draw_cleanup(ui, &ctx),
                    View::Settings => self.draw_settings(ui, &ctx),
                }
            });

        if self.show_token_dialog {
            self.draw_token_dialog(&ctx);
        }
    }
}

impl App {
    fn draw_home(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut update_install = false;
        let mut update_check = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {

                info_card(ui, "О системе", |ui| {
                    if let Some(info) = &self.sys_info {
                        let os_line = if info.build.is_empty() {
                            info.os.clone()
                        } else {
                            format!("{} (build {})", info.os, info.build)
                        };
                        let os_line = if info.arch.is_empty() {
                            os_line
                        } else {
                            format!("{}, {}", os_line, info.arch)
                        };

                        let user_line = match info.is_admin {
                            Some(true) => format!("{} (администратор)", info.user),
                            Some(false) => format!("{} (без прав администратора)", info.user),
                            None => info.user.clone(),
                        };

                        let ram_line = if info.ram_gb.is_empty() {
                            "—".to_string()
                        } else {
                            format!("{} ГБ", info.ram_gb)
                        };

                        info_row(ui, "ОС", &os_line);
                        info_row(ui, "Имя ПК", &info.hostname);
                        info_row(ui, "Пользователь", &user_line);
                        info_row(ui, "Процессор", &info.cpu);
                        info_row(ui, "Видеокарта", &info.gpu);
                        info_row(ui, "ОЗУ", &ram_line);
                    } else {
                        ui.label(
                            egui::RichText::new("Собираем сведения о системе...")
                                .size(13.0)
                                .italics()
                                .color(egui::Color32::from_gray(170)),
                        );
                    }
                });
                ui.add_space(10.0);

                info_card(ui, "Приложение", |ui| {
                    info_row(ui, "Версия", APP_VERSION);
                    info_row(
                        ui,
                        "Репозиторий",
                        &format!("github.com/{REPO_OWNER}/{REPO_NAME}"),
                    );

                    let (status_text, status_color) = match &self.update_state {
                        UpdateState::Idle => (
                            "Готов к проверке обновлений.".to_string(),
                            egui::Color32::from_gray(170),
                        ),
                        UpdateState::Checking => (
                            "Проверка обновлений...".to_string(),
                            egui::Color32::from_gray(170),
                        ),
                        UpdateState::Installing => (
                            "Загрузка и установка обновления...".to_string(),
                            egui::Color32::from_gray(170),
                        ),
                        UpdateState::UpToDate { latest } => (
                            format!("У вас актуальная версия (последняя на GitHub: {latest})."),
                            egui::Color32::from_rgb(120, 200, 140),
                        ),
                        UpdateState::Available { latest } => (
                            format!("Доступна версия {latest}."),
                            egui::Color32::from_rgb(220, 180, 100),
                        ),
                        UpdateState::Done { from, to } => (
                            format!("Обновлено: {from} → {to}. Перезапустите приложение."),
                            egui::Color32::from_rgb(120, 200, 140),
                        ),
                        UpdateState::Error(e) => (
                            e.clone(),
                            egui::Color32::from_rgb(220, 120, 120),
                        ),
                    };

                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(status_text)
                            .size(13.0)
                            .color(status_color),
                    );

                    let busy = matches!(
                        self.update_state,
                        UpdateState::Checking | UpdateState::Installing
                    );

                    if self.config.github_token.is_some() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Используется сохранённый GitHub-токен.",
                            )
                            .size(11.0)
                            .italics()
                            .color(egui::Color32::from_gray(140)),
                        );
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if let UpdateState::Available { .. } = self.update_state {
                            let btn = egui::Button::new(
                                egui::RichText::new("Обновить").size(13.0),
                            )
                            .min_size(egui::vec2(120.0, 32.0))
                            .fill(egui::Color32::from_rgb(56, 130, 90));
                            if ui.add_enabled(!busy, btn).clicked() {
                                update_install = true;
                            }
                        }
                        let btn = egui::Button::new(
                            egui::RichText::new("Проверить обновления").size(13.0),
                        )
                        .min_size(egui::vec2(170.0, 32.0))
                        .fill(egui::Color32::from_rgb(56, 90, 170));
                        if ui.add_enabled(!busy, btn).clicked() {
                            update_check = true;
                        }

                        if is_rate_limit_error(&self.update_state) {
                            let btn = egui::Button::new(
                                egui::RichText::new("Добавить токен").size(13.0),
                            )
                            .min_size(egui::vec2(150.0, 32.0))
                            .fill(egui::Color32::from_rgb(120, 90, 56));
                            if ui.add_enabled(!busy, btn).clicked() {
                                self.token_input = self
                                    .config
                                    .github_token
                                    .clone()
                                    .unwrap_or_default();
                                self.token_dialog_error = None;
                                self.show_token_dialog = true;
                            }
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

    fn draw_token_dialog(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let mut save = false;
        let mut clear = false;

        let mut open = self.show_token_dialog;
        egui::Window::new("GitHub токен")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.label(
                    egui::RichText::new(
                        "Анонимный API GitHub ограничен 60 запросами в час. \
                         Личный access token поднимает лимит до 5000 в час.",
                    )
                    .size(12.0)
                    .color(egui::Color32::from_gray(180)),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Создать: github.com → Settings → Developer settings → \
                         Personal access tokens → Tokens (classic). \
                         Минимум прав: public_repo (для публичного репозитория достаточно).",
                    )
                    .size(11.0)
                    .color(egui::Color32::from_gray(150)),
                );
                ui.add_space(10.0);

                ui.label(
                    egui::RichText::new("Токен:")
                        .size(13.0)
                        .color(egui::Color32::from_gray(220)),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.token_input)
                        .password(true)
                        .desired_width(f32::INFINITY)
                        .hint_text("ghp_..."),
                );

                if let Some(err) = &self.token_dialog_error {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(err)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(220, 120, 120)),
                    );
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Сохраняется в {}",
                        appdata_config_path().display()
                    ))
                    .size(11.0)
                    .italics()
                    .color(egui::Color32::from_gray(130)),
                );

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(
                        egui::RichText::new("Сохранить").size(13.0),
                    )
                    .min_size(egui::vec2(110.0, 30.0))
                    .fill(egui::Color32::from_rgb(56, 130, 90));
                    if ui.add(save_btn).clicked() {
                        save = true;
                    }

                    let cancel_btn = egui::Button::new(
                        egui::RichText::new("Отмена").size(13.0),
                    )
                    .min_size(egui::vec2(110.0, 30.0));
                    if ui.add(cancel_btn).clicked() {
                        close = true;
                    }

                    if self.config.github_token.is_some() {
                        let clear_btn = egui::Button::new(
                            egui::RichText::new("Удалить").size(13.0),
                        )
                        .min_size(egui::vec2(110.0, 30.0))
                        .fill(egui::Color32::from_rgb(120, 60, 60));
                        if ui.add(clear_btn).clicked() {
                            clear = true;
                        }
                    }
                });
            });

        if !open {
            close = true;
        }

        if save {
            let trimmed = self.token_input.trim().to_string();
            if trimmed.is_empty() {
                self.token_dialog_error =
                    Some("Поле токена пустое.".to_string());
            } else {
                self.config.github_token = Some(trimmed);
                match save_config(&self.config, &self.logger) {
                    Ok(()) => {
                        self.show_token_dialog = false;
                        self.token_input.clear();
                        self.token_dialog_error = None;

                        self.start_update_check(ctx.clone());
                    }
                    Err(e) => {
                        self.token_dialog_error =
                            Some(format!("Не удалось сохранить: {e}"));
                    }
                }
            }
        }

        if clear {
            self.config.github_token = None;
            match save_config(&self.config, &self.logger) {
                Ok(()) => {
                    self.show_token_dialog = false;
                    self.token_input.clear();
                    self.token_dialog_error = None;
                }
                Err(e) => {
                    self.token_dialog_error =
                        Some(format!("Не удалось сохранить: {e}"));
                }
            }
        }

        if close {
            self.show_token_dialog = false;
            self.token_input.clear();
            self.token_dialog_error = None;
        }
    }

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

    fn draw_telemetry(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut to_disable: Option<TelemetryId> = None;
        let mut to_enable: Option<TelemetryId> = None;

        ui.label(
            egui::RichText::new(
                "Отключение фоновой телеметрии популярного ПО. Требуются права администратора.\n\
                 Изменения применяются через политики реестра, службы и задачи планировщика.",
            )
            .size(12.0)
            .color(egui::Color32::from_gray(170)),
        );
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for item in &self.telemetry {
                    match draw_telemetry_card(ui, item) {
                        TelemetryAction::None => {}
                        TelemetryAction::Disable => to_disable = Some(item.id),
                        TelemetryAction::Enable => to_enable = Some(item.id),
                    }
                    ui.add_space(10.0);
                }
            });

        if let Some(id) = to_disable {
            self.start_telemetry_op(id, true, ctx.clone());
        }
        if let Some(id) = to_enable {
            self.start_telemetry_op(id, false, ctx.clone());
        }
    }

    fn draw_memory(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {

        let due = match self.mem_last_refresh {
            None => true,
            Some(t) => t.elapsed() >= std::time::Duration::from_secs(3),
        };
        if due {
            self.start_mem_refresh(ctx.clone());
        }

        ctx.request_repaint_after(std::time::Duration::from_secs(3));

        ui.label(
            egui::RichText::new(
                "Мониторинг и очистка standby-кеша оперативной памяти Windows. \
                 Операции вызывают NtSetSystemInformation и требуют прав администратора.\n\
                 Раздел в статусе Beta — поведение может отличаться на разных системах.",
            )
            .size(12.0)
            .color(egui::Color32::from_gray(170)),
        );
        ui.add_space(10.0);

        let mut refresh_now = false;
        let mut to_run: Option<MemOp> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {

                info_card(ui, "Использование памяти", |ui| {
                    if let Some(info) = self.mem_info {
                        let used = info.total_bytes.saturating_sub(info.avail_bytes);
                        info_row(ui, "Всего", &format_bytes(info.total_bytes));
                        info_row(
                            ui,
                            "Используется",
                            &format!(
                                "{}  ({}%)",
                                format_bytes(used),
                                info.memory_load
                            ),
                        );
                        info_row(ui, "Доступно", &format_bytes(info.avail_bytes));
                        info_row(ui, "Свободно", &format_bytes(info.free_bytes));
                        info_row(ui, "Standby (ожидание)", &format_bytes(info.standby_bytes));
                        info_row(ui, "Modified", &format_bytes(info.modified_bytes));
                        ui.add_space(6.0);

                        let bar_h = 10.0;
                        let bar_w = ui.available_width().min(420.0);
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, bar_h),
                            egui::Sense::hover(),
                        );
                        let total = info.total_bytes.max(1) as f32;
                        let used_w = bar_w * (used as f32 / total);
                        let standby_w = bar_w * (info.standby_bytes as f32 / total);
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(4),
                            egui::Color32::from_rgb(48, 48, 56),
                        );
                        let used_rect = egui::Rect::from_min_size(
                            rect.left_top(),
                            egui::vec2(used_w, bar_h),
                        );
                        ui.painter().rect_filled(
                            used_rect,
                            egui::CornerRadius::same(4),
                            egui::Color32::from_rgb(170, 80, 80),
                        );
                        let standby_rect = egui::Rect::from_min_size(
                            egui::pos2(used_rect.right(), rect.top()),
                            egui::vec2(standby_w, bar_h),
                        );
                        ui.painter().rect_filled(
                            standby_rect,
                            egui::CornerRadius::same(4),
                            egui::Color32::from_rgb(190, 150, 70),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Сбор сведений о памяти...")
                                .size(13.0)
                                .italics()
                                .color(egui::Color32::from_gray(170)),
                        );
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let btn = egui::Button::new(
                            egui::RichText::new("Обновить").size(13.0),
                        )
                        .min_size(egui::vec2(140.0, 30.0))
                        .fill(egui::Color32::from_rgb(56, 90, 170));
                        if ui
                            .add_enabled(!self.mem_refresh_in_flight, btn)
                            .clicked()
                        {
                            refresh_now = true;
                        }
                        if self.mem_refresh_in_flight {
                            ui.label(
                                egui::RichText::new("обновление...")
                                    .size(11.0)
                                    .italics()
                                    .color(egui::Color32::from_gray(140)),
                            );
                        } else if let Some(t) = self.mem_last_refresh {
                            let secs = t.elapsed().as_secs();
                            ui.label(
                                egui::RichText::new(format!(
                                    "обновлено {secs} с назад"
                                ))
                                .size(11.0)
                                .italics()
                                .color(egui::Color32::from_gray(140)),
                            );
                        }
                    });
                });
                ui.add_space(10.0);

                for op in [
                    MemOp::PurgeStandby,
                    MemOp::PurgeLowPriorityStandby,
                    MemOp::EmptyWorkingSets,
                    MemOp::FlushModified,
                ] {
                    let busy = self.mem_busy;
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgb(34, 34, 40))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(48, 48, 56),
                        ))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let label = if busy { "..." } else { "Выполнить" };
                                        let color = if busy {
                                            egui::Color32::from_rgb(72, 72, 88)
                                        } else {
                                            egui::Color32::from_rgb(170, 110, 40)
                                        };
                                        let btn = egui::Button::new(
                                            egui::RichText::new(label).size(13.0),
                                        )
                                        .min_size(egui::vec2(120.0, 36.0))
                                        .fill(color);
                                        if ui.add_enabled(!busy, btn).clicked() {
                                            to_run = Some(op);
                                        }
                                        ui.with_layout(
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(op.title())
                                                        .size(15.0)
                                                        .strong()
                                                        .color(egui::Color32::from_gray(230)),
                                                );
                                                ui.add_space(2.0);
                                                ui.label(
                                                    egui::RichText::new(op.description())
                                                        .size(12.0)
                                                        .color(egui::Color32::from_gray(170)),
                                                );
                                            },
                                        );
                                    },
                                );
                            });
                        });
                    ui.add_space(8.0);
                }

                if let Some(log) = &self.mem_log {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(log)
                            .size(12.0)
                            .italics()
                            .color(egui::Color32::from_gray(180)),
                    );
                }
            });

        if refresh_now {
            self.start_mem_refresh(ctx.clone());
        }
        if let Some(op) = to_run {
            self.start_mem_op(op, ctx.clone());
        }
    }

    fn draw_cleanup(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.cleanup_sizes_loaded && !self.cleanup_refresh_in_flight {
            self.start_cleanup_refresh(ctx.clone());
        }

        ui.label(
            egui::RichText::new(
                "Освобождение места на диске. Требуются права администратора.\n\
                 Каждая категория считает занимаемое место и очищает соответствующие пути \
                 или вызывает системные утилиты (DISM, vssadmin, powercfg, wsreset).",
            )
            .size(12.0)
            .color(egui::Color32::from_gray(170)),
        );
        ui.add_space(8.0);

        let mut refresh_now = false;
        let mut to_run: Option<CleanupId> = None;
        let total_known: u64 = self
            .cleanup_items
            .iter()
            .map(|c| match c.size {
                CleanupSize::Bytes(n) => n,
                _ => 0,
            })
            .sum();

        ui.horizontal(|ui| {
            let btn = egui::Button::new(
                egui::RichText::new("Пересчитать размеры").size(13.0),
            )
            .min_size(egui::vec2(180.0, 30.0))
            .fill(egui::Color32::from_rgb(56, 90, 170));
            if ui
                .add_enabled(!self.cleanup_refresh_in_flight, btn)
                .clicked()
            {
                refresh_now = true;
            }
            if self.cleanup_refresh_in_flight {
                ui.label(
                    egui::RichText::new("подсчёт...")
                        .size(11.0)
                        .italics()
                        .color(egui::Color32::from_gray(140)),
                );
            } else if self.cleanup_sizes_loaded {
                ui.label(
                    egui::RichText::new(format!(
                        "Всего к очистке: ≈ {}",
                        format_bytes(total_known)
                    ))
                    .size(12.0)
                    .color(egui::Color32::from_gray(200)),
                );
            }
        });
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for item in &self.cleanup_items {
                    if let Some(id) = draw_cleanup_card(ui, item) {
                        to_run = Some(id);
                    }
                    ui.add_space(8.0);
                }
            });

        if refresh_now {
            self.start_cleanup_refresh(ctx.clone());
        }
        if let Some(id) = to_run {
            self.start_cleanup_op(id, ctx.clone());
        }
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {

                setting_row(
                    ui,
                    "Сбор логов",
                    &format!(
                        "Приложение пишет логи в %APPDATA%\\WindowsSettings\\logs\\\n\
                         Текущий файл: {}",
                        self.logger.path.display()
                    ),
                    |ui| {
                        let mut new_level: Option<LogLevel> = None;
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.settings.log_level == LogLevel::Normal,
                                    "  Обычный  ",
                                )
                                .clicked()
                            {
                                new_level = Some(LogLevel::Normal);
                            }
                            if ui
                                .selectable_label(
                                    self.settings.log_level == LogLevel::Debug,
                                    "  Debug  ",
                                )
                                .clicked()
                            {
                                new_level = Some(LogLevel::Debug);
                            }
                        });
                        if let Some(level) = new_level {
                            if level != self.settings.log_level {
                                self.settings.log_level = level;
                                self.logger.set_level(level);
                                if let Err(e) = save_settings(&self.settings, &self.logger) {
                                    self.logger.log(
                                        LogLevel::Normal,
                                        &format!("Не удалось сохранить settings.conf: {e}"),
                                    );
                                }
                            }
                        }
                    },
                );
                ui.add_space(10.0);

                info_card(ui, "Хранение", |ui| {
                    info_row(
                        ui,
                        "Настройки",
                        &appdata_settings_path().display().to_string(),
                    );
                    info_row(
                        ui,
                        "Токен GitHub",
                        &appdata_config_path().display().to_string(),
                    );
                });
                ui.add_space(10.0);
            });
    }
}

fn info_card<R>(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(15.0)
                    .strong()
                    .color(egui::Color32::from_gray(230)),
            );
            ui.add_space(6.0);
            content(ui);
        });
}

fn info_row(ui: &mut egui::Ui, key: &str, value: &str) {
    let display_value = if value.trim().is_empty() { "—" } else { value };
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(140.0, 18.0),
            egui::Label::new(
                egui::RichText::new(key)
                    .size(12.0)
                    .color(egui::Color32::from_gray(150)),
            ),
        );
        ui.label(
            egui::RichText::new(display_value)
                .size(13.0)
                .color(egui::Color32::from_gray(225)),
        );
    });
    ui.add_space(2.0);
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

#[derive(PartialEq, Eq)]
enum TelemetryAction {
    None,
    Disable,
    Enable,
}

fn draw_telemetry_card(ui: &mut egui::Ui, item: &TelemetryItem) -> TelemetryAction {
    let mut action = TelemetryAction::None;
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
                        let (label, color, enabled) = match (item.status, item.busy) {
                            (_, true) => ("...", egui::Color32::from_rgb(72, 72, 88), false),
                            (TelemetryStatus::Enabled, false) => {
                                ("Отключить", egui::Color32::from_rgb(170, 60, 60), true)
                            }
                            (TelemetryStatus::Disabled, false) => {
                                ("Включить", egui::Color32::from_rgb(56, 130, 90), true)
                            }
                            (TelemetryStatus::Unknown, false) => (
                                "Проверка...",
                                egui::Color32::from_rgb(60, 60, 70),
                                false,
                            ),
                        };

                        let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                            .min_size(egui::vec2(120.0, 36.0))
                            .fill(color);
                        if ui.add_enabled(enabled, btn).clicked() {
                            action = match item.status {
                                TelemetryStatus::Enabled => TelemetryAction::Disable,
                                TelemetryStatus::Disabled => TelemetryAction::Enable,
                                TelemetryStatus::Unknown => TelemetryAction::None,
                            };
                        }

                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            let (status_label, status_color) = match item.status {
                                TelemetryStatus::Enabled => (
                                    "Включена",
                                    egui::Color32::from_rgb(220, 120, 120),
                                ),
                                TelemetryStatus::Disabled => (
                                    "Отключена",
                                    egui::Color32::from_rgb(120, 200, 140),
                                ),
                                TelemetryStatus::Unknown => (
                                    "Проверяется...",
                                    egui::Color32::from_gray(170),
                                ),
                            };

                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&item.title)
                                        .size(15.0)
                                        .strong()
                                        .color(egui::Color32::from_gray(230)),
                                );
                                ui.label(
                                    egui::RichText::new(format!("• {status_label}"))
                                        .size(12.0)
                                        .color(status_color),
                                );
                            });
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(&item.description)
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(170)),
                            );
                            if let Some(log) = &item.log {
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

fn draw_cleanup_card(ui: &mut egui::Ui, item: &CleanupItem) -> Option<CleanupId> {
    let mut clicked: Option<CleanupId> = None;
    let danger_color = egui::Color32::from_rgb(220, 160, 90);
    let neutral_color = egui::Color32::from_rgb(225, 225, 225);

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(
            1.0,
            if item.danger {
                egui::Color32::from_rgb(110, 80, 50)
            } else {
                egui::Color32::from_rgb(48, 48, 56)
            },
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let (label, color, enabled) = if item.busy {
                            (
                                "...".to_string(),
                                egui::Color32::from_rgb(72, 72, 88),
                                false,
                            )
                        } else {
                            let base = if item.danger {
                                egui::Color32::from_rgb(170, 70, 60)
                            } else {
                                egui::Color32::from_rgb(170, 110, 40)
                            };
                            let nothing_to_clean = matches!(item.size, CleanupSize::Bytes(0));
                            let lbl = if nothing_to_clean {
                                "Пусто".to_string()
                            } else if item.danger {
                                "Выполнить".to_string()
                            } else {
                                "Очистить".to_string()
                            };
                            let col = if nothing_to_clean {
                                egui::Color32::from_rgb(60, 60, 70)
                            } else {
                                base
                            };
                            (lbl, col, !nothing_to_clean)
                        };

                        let btn = egui::Button::new(
                            egui::RichText::new(label).size(13.0),
                        )
                        .min_size(egui::vec2(120.0, 36.0))
                        .fill(color);
                        if ui.add_enabled(enabled, btn).clicked() {
                            clicked = Some(item.id);
                        }

                        ui.with_layout(
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&item.title)
                                            .size(15.0)
                                            .strong()
                                            .color(if item.danger {
                                                danger_color
                                            } else {
                                                neutral_color
                                            }),
                                    );

                                    let size_label = match item.size {
                                        CleanupSize::Unknown => {
                                            "• подсчёт...".to_string()
                                        }
                                        CleanupSize::NotApplicable => {
                                            "• системная операция".to_string()
                                        }
                                        CleanupSize::Bytes(n) => {
                                            format!("• {}", format_bytes(n))
                                        }
                                    };
                                    let size_color = match item.size {
                                        CleanupSize::Bytes(0) => {
                                            egui::Color32::from_gray(120)
                                        }
                                        CleanupSize::Bytes(_) => {
                                            egui::Color32::from_rgb(140, 200, 240)
                                        }
                                        _ => egui::Color32::from_gray(150),
                                    };
                                    ui.label(
                                        egui::RichText::new(size_label)
                                            .size(12.0)
                                            .color(size_color),
                                    );

                                    if item.danger {
                                        ui.label(
                                            egui::RichText::new("• ⚠ необратимо")
                                                .size(11.0)
                                                .color(danger_color),
                                        );
                                    }
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(&item.description)
                                        .size(12.0)
                                        .color(egui::Color32::from_gray(170)),
                                );
                                if let Some(log) = &item.log {
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(log)
                                            .size(11.0)
                                            .italics()
                                            .color(egui::Color32::from_gray(130)),
                                    );
                                }
                            },
                        );
                    },
                );
            });
        });
    clicked
}

fn telemetry_items() -> Vec<TelemetryItem> {
    let items: &[(TelemetryId, &str, &str)] = &[
        (
            TelemetryId::Office,
            "Microsoft Office",
            "OfficeTelemetryAgent, ClientTelemetry и связанные задачи планировщика.",
        ),
        (
            TelemetryId::Firefox,
            "Mozilla Firefox",
            "Политики DisableTelemetry, DisableFirefoxStudies, DisableDefaultBrowserAgent.",
        ),
        (
            TelemetryId::Chrome,
            "Google Chrome",
            "MetricsReportingEnabled = 0 и отключение задач GoogleUpdateTask*.",
        ),
        (
            TelemetryId::Nvidia,
            "NVIDIA",
            "Служба NvTelemetryContainer и задачи NvTmRep / NvTmMon / NvNodeLauncher.",
        ),
        (
            TelemetryId::VisualStudio,
            "Visual Studio (VSCEIP)",
            "Customer Experience Improvement Program и Feedback для VS 2015–2022.",
        ),
        (
            TelemetryId::Windows,
            "Windows 11",
            "Службы DiagTrack и dmwappushservice + политики AllowTelemetry = 0.",
        ),
    ];

    items
        .iter()
        .map(|(id, title, desc)| TelemetryItem {
            id: *id,
            title: (*title).to_string(),
            description: (*desc).to_string(),
            status: TelemetryStatus::Unknown,
            busy: false,
            log: None,
        })
        .collect()
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

    if item.beta {
        let badge_w = 38.0;
        let badge_h = 18.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - badge_w - 8.0, rect.center().y - badge_h * 0.5),
            egui::vec2(badge_w, badge_h),
        );
        ui.painter().rect_filled(
            badge_rect,
            egui::CornerRadius::same(4),
            egui::Color32::from_rgb(170, 110, 40),
        );
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Beta",
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
    }

    response
}

fn draw_beta_badge(ui: &mut egui::Ui, font_size: f32) {
    let text = "Beta";
    let pad_x = 8.0;
    let pad_y = 3.0;
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(font_size),
        egui::Color32::WHITE,
    );
    let size = galley.size() + egui::vec2(pad_x * 2.0, pad_y * 2.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(4),
        egui::Color32::from_rgb(170, 110, 40),
    );
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        egui::Color32::WHITE,
    );
}

fn run_powershell(script: &str, logger: &Logger) -> (bool, String) {

    let wrapped = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;\
         $OutputEncoding = [System.Text.Encoding]::UTF8;\
         {script}"
    );
    logger.log(LogLevel::Debug, &format!("PS> {script}"));

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-NonInteractive",
            "-Command",
            &wrapped,
        ])
        .creation_flags(CREATE_NO_WINDOW)
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

const SYSINFO_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$os = Get-CimInstance Win32_OperatingSystem
$cs = Get-CimInstance Win32_ComputerSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$gpus = Get-CimInstance Win32_VideoController | Where-Object { $_.Name } | Select-Object -ExpandProperty Name
try {
    $adm = [bool]([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} catch { $adm = '' }
"os=$($os.Caption)"
"build=$($os.BuildNumber)"
"arch=$($os.OSArchitecture)"
"hostname=$($cs.Name)"
"user=$env:USERNAME"
"admin=$adm"
"cpu=$($cpu.Name)"
$gpuLine = if ($gpus) { ($gpus -join ', ') } else { '' }
"gpu=$gpuLine"
$ramGb = if ($cs.TotalPhysicalMemory) { [math]::Round($cs.TotalPhysicalMemory/1GB,1) } else { '' }
"ram_gb=$ramGb"
"#;

fn collect_sys_info(logger: &Logger) -> SysInfo {
    let (ok, out) = run_powershell(SYSINFO_SCRIPT, logger);
    let mut info = SysInfo::default();
    if !ok {
        logger.log(
            LogLevel::Normal,
            &format!("System info query failed: {out}"),
        );
        return info;
    }
    for line in out.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim().to_string();
        match k.trim() {
            "os" => info.os = v,
            "build" => info.build = v,
            "arch" => info.arch = v,
            "hostname" => info.hostname = v,
            "user" => info.user = v,
            "admin" => {
                info.is_admin = match v.as_str() {
                    "True" => Some(true),
                    "False" => Some(false),
                    _ => None,
                };
            }
            "cpu" => info.cpu = v.split_whitespace().collect::<Vec<_>>().join(" "),
            "gpu" => info.gpu = v,
            "ram_gb" => info.ram_gb = v,
            _ => {}
        }
    }
    info
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

const TELEMETRY_STATUS_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'

function Get-Reg([string]$Path, [string]$Name) {
    try {
        if (Test-Path $Path) {
            return (Get-ItemProperty -Path $Path -Name $Name -ErrorAction Stop).$Name
        }
    } catch {}
    return $null
}

# Office
$office = $false
foreach ($v in @('14.0','15.0','16.0')) {
    if ((Get-Reg "HKCU:\Software\Policies\Microsoft\office\$v\osm" 'enablelogging') -eq 0) { $office = $true }
    if ((Get-Reg "HKCU:\Software\Policies\Microsoft\office\$v\osm" 'enableupload') -eq 0) { $office = $true }
}
if ((Get-Reg "HKCU:\Software\Policies\Microsoft\Office\Common\ClientTelemetry" 'DisableTelemetry') -eq 1) { $office = $true }
Write-Output ("office=" + $(if ($office) { 'disabled' } else { 'enabled' }))

# Firefox
$ff = ((Get-Reg "HKLM:\Software\Policies\Mozilla\Firefox" 'DisableTelemetry') -eq 1)
Write-Output ("firefox=" + $(if ($ff) { 'disabled' } else { 'enabled' }))

# Chrome
$ch = ((Get-Reg "HKLM:\Software\Policies\Google\Chrome" 'MetricsReportingEnabled') -eq 0)
Write-Output ("chrome=" + $(if ($ch) { 'disabled' } else { 'enabled' }))

# NVIDIA
$nv = $false
$svc = Get-Service -Name 'NvTelemetryContainer' -ErrorAction SilentlyContinue
if (-not $svc) { $nv = $true }
elseif ($svc.StartType -eq 'Disabled') { $nv = $true }
Write-Output ("nvidia=" + $(if ($nv) { 'disabled' } else { 'enabled' }))

# Visual Studio
$vs = $false
foreach ($v in @('14.0','15.0','16.0','17.0')) {
    if ((Get-Reg "HKCU:\Software\Microsoft\VSCommon\$v\SQM" 'OptIn') -eq 0) { $vs = $true }
}
if ((Get-Reg "HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\SQM" 'OptIn') -eq 0) { $vs = $true }
Write-Output ("vs=" + $(if ($vs) { 'disabled' } else { 'enabled' }))

# Windows
$w = $false
$diag = Get-Service -Name 'DiagTrack' -ErrorAction SilentlyContinue
if ($diag -and $diag.StartType -eq 'Disabled') { $w = $true }
if ((Get-Reg "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection" 'AllowTelemetry') -eq 0) { $w = $true }
Write-Output ("windows=" + $(if ($w) { 'disabled' } else { 'enabled' }))
"#;

fn query_telemetry_status(logger: &Logger) -> Vec<(TelemetryId, TelemetryStatus)> {
    let (ok, out) = run_powershell(TELEMETRY_STATUS_SCRIPT, logger);
    if !ok {
        logger.log(
            LogLevel::Normal,
            &format!("Telemetry status query failed: {out}"),
        );
    }
    let mut result = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once('=') {
            if let Some(id) = TelemetryId::from_key(k.trim()) {
                let status = match v.trim() {
                    "disabled" => TelemetryStatus::Disabled,
                    "enabled" => TelemetryStatus::Enabled,
                    _ => TelemetryStatus::Unknown,
                };
                result.push((id, status));
            }
        }
    }
    result
}

fn run_telemetry_op(id: TelemetryId, disable: bool, logger: &Logger) -> (bool, String) {
    let script = telemetry_script(id, disable);
    run_powershell(script, logger)
}

fn telemetry_script(id: TelemetryId, disable: bool) -> &'static str {
    match (id, disable) {
        (TelemetryId::Office, true) => OFFICE_DISABLE,
        (TelemetryId::Office, false) => OFFICE_ENABLE,
        (TelemetryId::Firefox, true) => FIREFOX_DISABLE,
        (TelemetryId::Firefox, false) => FIREFOX_ENABLE,
        (TelemetryId::Chrome, true) => CHROME_DISABLE,
        (TelemetryId::Chrome, false) => CHROME_ENABLE,
        (TelemetryId::Nvidia, true) => NVIDIA_DISABLE,
        (TelemetryId::Nvidia, false) => NVIDIA_ENABLE,
        (TelemetryId::VisualStudio, true) => VS_DISABLE,
        (TelemetryId::VisualStudio, false) => VS_ENABLE,
        (TelemetryId::Windows, true) => WINDOWS_DISABLE,
        (TelemetryId::Windows, false) => WINDOWS_ENABLE,
    }
}

const OFFICE_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
foreach ($v in @('14.0','15.0','16.0')) {
    $p = "HKCU:\Software\Policies\Microsoft\office\$v\osm"
    New-Item -Path $p -Force | Out-Null
    Set-ItemProperty -Path $p -Name 'enablelogging' -Value 0 -Type DWord -Force
    Set-ItemProperty -Path $p -Name 'enableupload' -Value 0 -Type DWord -Force
}
$ct = "HKCU:\Software\Policies\Microsoft\Office\Common\ClientTelemetry"
New-Item -Path $ct -Force | Out-Null
Set-ItemProperty -Path $ct -Name 'DisableTelemetry' -Value 1 -Type DWord -Force
foreach ($t in @(
    '\Microsoft\Office\OfficeTelemetryAgentLogOn2016',
    '\Microsoft\Office\OfficeTelemetryAgentFallBack2016',
    '\Microsoft\Office\Office Feature Updates'
)) {
    Disable-ScheduledTask -TaskName $t -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'Office telemetry: отключена.'
"#;

const OFFICE_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
foreach ($v in @('14.0','15.0','16.0')) {
    Remove-Item -Path "HKCU:\Software\Policies\Microsoft\office\$v\osm" -Recurse -Force -ErrorAction SilentlyContinue
}
Remove-Item -Path "HKCU:\Software\Policies\Microsoft\Office\Common\ClientTelemetry" -Recurse -Force -ErrorAction SilentlyContinue
foreach ($t in @(
    '\Microsoft\Office\OfficeTelemetryAgentLogOn2016',
    '\Microsoft\Office\OfficeTelemetryAgentFallBack2016',
    '\Microsoft\Office\Office Feature Updates'
)) {
    Enable-ScheduledTask -TaskName $t -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'Office telemetry: включена.'
"#;

const FIREFOX_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
$p = "HKLM:\Software\Policies\Mozilla\Firefox"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'DisableTelemetry' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'DisableFirefoxStudies' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'DisableDefaultBrowserAgent' -Value 1 -Type DWord -Force
Write-Output 'Firefox telemetry: отключена.'
"#;

const FIREFOX_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
$p = "HKLM:\Software\Policies\Mozilla\Firefox"
Remove-ItemProperty -Path $p -Name 'DisableTelemetry' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'DisableFirefoxStudies' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'DisableDefaultBrowserAgent' -Force -ErrorAction SilentlyContinue
Write-Output 'Firefox telemetry: включена.'
"#;

const CHROME_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
$p = "HKLM:\Software\Policies\Google\Chrome"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'MetricsReportingEnabled' -Value 0 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'DefaultBrowserSettingEnabled' -Value 0 -Type DWord -Force
Get-ScheduledTask -TaskName 'GoogleUpdateTask*' -ErrorAction SilentlyContinue | ForEach-Object {
    Disable-ScheduledTask -TaskName $_.TaskName -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'Chrome telemetry: отключена.'
"#;

const CHROME_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
$p = "HKLM:\Software\Policies\Google\Chrome"
Remove-ItemProperty -Path $p -Name 'MetricsReportingEnabled' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path $p -Name 'DefaultBrowserSettingEnabled' -Force -ErrorAction SilentlyContinue
Get-ScheduledTask -TaskName 'GoogleUpdateTask*' -ErrorAction SilentlyContinue | ForEach-Object {
    Enable-ScheduledTask -TaskName $_.TaskName -ErrorAction SilentlyContinue | Out-Null
}
Write-Output 'Chrome telemetry: включена.'
"#;

const NVIDIA_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'NvTelemetryContainer' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'NvTelemetryContainer' -StartupType Disabled -ErrorAction SilentlyContinue
foreach ($pat in @('NvTmRep_CrashReport*','NvTmMon*','NvTmRep*','NvDriverUpdateCheckDaily_*','NvNodeLauncher_*')) {
    Get-ScheduledTask -TaskName $pat -ErrorAction SilentlyContinue | ForEach-Object {
        Disable-ScheduledTask -TaskName $_.TaskName -ErrorAction SilentlyContinue | Out-Null
    }
}
$p = "HKLM:\SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'OptInOrOutPreference' -Value 0 -Type DWord -Force
Write-Output 'NVIDIA telemetry: отключена.'
"#;

const NVIDIA_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'NvTelemetryContainer' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'NvTelemetryContainer' -ErrorAction SilentlyContinue
foreach ($pat in @('NvTmRep_CrashReport*','NvTmMon*','NvTmRep*','NvDriverUpdateCheckDaily_*','NvNodeLauncher_*')) {
    Get-ScheduledTask -TaskName $pat -ErrorAction SilentlyContinue | ForEach-Object {
        Enable-ScheduledTask -TaskName $_.TaskName -ErrorAction SilentlyContinue | Out-Null
    }
}
Remove-ItemProperty -Path "HKLM:\SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client" -Name 'OptInOrOutPreference' -Force -ErrorAction SilentlyContinue
Write-Output 'NVIDIA telemetry: включена.'
"#;

const VS_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
foreach ($v in @('14.0','15.0','16.0','17.0')) {
    $p = "HKCU:\Software\Microsoft\VSCommon\$v\SQM"
    New-Item -Path $p -Force | Out-Null
    Set-ItemProperty -Path $p -Name 'OptIn' -Value 0 -Type DWord -Force
}
$p = "HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\SQM"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'OptIn' -Value 0 -Type DWord -Force
$p = "HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'DisableFeedbackDialog' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'DisableEmailInput' -Value 1 -Type DWord -Force
Set-ItemProperty -Path $p -Name 'DisableScreenshotCapture' -Value 1 -Type DWord -Force
Write-Output 'Visual Studio telemetry: отключена.'
"#;

const VS_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
foreach ($v in @('14.0','15.0','16.0','17.0')) {
    Remove-ItemProperty -Path "HKCU:\Software\Microsoft\VSCommon\$v\SQM" -Name 'OptIn' -Force -ErrorAction SilentlyContinue
}
Remove-Item -Path "HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\SQM" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "HKLM:\SOFTWARE\Policies\Microsoft\VisualStudio\Feedback" -Recurse -Force -ErrorAction SilentlyContinue
Write-Output 'Visual Studio telemetry: включена.'
"#;

const WINDOWS_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'DiagTrack' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DiagTrack' -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name 'dmwappushservice' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Disabled -ErrorAction SilentlyContinue
$p = "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'AllowTelemetry' -Value 0 -Type DWord -Force
$p = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection"
New-Item -Path $p -Force | Out-Null
Set-ItemProperty -Path $p -Name 'AllowTelemetry' -Value 0 -Type DWord -Force
Write-Output 'Windows telemetry: отключена.'
"#;

const WINDOWS_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'DiagTrack' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'DiagTrack' -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Manual -ErrorAction SilentlyContinue
Remove-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection" -Name 'AllowTelemetry' -Force -ErrorAction SilentlyContinue
Remove-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection" -Name 'AllowTelemetry' -Force -ErrorAction SilentlyContinue
Write-Output 'Windows telemetry: включена.'
"#;

const MEM_INFO_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$src = @'
using System;
using System.Runtime.InteropServices;

public static class WSMemInfo {
    [DllImport("ntdll.dll")]
    public static extern uint NtQuerySystemInformation(int Class, IntPtr Info, int Length, out int RetLen);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GlobalMemoryStatusEx(ref MEMORYSTATUSEX lpBuffer);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    public struct MEMORYSTATUSEX {
        public uint dwLength;
        public uint dwMemoryLoad;
        public ulong ullTotalPhys;
        public ulong ullAvailPhys;
        public ulong ullTotalPageFile;
        public ulong ullAvailPageFile;
        public ulong ullTotalVirtual;
        public ulong ullAvailVirtual;
        public ulong ullAvailExtendedVirtual;
    }

    public static string Get() {
        var ms = new MEMORYSTATUSEX();
        ms.dwLength = (uint)Marshal.SizeOf(typeof(MEMORYSTATUSEX));
        GlobalMemoryStatusEx(ref ms);

        // SystemMemoryListInformation = 80 (0x50). Размер структуры:
        // ZeroPage, FreePage, ModifiedPage, ModifiedNoWritePage, BadPage,
        // PageCountByPriority[8], RepurposedPagesByPriority[8], ModifiedPageCountPageFile
        // = 5 + 8 + 8 + 1 = 22 SIZE_T полей.
        int slots = 22;
        int sz = IntPtr.Size * slots;
        IntPtr buf = Marshal.AllocHGlobal(sz);
        ulong standby = 0;
        ulong modified = 0;
        ulong free = 0;
        try {
            int ret;
            uint status = NtQuerySystemInformation(80, buf, sz, out ret);
            if (status == 0) {
                int psz = Environment.SystemPageSize;
                long zp = Read(buf, 0);
                long fp = Read(buf, 1);
                long mp = Read(buf, 2);
                free = (ulong)(zp + fp) * (ulong)psz;
                modified = (ulong)mp * (ulong)psz;
                long s = 0;
                for (int i = 0; i < 8; i++) s += Read(buf, 5 + i);
                standby = (ulong)s * (ulong)psz;
            }
        } finally {
            Marshal.FreeHGlobal(buf);
        }
        return string.Format(
            "total={0};avail={1};standby={2};modified={3};free={4};load={5}",
            ms.ullTotalPhys, ms.ullAvailPhys, standby, modified, free, ms.dwMemoryLoad);
    }

    static long Read(IntPtr p, int idx) {
        IntPtr v = Marshal.ReadIntPtr(p, idx * IntPtr.Size);
        return v.ToInt64();
    }
}
'@
Add-Type -TypeDefinition $src -Language CSharp | Out-Null
[WSMemInfo]::Get()
"#;

const MEM_CLEAN_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$src = @'
using System;
using System.Runtime.InteropServices;

public static class WSMemClean {
    // ВАЖНО: LUID должен быть отдельной структурой из двух 32-битных полей.
    // Если объявить его как `long` внутри TOKEN_PRIVILEGES, в 64-битном
    // процессе компилятор добавит 4 байта padding между PrivilegeCount и
    // Luid (natural alignment Int64 = 8). Из-за этого LUID уезжает на
    // неверное смещение, AdjustTokenPrivileges вернёт ERROR_NOT_ALL_ASSIGNED
    // (1300), привилегия не включится, и NtSetSystemInformation упадёт с
    // STATUS_PRIVILEGE_NOT_HELD (0xC0000061).
    [StructLayout(LayoutKind.Sequential)]
    public struct LUID {
        public uint LowPart;
        public int HighPart;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct TOKEN_PRIVILEGES {
        public uint PrivilegeCount;
        public LUID Luid;
        public uint Attributes;
    }

    [DllImport("ntdll.dll")]
    public static extern uint NtSetSystemInformation(int InfoClass, IntPtr Info, int Length);

    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool OpenProcessToken(IntPtr ProcessHandle, uint DesiredAccess, out IntPtr TokenHandle);

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool LookupPrivilegeValue(string lpSystemName, string lpName, out LUID lpLuid);

    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool AdjustTokenPrivileges(IntPtr TokenHandle, [MarshalAs(UnmanagedType.Bool)] bool DisableAllPrivileges, ref TOKEN_PRIVILEGES NewState, uint BufferLength, IntPtr PreviousState, IntPtr ReturnLength);

    [DllImport("kernel32.dll")]
    public static extern IntPtr GetCurrentProcess();

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool CloseHandle(IntPtr h);

    const uint SE_PRIVILEGE_ENABLED = 0x00000002;
    const uint TOKEN_ADJUST_PRIVILEGES = 0x0020;
    const uint TOKEN_QUERY = 0x0008;

    static bool Enable(string name) {
        IntPtr token;
        if (!OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, out token)) return false;
        try {
            LUID luid;
            if (!LookupPrivilegeValue(null, name, out luid)) return false;
            var tp = new TOKEN_PRIVILEGES {
                PrivilegeCount = 1,
                Luid = luid,
                Attributes = SE_PRIVILEGE_ENABLED
            };
            if (!AdjustTokenPrivileges(token, false, ref tp, 0, IntPtr.Zero, IntPtr.Zero)) return false;
            // Windows возвращает true даже если привилегии нет; реальный статус — в GetLastError.
            return Marshal.GetLastWin32Error() == 0;
        } finally {
            CloseHandle(token);
        }
    }

    public static uint Run(int command) {
        Enable("SeProfileSingleProcessPrivilege");
        Enable("SeIncreaseQuotaPrivilege");
        IntPtr ptr = Marshal.AllocHGlobal(4);
        try {
            Marshal.WriteInt32(ptr, command);
            // SystemMemoryListInformation = 80
            return NtSetSystemInformation(80, ptr, 4);
        } finally {
            Marshal.FreeHGlobal(ptr);
        }
    }
}
'@
Add-Type -TypeDefinition $src -Language CSharp | Out-Null
$status = [WSMemClean]::Run($Cmd)
if ($status -eq 0) {
    Write-Output "ok"
    exit 0
} else {
    Write-Output ("NtSetSystemInformation status=0x{0:X8} (0xC0000061 = STATUS_PRIVILEGE_NOT_HELD, нужны права администратора)" -f $status)
    exit 1
}
"#;

fn collect_mem_info(logger: &Logger) -> MemInfo {
    let (ok, out) = run_powershell(MEM_INFO_SCRIPT, logger);
    let mut info = MemInfo::default();
    if !ok {
        logger.log(
            LogLevel::Normal,
            &format!("Memory info query failed: {out}"),
        );
        return info;
    }

    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() || !line.contains('=') {
            continue;
        }
        for part in line.split(';') {
            let Some((k, v)) = part.split_once('=') else { continue };
            match k.trim() {
                "total" => info.total_bytes = v.trim().parse().unwrap_or(0),
                "avail" => info.avail_bytes = v.trim().parse().unwrap_or(0),
                "standby" => info.standby_bytes = v.trim().parse().unwrap_or(0),
                "modified" => info.modified_bytes = v.trim().parse().unwrap_or(0),
                "free" => info.free_bytes = v.trim().parse().unwrap_or(0),
                "load" => info.memory_load = v.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
        if info.total_bytes > 0 {
            break;
        }
    }
    info
}

fn run_mem_op(op: MemOp, logger: &Logger) -> (bool, String) {

    let cmd = op.command();
    let wrapped = format!("$Cmd = {cmd}\n{MEM_CLEAN_SCRIPT}");
    run_powershell(&wrapped, logger)
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0".to_string();
    }
    let units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    let mut value = bytes as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx < units.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx <= 1 {
        format!("{:.0} {}", value, units[idx])
    } else {
        format!("{:.2} {}", value, units[idx])
    }
}

fn check_latest_release(logger: &Logger, token: Option<&str>) -> Result<String, String> {
    logger.log(
        LogLevel::Debug,
        &format!(
            "Fetching latest release for {REPO_OWNER}/{REPO_NAME} (auth={})",
            if token.is_some() { "token" } else { "anonymous" }
        ),
    );
    let mut builder = self_update::backends::github::ReleaseList::configure();
    builder.repo_owner(REPO_OWNER).repo_name(REPO_NAME);
    if let Some(t) = token {
        builder.auth_token(t);
    }
    let releases = builder
        .build()
        .map_err(|e| friendly_github_error(&e.to_string()))?
        .fetch()
        .map_err(|e| friendly_github_error(&e.to_string()))?;

    let latest = releases
        .first()
        .ok_or_else(|| "На GitHub нет ни одного релиза.".to_string())?;

    let v = latest.version.trim_start_matches('v').to_string();
    logger.log(LogLevel::Debug, &format!("Latest tag: {}", latest.version));
    Ok(v)
}

fn is_rate_limit_error(state: &UpdateState) -> bool {
    if let UpdateState::Error(e) = state {
        e.contains("403")
    } else {
        false
    }
}

fn friendly_github_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("403") {
        "GitHub отклонил запрос (HTTP 403). Скорее всего, превышен лимит анонимных запросов \
         к API (60 в час с одного IP). Попробуйте позже."
            .to_string()
    } else if lower.contains("404") {
        "Репозиторий или релизы не найдены (HTTP 404).".to_string()
    } else if lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("connection")
        || lower.contains("timed out")
        || lower.contains("timeout")
    {
        "Нет соединения с GitHub. Проверьте интернет и попробуйте снова.".to_string()
    } else {
        format!("Не удалось получить данные с GitHub: {raw}")
    }
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (semver::Version::parse(latest), semver::Version::parse(current)) {
        (Ok(l), Ok(c)) => l > c,

        _ => latest != current,
    }
}

fn do_self_update(logger: &Logger, token: Option<&str>) -> Result<String, String> {
    logger.log(
        LogLevel::Debug,
        &format!(
            "Self-update from {REPO_OWNER}/{REPO_NAME}, current {APP_VERSION} (auth={})",
            if token.is_some() { "token" } else { "anonymous" }
        ),
    );

    let mut builder = self_update::backends::github::ReleaseList::configure();
    builder.repo_owner(REPO_OWNER).repo_name(REPO_NAME);
    if let Some(t) = token {
        builder.auth_token(t);
    }
    let releases = builder
        .build()
        .map_err(|e| friendly_github_error(&e.to_string()))?
        .fetch()
        .map_err(|e| friendly_github_error(&e.to_string()))?;
    let latest = releases
        .first()
        .ok_or_else(|| "На GitHub нет ни одного релиза.".to_string())?;
    let version = latest.version.trim_start_matches('v').to_string();
    logger.log(
        LogLevel::Debug,
        &format!("Self-update: latest tag={}, version={}", latest.version, version),
    );

    let asset = latest
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&format!("{REPO_NAME}.exe")))
        .or_else(|| latest.assets.iter().find(|a| a.name.to_ascii_lowercase().ends_with(".exe")))
        .ok_or_else(|| format!("В релизе {} нет .exe-файла.", latest.version))?;
    logger.log(
        LogLevel::Debug,
        &format!("Self-update: asset name={}, url={}", asset.name, asset.download_url),
    );

    let body = download_asset_bytes(&asset.download_url, token)?;
    if body.is_empty() {
        return Err("Загруженный файл оказался пустым.".to_string());
    }
    if body.len() < 1024 * 100 {
        return Err(format!(
            "Загружено только {} байт — это явно битый бинарь.",
            body.len()
        ));
    }

    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!(
        "windows-settings.update.{}.exe",
        local_timestamp_filename()
    ));
    fs::write(&tmp_path, &body).map_err(|e| format!("Не удалось записать {}: {e}", tmp_path.display()))?;
    logger.log(
        LogLevel::Debug,
        &format!("Self-update: wrote {} bytes to {}", body.len(), tmp_path.display()),
    );

    self_replace::self_replace(&tmp_path)
        .map_err(|e| format!("Не удалось заменить бинарь: {e}"))?;

    let _ = fs::remove_file(&tmp_path);

    logger.log(
        LogLevel::Debug,
        &format!("Self-update: replaced binary, new version {version}"),
    );
    Ok(version)
}

fn download_asset_bytes(url: &str, token: Option<&str>) -> Result<Vec<u8>, String> {

    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .build();
    let agent = ureq::Agent::config_builder()
        .max_redirects(8)
        .tls_config(tls)
        .build()
        .new_agent();
    let mut req = agent
        .get(url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", &format!("{REPO_NAME}/{APP_VERSION}"));
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let mut resp = req
        .call()
        .map_err(|e| friendly_github_error(&e.to_string()))?;
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(friendly_github_error(&format!(
            "HTTP {status} при загрузке asset"
        )));
    }
    let mut bytes = Vec::with_capacity(16 * 1024 * 1024);
    std::io::Read::read_to_end(
        &mut resp.body_mut().as_reader(),
        &mut bytes,
    )
    .map_err(|e| format!("Ошибка чтения тела ответа: {e}"))?;
    Ok(bytes)
}

// =====================================================================
//                              CLEANUP
// =====================================================================

fn cleanup_items() -> Vec<CleanupItem> {
    let defs: &[(CleanupId, &str, &str, bool)] = &[
        (
            CleanupId::RecycleBin,
            "Корзина",
            "Очистка корзины на всех дисках. Файлы удаляются безвозвратно.",
            false,
        ),
        (
            CleanupId::UserTemp,
            "Временные файлы пользователя",
            "%TEMP% и %LOCALAPPDATA%\\Temp — кэш установщиков, временные файлы программ.",
            false,
        ),
        (
            CleanupId::SystemTemp,
            "Временные файлы Windows",
            "C:\\Windows\\Temp — временные файлы системы и установщиков.",
            false,
        ),
        (
            CleanupId::CrashDumps,
            "Дампы падений (CrashDumps)",
            "%LOCALAPPDATA%\\CrashDumps — дампы упавших процессов.",
            false,
        ),
        (
            CleanupId::WerReports,
            "Отчёты Windows Error Reporting",
            "%ProgramData%\\Microsoft\\Windows\\WER и пользовательский WER.",
            false,
        ),
        (
            CleanupId::MinidumpAndLkr,
            "MEMORY.DMP / Minidump / LiveKernelReports",
            "C:\\Windows\\MEMORY.DMP, C:\\Windows\\Minidump\\*, C:\\Windows\\LiveKernelReports\\*.",
            false,
        ),
        (
            CleanupId::SoftwareDistribution,
            "Кэш Windows Update",
            "C:\\Windows\\SoftwareDistribution\\Download — скачанные пакеты обновлений. \
             Служба wuauserv будет временно остановлена.",
            false,
        ),
        (
            CleanupId::Catroot2,
            "catroot2",
            "C:\\Windows\\System32\\catroot2 — кэш подписей обновлений. \
             Службы cryptsvc/bits будут временно остановлены.",
            false,
        ),
        (
            CleanupId::DeliveryOptimization,
            "Delivery Optimization Cache",
            "C:\\Windows\\SoftwareDistribution\\DeliveryOptimization и \
             %ProgramData%\\Microsoft\\Windows\\DeliveryOptimization\\Cache.",
            false,
        ),
        (
            CleanupId::WindowsOld,
            "Предыдущая Windows (Windows.old)",
            "C:\\Windows.old — после удаления откатиться на старую версию ОС будет нельзя.",
            true,
        ),
        (
            CleanupId::UpgradeLeftovers,
            "Остатки апгрейда",
            "$Windows.~BT, $Windows.~WS, $Windows.~LS, C:\\ESD, C:\\Windows\\Panther.",
            false,
        ),
        (
            CleanupId::LastGood,
            "LastGood / LastGood.tmp",
            "C:\\Windows\\LastGood и C:\\Windows\\LastGood.tmp — резерв удачной конфигурации.",
            false,
        ),
        (
            CleanupId::Prefetch,
            "Prefetch",
            "C:\\Windows\\Prefetch — кэш ускорения запуска приложений (перестроится).",
            false,
        ),
        (
            CleanupId::FontCache,
            "Кэш шрифтов",
            "FontCache служба остановится, кэш будет очищен и пересоздан.",
            false,
        ),
        (
            CleanupId::IconCache,
            "Кэш значков",
            "IconCache.db и iconcache_*.db в %LOCALAPPDATA%\\Microsoft\\Windows\\Explorer.",
            false,
        ),
        (
            CleanupId::ThumbnailCache,
            "Кэш миниатюр",
            "thumbcache_*.db в %LOCALAPPDATA%\\Microsoft\\Windows\\Explorer.",
            false,
        ),
        (
            CleanupId::DnsCache,
            "DNS-кэш",
            "ipconfig /flushdns — сбросить кэш DNS-резолвера.",
            false,
        ),
        (
            CleanupId::StoreCache,
            "Кэш Microsoft Store",
            "wsreset.exe — сброс кэша магазина приложений.",
            false,
        ),
        (
            CleanupId::SearchCache,
            "Кэш Windows Search",
            "%ProgramData%\\Microsoft\\Search\\Data\\Applications\\Windows. \
             Служба WSearch будет остановлена, индекс будет перестроен.",
            false,
        ),
        (
            CleanupId::CbsDismLogs,
            "Логи CBS и DISM",
            "C:\\Windows\\Logs\\CBS\\*.log и C:\\Windows\\Logs\\DISM\\*.log.",
            false,
        ),
        (
            CleanupId::PrintQueue,
            "Очередь печати",
            "C:\\Windows\\System32\\spool\\PRINTERS — застрявшие задания на печать. \
             Служба Spooler будет временно остановлена.",
            false,
        ),
        (
            CleanupId::RecentFiles,
            "Недавние документы",
            "%APPDATA%\\Microsoft\\Windows\\Recent — список недавно открытых файлов.",
            false,
        ),
        (
            CleanupId::EdgeCache,
            "Кэш Microsoft Edge",
            "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Cache.",
            false,
        ),
        (
            CleanupId::ChromeCache,
            "Кэш Google Chrome",
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache.",
            false,
        ),
        (
            CleanupId::FirefoxCache,
            "Кэш Mozilla Firefox",
            "%LOCALAPPDATA%\\Mozilla\\Firefox\\Profiles\\*\\cache2.",
            false,
        ),
        (
            CleanupId::WinSxSComponentCleanup,
            "Очистка WinSxS (компоненты)",
            "DISM /Online /Cleanup-Image /StartComponentCleanup /ResetBase. \
             После этого старые обновления нельзя будет удалить.",
            true,
        ),
        (
            CleanupId::OldRestorePoints,
            "Старые точки восстановления",
            "vssadmin delete shadows /for=C: /all /quiet — удалит ВСЕ теневые копии диска C:. \
             Откат к точке восстановления станет невозможен.",
            true,
        ),
        (
            CleanupId::HiberfilOff,
            "Отключить гибернацию (hiberfil.sys)",
            "powercfg -h off — освобождает место, равное размеру ОЗУ. \
             Гибернация и быстрый запуск Windows отключатся.",
            true,
        ),
    ];

    defs.iter()
        .map(|(id, title, desc, danger)| CleanupItem {
            id: *id,
            title: (*title).to_string(),
            description: (*desc).to_string(),
            size: CleanupSize::Unknown,
            danger: *danger,
            busy: false,
            log: None,
        })
        .collect()
}

const CLEANUP_SIZES_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$ProgressPreference = 'SilentlyContinue'

function Folder-Size {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return 0 }
    try {
        $sum = 0
        Get-ChildItem -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue |
            Where-Object { -not $_.PSIsContainer } |
            ForEach-Object { $sum += [int64]$_.Length }
        return [int64]$sum
    } catch { return 0 }
}

function Items-Size {
    param([string[]]$Paths)
    $sum = 0
    foreach ($p in $Paths) {
        $sum += Folder-Size $p
    }
    return [int64]$sum
}

function Glob-Size {
    param([string]$Pattern)
    $sum = 0
    try {
        Get-ChildItem -Path $Pattern -Force -ErrorAction SilentlyContinue |
            ForEach-Object {
                if ($_.PSIsContainer) { $sum += Folder-Size $_.FullName }
                else { $sum += [int64]$_.Length }
            }
    } catch {}
    return [int64]$sum
}

# Recycle Bin (all drives)
$rb = 0
try {
    foreach ($d in (Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue)) {
        $p = Join-Path $d.Root '$Recycle.Bin'
        $rb += Folder-Size $p
    }
} catch {}
"recyclebin=$rb"

"usertemp=" + (Items-Size @($env:TEMP, (Join-Path $env:LOCALAPPDATA 'Temp')))
"systemtemp=" + (Folder-Size 'C:\Windows\Temp')
"crashdumps=" + (Folder-Size (Join-Path $env:LOCALAPPDATA 'CrashDumps'))
"wer=" + (Items-Size @(
    (Join-Path $env:ProgramData 'Microsoft\Windows\WER'),
    (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\WER')
))

$dump = 0
if (Test-Path 'C:\Windows\MEMORY.DMP') { $dump += (Get-Item 'C:\Windows\MEMORY.DMP').Length }
$dump += Folder-Size 'C:\Windows\Minidump'
$dump += Folder-Size 'C:\Windows\LiveKernelReports'
"minidump=$dump"

"wuadl=" + (Folder-Size 'C:\Windows\SoftwareDistribution\Download')
"catroot2=" + (Folder-Size 'C:\Windows\System32\catroot2')
"deliveryopt=" + (Items-Size @(
    'C:\Windows\SoftwareDistribution\DeliveryOptimization',
    (Join-Path $env:ProgramData 'Microsoft\Windows\DeliveryOptimization\Cache')
))

"windowsold=" + (Folder-Size 'C:\Windows.old')

$leftovers = 0
foreach ($p in @('C:\$Windows.~BT','C:\$Windows.~WS','C:\$Windows.~LS','C:\ESD','C:\Windows\Panther')) {
    $leftovers += Folder-Size $p
}
"upgradeleft=$leftovers"

"lastgood=" + (Items-Size @('C:\Windows\LastGood','C:\Windows\LastGood.tmp'))
"prefetch=" + (Folder-Size 'C:\Windows\Prefetch')
"fontcache=" + (Folder-Size 'C:\Windows\ServiceProfiles\LocalService\AppData\Local\FontCache')

$icon = 0
$icon += Glob-Size (Join-Path $env:LOCALAPPDATA 'IconCache.db')
$icon += Glob-Size (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\Explorer\iconcache_*.db')
"iconcache=$icon"

"thumbcache=" + (Glob-Size (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\Explorer\thumbcache_*.db'))
"dnscache=-1"
"storecache=-1"
"searchcache=" + (Folder-Size (Join-Path $env:ProgramData 'Microsoft\Search\Data\Applications\Windows'))

$logs = 0
$logs += Glob-Size 'C:\Windows\Logs\CBS\*'
$logs += Glob-Size 'C:\Windows\Logs\DISM\*'
"cbsdism=$logs"

"printq=" + (Folder-Size 'C:\Windows\System32\spool\PRINTERS')
"recent=" + (Folder-Size (Join-Path $env:APPDATA 'Microsoft\Windows\Recent'))
"edgecache=" + (Folder-Size (Join-Path $env:LOCALAPPDATA 'Microsoft\Edge\User Data\Default\Cache'))
"chromecache=" + (Folder-Size (Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data\Default\Cache'))

$ff = 0
$ffRoot = Join-Path $env:LOCALAPPDATA 'Mozilla\Firefox\Profiles'
if (Test-Path $ffRoot) {
    Get-ChildItem -LiteralPath $ffRoot -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        $ff += Folder-Size (Join-Path $_.FullName 'cache2')
    }
}
"firefoxcache=$ff"

"winsxs=-1"

$rp = 0
try {
    foreach ($sc in (Get-CimInstance Win32_ShadowCopy -ErrorAction SilentlyContinue)) {
        # размер shadow copy не доступен напрямую — считаем количеством
        $rp += 1
    }
} catch {}
"restorepts=count:$rp"

$hib = 0
if (Test-Path 'C:\hiberfil.sys') {
    try { $hib = (Get-Item 'C:\hiberfil.sys' -Force).Length } catch {}
}
"hiberfil=$hib"
"#;

fn query_cleanup_sizes(logger: &Logger) -> Vec<(CleanupId, CleanupSize)> {
    let (ok, out) = run_powershell(CLEANUP_SIZES_SCRIPT, logger);
    if !ok {
        logger.log(
            LogLevel::Normal,
            &format!("Cleanup sizes query failed: {out}"),
        );
    }
    let mut result = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once('=') else { continue };
        let Some(id) = CleanupId::from_key(k.trim()) else { continue };
        let v = v.trim();
        let size = if v == "-1" {
            CleanupSize::NotApplicable
        } else if let Some(rest) = v.strip_prefix("count:") {
            match rest.parse::<u64>() {
                Ok(0) => CleanupSize::Bytes(0),
                // для точек восстановления показываем "псевдо-размер" как N единиц,
                // но реально мы не знаем сколько ГБ занимает каждая копия.
                Ok(_) => CleanupSize::NotApplicable,
                Err(_) => CleanupSize::Unknown,
            }
        } else {
            match v.parse::<u64>() {
                Ok(n) => CleanupSize::Bytes(n),
                Err(_) => CleanupSize::Unknown,
            }
        };
        result.push((id, size));
    }
    result
}

fn cleanup_script(id: CleanupId) -> &'static str {
    match id {
        CleanupId::RecycleBin => RECYCLEBIN_CLEAN,
        CleanupId::UserTemp => USERTEMP_CLEAN,
        CleanupId::SystemTemp => SYSTEMTEMP_CLEAN,
        CleanupId::CrashDumps => CRASHDUMPS_CLEAN,
        CleanupId::WerReports => WER_CLEAN,
        CleanupId::MinidumpAndLkr => MINIDUMP_CLEAN,
        CleanupId::SoftwareDistribution => WUADL_CLEAN,
        CleanupId::Catroot2 => CATROOT2_CLEAN,
        CleanupId::DeliveryOptimization => DELIVERYOPT_CLEAN,
        CleanupId::WindowsOld => WINDOWSOLD_CLEAN,
        CleanupId::UpgradeLeftovers => UPGRADELEFT_CLEAN,
        CleanupId::LastGood => LASTGOOD_CLEAN,
        CleanupId::Prefetch => PREFETCH_CLEAN,
        CleanupId::FontCache => FONTCACHE_CLEAN,
        CleanupId::IconCache => ICONCACHE_CLEAN,
        CleanupId::ThumbnailCache => THUMBCACHE_CLEAN,
        CleanupId::DnsCache => DNSCACHE_CLEAN,
        CleanupId::StoreCache => STORECACHE_CLEAN,
        CleanupId::SearchCache => SEARCHCACHE_CLEAN,
        CleanupId::CbsDismLogs => CBSDISM_CLEAN,
        CleanupId::PrintQueue => PRINTQ_CLEAN,
        CleanupId::RecentFiles => RECENT_CLEAN,
        CleanupId::EdgeCache => EDGECACHE_CLEAN,
        CleanupId::ChromeCache => CHROMECACHE_CLEAN,
        CleanupId::FirefoxCache => FIREFOXCACHE_CLEAN,
        CleanupId::WinSxSComponentCleanup => WINSXS_CLEAN,
        CleanupId::OldRestorePoints => RESTOREPTS_CLEAN,
        CleanupId::HiberfilOff => HIBERFIL_CLEAN,
    }
}

fn run_cleanup_op(id: CleanupId, logger: &Logger) -> (bool, String) {
    let script = cleanup_script(id);
    run_powershell(script, logger)
}

const RECYCLEBIN_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
try {
    Clear-RecycleBin -Force -ErrorAction Stop
    Write-Output 'Корзина очищена.'
} catch {
    foreach ($d in (Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue)) {
        $p = Join-Path $d.Root '$Recycle.Bin'
        if (Test-Path $p) {
            Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
                Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Output ('Корзина очищена (fallback). ' + $_.Exception.Message)
}
"#;

const USERTEMP_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
foreach ($p in @($env:TEMP, (Join-Path $env:LOCALAPPDATA 'Temp'))) {
    if (Test-Path $p) {
        Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Output 'Временные файлы пользователя очищены.'
"#;

const SYSTEMTEMP_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = 'C:\Windows\Temp'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'C:\Windows\Temp очищен.'
"#;

const CRASHDUMPS_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = Join-Path $env:LOCALAPPDATA 'CrashDumps'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'Дампы падений удалены.'
"#;

const WER_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
foreach ($p in @(
    (Join-Path $env:ProgramData 'Microsoft\Windows\WER'),
    (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\WER')
)) {
    if (Test-Path $p) {
        Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
            Where-Object { -not $_.PSIsContainer } |
            Remove-Item -Force -ErrorAction SilentlyContinue
    }
}
Write-Output 'WER отчёты удалены.'
"#;

const MINIDUMP_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Remove-Item 'C:\Windows\MEMORY.DMP' -Force -ErrorAction SilentlyContinue
foreach ($p in @('C:\Windows\Minidump','C:\Windows\LiveKernelReports')) {
    if (Test-Path $p) {
        Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Output 'MEMORY.DMP / Minidump / LiveKernelReports очищены.'
"#;

const WUADL_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Stop-Service -Name wuauserv -Force -ErrorAction SilentlyContinue
$p = 'C:\Windows\SoftwareDistribution\Download'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Start-Service -Name wuauserv -ErrorAction SilentlyContinue
Write-Output 'Кэш Windows Update (SoftwareDistribution\Download) очищен.'
"#;

const CATROOT2_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Stop-Service -Name cryptsvc -Force -ErrorAction SilentlyContinue
Stop-Service -Name bits -Force -ErrorAction SilentlyContinue
$p = 'C:\Windows\System32\catroot2'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Start-Service -Name bits -ErrorAction SilentlyContinue
Start-Service -Name cryptsvc -ErrorAction SilentlyContinue
Write-Output 'catroot2 очищен.'
"#;

const DELIVERYOPT_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Stop-Service -Name DoSvc -Force -ErrorAction SilentlyContinue
foreach ($p in @(
    'C:\Windows\SoftwareDistribution\DeliveryOptimization',
    (Join-Path $env:ProgramData 'Microsoft\Windows\DeliveryOptimization\Cache')
)) {
    if (Test-Path $p) {
        Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Start-Service -Name DoSvc -ErrorAction SilentlyContinue
Write-Output 'Delivery Optimization Cache очищен.'
"#;

const WINDOWSOLD_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = 'C:\Windows.old'
if (-not (Test-Path $p)) {
    Write-Output 'C:\Windows.old не найдена.'
    exit 0
}
try {
    takeown /F $p /R /D Y | Out-Null
    icacls $p /grant administrators:F /T /C /Q | Out-Null
} catch {}
Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue
if (Test-Path $p) {
    Write-Output 'Часть файлов не удалось удалить (нужны права TrustedInstaller). Используйте «Очистка диска» от имени администратора.'
    exit 1
}
Write-Output 'C:\Windows.old удалена.'
"#;

const UPGRADELEFT_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
foreach ($p in @('C:\$Windows.~BT','C:\$Windows.~WS','C:\$Windows.~LS','C:\ESD','C:\Windows\Panther')) {
    if (Test-Path $p) {
        try { takeown /F $p /R /D Y | Out-Null } catch {}
        try { icacls $p /grant administrators:F /T /C /Q | Out-Null } catch {}
        Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Output 'Остатки апгрейда удалены.'
"#;

const LASTGOOD_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
foreach ($p in @('C:\Windows\LastGood','C:\Windows\LastGood.tmp')) {
    if (Test-Path $p) {
        try { takeown /F $p /R /D Y | Out-Null } catch {}
        try { icacls $p /grant administrators:F /T /C /Q | Out-Null } catch {}
        Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Output 'LastGood / LastGood.tmp удалены.'
"#;

const PREFETCH_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = 'C:\Windows\Prefetch'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'Prefetch очищен.'
"#;

const FONTCACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Stop-Service -Name FontCache -Force -ErrorAction SilentlyContinue
Stop-Service -Name FontCache3.0.0.0 -Force -ErrorAction SilentlyContinue
$p = 'C:\Windows\ServiceProfiles\LocalService\AppData\Local\FontCache'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Remove-Item 'C:\Windows\System32\FNTCACHE.DAT' -Force -ErrorAction SilentlyContinue
Start-Service -Name FontCache -ErrorAction SilentlyContinue
Write-Output 'Кэш шрифтов очищен.'
"#;

const ICONCACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Remove-Item (Join-Path $env:LOCALAPPDATA 'IconCache.db') -Force -ErrorAction SilentlyContinue
Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\Explorer') -Filter 'iconcache_*.db' -Force -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue
Write-Output 'Кэш значков очищен (перезагрузка/перезапуск проводника применит изменения).'
"#;

const THUMBCACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\Explorer') -Filter 'thumbcache_*.db' -Force -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue
Write-Output 'Кэш миниатюр очищен.'
"#;

const DNSCACHE_CLEAN: &str = r#"
ipconfig /flushdns | Out-Null
Write-Output 'DNS-кэш сброшен.'
"#;

const STORECACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Start-Process -FilePath 'wsreset.exe' -ArgumentList '-i' -WindowStyle Hidden -ErrorAction SilentlyContinue
Write-Output 'Запущен wsreset.exe — кэш Microsoft Store будет сброшен.'
"#;

const SEARCHCACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Stop-Service -Name WSearch -Force -ErrorAction SilentlyContinue
$p = Join-Path $env:ProgramData 'Microsoft\Search\Data\Applications\Windows'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Start-Service -Name WSearch -ErrorAction SilentlyContinue
Write-Output 'Кэш Windows Search очищен (индекс будет перестроен).'
"#;

const CBSDISM_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
foreach ($p in @('C:\Windows\Logs\CBS','C:\Windows\Logs\DISM')) {
    if (Test-Path $p) {
        Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Output 'Логи CBS и DISM очищены.'
"#;

const PRINTQ_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
Stop-Service -Name Spooler -Force -ErrorAction SilentlyContinue
$p = 'C:\Windows\System32\spool\PRINTERS'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Start-Service -Name Spooler -ErrorAction SilentlyContinue
Write-Output 'Очередь печати очищена.'
"#;

const RECENT_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = Join-Path $env:APPDATA 'Microsoft\Windows\Recent'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'Список недавних документов очищен.'
"#;

const EDGECACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = Join-Path $env:LOCALAPPDATA 'Microsoft\Edge\User Data\Default\Cache'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'Кэш Microsoft Edge очищен.'
"#;

const CHROMECACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$p = Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data\Default\Cache'
if (Test-Path $p) {
    Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'Кэш Google Chrome очищен.'
"#;

const FIREFOXCACHE_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$root = Join-Path $env:LOCALAPPDATA 'Mozilla\Firefox\Profiles'
if (Test-Path $root) {
    Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        $c = Join-Path $_.FullName 'cache2'
        if (Test-Path $c) {
            Get-ChildItem -LiteralPath $c -Recurse -Force -ErrorAction SilentlyContinue |
                Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}
Write-Output 'Кэш Mozilla Firefox очищен.'
"#;

const WINSXS_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$out = (Dism.exe /Online /Cleanup-Image /StartComponentCleanup /ResetBase 2>&1 | Out-String)
Write-Output $out
"#;

const RESTOREPTS_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$out = (vssadmin.exe delete shadows /for=C: /all /quiet 2>&1 | Out-String)
Write-Output $out
"#;

const HIBERFIL_CLEAN: &str = r#"
$ErrorActionPreference='Continue'
$out = (powercfg.exe /h off 2>&1 | Out-String)
if (-not $out.Trim()) { $out = 'powercfg /h off выполнено.' }
Write-Output $out
"#;
