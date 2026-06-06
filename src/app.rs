use crate::time_win::{appdata_config_path, appdata_settings_path};

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use eframe::egui;

use crate::cleanup::*;
use crate::config::{AppSettings, Config, load_config, load_settings, save_config, save_settings};
use crate::icons;
use crate::logger::{LogLevel, Logger};
use crate::memory::*;
use crate::services::{query_service_status, run_service_op, service_items};
use crate::system::collect_sys_info;
use crate::telemetry::{query_telemetry_status, run_telemetry_op, telemetry_items};
use crate::types::*;
use crate::types::{ServiceItem, ServiceStatus};
use crate::ui::*;
use crate::update::format_bytes;
use crate::update::*;
use crate::uwp::*;

pub struct App {
    pub view: View,
    pub nav_items: Vec<NavItem>,
    pub cards: Vec<Card>,
    pub telemetry: Vec<TelemetryItem>,
    pub tx: Sender<Msg>,
    pub rx: Receiver<Msg>,
    pub logger: Arc<Logger>,
    pub settings: AppSettings,
    pub update_state: UpdateState,
    pub sys_info: Option<SysInfo>,
    pub config: Config,
    pub show_token_dialog: bool,
    pub token_input: String,
    pub token_dialog_error: Option<String>,
    pub mem_info: Option<MemInfo>,
    pub mem_busy: bool,
    pub mem_log: Option<String>,
    pub mem_refresh_in_flight: bool,
    pub mem_last_refresh: Option<std::time::Instant>,
    pub cleanup_items: Vec<CleanupItem>,
    pub cleanup_refresh_in_flight: bool,
    pub cleanup_sizes_loaded: bool,
    pub services: Vec<ServiceItem>,
    pub tasks: Vec<TaskEntry>,
}

const REPO_OWNER: &str = "EnderMur";
const REPO_NAME: &str = "windows-settings";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

impl App {
    pub fn new(logger: Arc<Logger>) -> Self {
        let (tx, rx) = channel();
        let config = load_config(&logger);
        let settings = load_settings(&logger);
        logger.set_level(settings.log_level);
        Self {
            view: View::Home,
            nav_items: vec![
                NavItem {
                    icon: icons::HOME_PNG,
                    label: "Главная",
                    beta: false,
                },
                NavItem {
                    icon: icons::UWP_PNG,
                    label: "UWP приложения",
                    beta: false,
                },
                NavItem {
                    icon: icons::TELEMETRY_PNG,
                    label: "Телеметрия",
                    beta: false,
                },
                NavItem {
                    icon: icons::MEMORY_PNG,
                    label: "ОЗУ",
                    beta: true,
                },
                NavItem {
                    icon: icons::CLEANUP_PNG,
                    label: "Очистка",
                    beta: true,
                },
                NavItem {
                    icon: icons::SERVICES_PNG,
                    label: "Службы",
                    beta: true,
                },
            ],
            cards: uwp_apps(),
            telemetry: telemetry_items(),
            cleanup_items: cleanup_items(),
            cleanup_refresh_in_flight: false,
            cleanup_sizes_loaded: false,
            services: service_items(),
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
            tasks: Vec::new(),
        }
    }

    pub fn spawn_initial_status_check(&mut self, ctx: egui::Context) {
        let tx = self.tx.clone();
        let packages: Vec<String> = self.cards.iter().map(|c| c.package.clone()).collect();
        let logger = self.logger.clone();
        let ctx1 = ctx.clone();
        logger.log(
            LogLevel::Normal,
            &format!("Initial status check for {} packages", packages.len()),
        );
        thread::spawn(move || {
            let mut task = TaskEntry::new("UWP: проверка установленных пакетов");
            task.log = format!("Запрос статуса {} пакетов...", packages.len());
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx1.request_repaint();

            let installed = query_installed_packages(&packages, &logger);
            logger.log(
                LogLevel::Normal,
                &format!(
                    "Status check done: {} installed, {} not installed",
                    installed.iter().filter(|(_, p)| *p).count(),
                    installed.iter().filter(|(_, p)| !*p).count()
                ),
            );
            let mut task = TaskEntry::new("UWP: проверка установленных пакетов");
            task.status = TaskStatus::Done;
            task.log = format!(
                "Готово: {} установлено, {} не установлено",
                installed.iter().filter(|(_, p)| *p).count(),
                installed.iter().filter(|(_, p)| !*p).count()
            );
            let _ = tx.send(Msg::TaskUpdate(task));
            let _ = tx.send(Msg::BulkStatus(installed));
            ctx1.request_repaint();
        });

        let tx = self.tx.clone();
        let logger = self.logger.clone();
        let ctx2 = ctx.clone();
        logger.log(LogLevel::Normal, "Initial telemetry status check");
        thread::spawn(move || {
            let mut task = TaskEntry::new("Телеметрия: проверка состояния");
            task.log = "Запрос состояния телеметрии...".into();
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx2.request_repaint();

            let statuses = query_telemetry_status(&logger);
            logger.log(
                LogLevel::Normal,
                &format!("Telemetry status check done: {} entries", statuses.len()),
            );
            let mut task = TaskEntry::new("Телеметрия: проверка состояния");
            task.status = TaskStatus::Done;
            task.log = format!("Готово: {} записей проверено", statuses.len());
            let _ = tx.send(Msg::TaskUpdate(task));
            let _ = tx.send(Msg::TelemetryBulkStatus(statuses));
            ctx2.request_repaint();
        });

        let tx = self.tx.clone();
        let logger = self.logger.clone();
        let ctx3 = ctx.clone();
        logger.log(LogLevel::Normal, "Initial system info collection");
        thread::spawn(move || {
            let mut task = TaskEntry::new("Система: сбор информации");
            task.log = "Сбор сведений о системе...".into();
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx3.request_repaint();

            let info = collect_sys_info(&logger);
            logger.log(
                LogLevel::Normal,
                &format!(
                    "System info collected: os='{}', build='{}', cpu='{}', gpu='{}'",
                    info.os, info.build, info.cpu, info.gpu
                ),
            );
            let mut task = TaskEntry::new("Система: сбор информации");
            task.status = TaskStatus::Done;
            task.log = format!("Готово: OS={}, CPU={}", info.os, info.cpu);
            let _ = tx.send(Msg::TaskUpdate(task));
            let _ = tx.send(Msg::SysInfoReady(info));
            ctx3.request_repaint();
        });

        self.start_update_check(ctx.clone());

        let tx = self.tx.clone();
        let logger = self.logger.clone();
        let ctx4 = ctx.clone();
        logger.log(LogLevel::Normal, "Initial services status check");
        thread::spawn(move || {
            let mut task = TaskEntry::new("Службы: проверка состояния");
            task.log = "Запрос состояния служб...".into();
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx4.request_repaint();

            let statuses = query_service_status(logger.as_ref());
            logger.log(
                LogLevel::Normal,
                &format!("Services status check done: {} entries", statuses.len()),
            );
            let mut task = TaskEntry::new("Службы: проверка состояния");
            task.status = TaskStatus::Done;
            task.log = format!("Готово: {} служб проверено", statuses.len());
            let _ = tx.send(Msg::TaskUpdate(task));
            let _ = tx.send(Msg::ServiceBulkStatus(statuses));
            ctx4.request_repaint();
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
                Msg::TelemetryBulkStatus(list) => {
                    for (id, status) in list {
                        if let Some(item) = self.telemetry.iter_mut().find(|t| t.id == id) {
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
                        if let Some(item) = self.cleanup_items.iter_mut().find(|c| c.id == id) {
                            item.size = size;
                        }
                    }
                    self.cleanup_refresh_in_flight = false;
                    self.cleanup_sizes_loaded = true;
                }
                Msg::CleanupOpDone { id, new_size, log } => {
                    if let Some(item) = self.cleanup_items.iter_mut().find(|c| c.id == id) {
                        item.busy = false;
                        item.log = Some(log);
                        item.size = new_size;
                    }
                }
                Msg::ServiceBulkStatus(list) => {
                    for (id, status) in list {
                        if let Some(item) = self.services.iter_mut().find(|s| s.id == id) {
                            item.status = status;
                        }
                    }
                }
                Msg::ServiceOpDone {
                    id,
                    new_status,
                    log,
                } => {
                    if let Some(item) = self.services.iter_mut().find(|s| s.id == id) {
                        item.status = new_status;
                        item.busy = false;
                        item.log = Some(log);
                    }
                }
                Msg::TaskUpdate(entry) => {
                    if let Some(existing) = self.tasks.iter_mut().find(|t| t.name == entry.name) {
                        existing.status = entry.status;
                        existing.log = entry.log;
                    } else {
                        self.tasks.push(entry);
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
                if token.is_some() {
                    "token"
                } else {
                    "anonymous"
                }
            ),
        );
        thread::spawn(move || {
            let mut task = TaskEntry::new("Обновление: проверка GitHub");
            task.log = "Запрос последнего релиза...".into();
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx.request_repaint();

            let result = check_latest_release(&logger, token.as_deref());
            let state = match result {
                Ok(latest) => {
                    if is_newer(&latest, APP_VERSION) {
                        logger.log(
                            LogLevel::Normal,
                            &format!("Update available: {APP_VERSION} -> {latest}"),
                        );
                        let mut task = TaskEntry::new("Обновление: проверка GitHub");
                        task.status = TaskStatus::Done;
                        task.log = format!("Доступна версия {latest}");
                        let _ = tx.send(Msg::TaskUpdate(task));
                        UpdateState::Available { latest }
                    } else {
                        logger.log(LogLevel::Normal, &format!("Up to date ({latest})"));
                        let mut task = TaskEntry::new("Обновление: проверка GitHub");
                        task.status = TaskStatus::Done;
                        task.log = format!("Актуальная версия: {latest}");
                        let _ = tx.send(Msg::TaskUpdate(task));
                        UpdateState::UpToDate { latest }
                    }
                }
                Err(e) => {
                    logger.log(LogLevel::Normal, &format!("Update check failed: {e}"));
                    let mut task = TaskEntry::new("Обновление: проверка GitHub");
                    task.status = TaskStatus::Failed;
                    task.log = format!("Ошибка: {e}");
                    let _ = tx.send(Msg::TaskUpdate(task));
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
                if token.is_some() {
                    "token"
                } else {
                    "anonymous"
                }
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
        let Some(card) = self.cards.get_mut(idx) else {
            return;
        };
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
        let Some(card) = self.cards.get_mut(idx) else {
            return;
        };
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
        logger.log(LogLevel::Normal, &format!("Memory op requested: {:?}", id));
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
        let Some(item) = self.telemetry.iter_mut().find(|t| t.id == id) else {
            return;
        };
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
            let mut task = TaskEntry::new("Очистка: подсчёт размеров");
            task.log = "Подсчёт размеров категорий...".into();
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx.request_repaint();

            let sizes = query_cleanup_sizes(&logger);
            logger.log(
                LogLevel::Normal,
                &format!("Cleanup sizes refresh done: {} entries", sizes.len()),
            );
            let mut task = TaskEntry::new("Очистка: подсчёт размеров");
            task.status = TaskStatus::Done;
            task.log = format!("Готово: {} категорий подсчитано", sizes.len());
            let _ = tx.send(Msg::TaskUpdate(task));
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
        logger.log(LogLevel::Normal, &format!("Cleanup op requested: {:?}", id));
        thread::spawn(move || {
            let mut task = TaskEntry::new(&format!("Очистка: {:?}", id));
            task.log = "Выполнение операции очистки...".into();
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx.request_repaint();

            let (ok, out) = run_cleanup_op(id, &logger);
            logger.log(
                LogLevel::Normal,
                &format!("Cleanup op {:?} result: ok={ok}, output={out}", id),
            );
            let mut task = TaskEntry::new(&format!("Очистка: {:?}", id));
            task.status = if ok {
                TaskStatus::Done
            } else {
                TaskStatus::Failed
            };
            task.log = if ok {
                "Готово".into()
            } else {
                format!("Ошибка: {out}")
            };
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx.request_repaint();

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
            let _ = tx.send(Msg::CleanupOpDone { id, new_size, log });
            let _ = tx.send(Msg::CleanupSizesReady(sizes));
            ctx.request_repaint();
        });
    }

    fn start_service_op(&mut self, id: ServiceId, disable: bool, ctx: egui::Context) {
        let Some(item) = self.services.iter_mut().find(|s| s.id == id) else {
            return;
        };
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
                "Service {} requested: {:?}",
                if disable { "disable" } else { "enable" },
                id
            ),
        );
        thread::spawn(move || {
            let op_name = if disable {
                "отключение"
            } else {
                "включение"
            };
            let mut task = TaskEntry::new(&format!("Служба {:?}: {}", id, op_name));
            task.log = format!(
                "{} службы {:?}...",
                if disable {
                    "Отключение"
                } else {
                    "Включение"
                },
                id
            );
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx.request_repaint();

            let (ok, out) = run_service_op(id, disable, &logger);
            logger.log(
                LogLevel::Normal,
                &format!(
                    "Service {} result for {:?}: ok={ok}, output={out}",
                    if disable { "disable" } else { "enable" },
                    id
                ),
            );
            let mut task = TaskEntry::new(&format!("Служба {:?}: {}", id, op_name));
            task.status = if ok {
                TaskStatus::Done
            } else {
                TaskStatus::Failed
            };
            task.log = if ok {
                "Готово".into()
            } else {
                format!("Ошибка: {out}")
            };
            let _ = tx.send(Msg::TaskUpdate(task));
            ctx.request_repaint();

            let new_status = if ok {
                if disable {
                    ServiceStatus::Disabled
                } else {
                    ServiceStatus::Running
                }
            } else if disable {
                ServiceStatus::Running
            } else {
                ServiceStatus::Disabled
            };
            let _ = tx.send(Msg::ServiceOpDone {
                id,
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
                                let target = match i {
                                    0 => View::Home,
                                    1 => View::Uwp,
                                    2 => View::Telemetry,
                                    3 => View::Memory,
                                    4 => View::Cleanup,
                                    5 => View::Services,
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
                            icon: icons::SETTINGS_PNG,
                            label: "Настройки",
                            beta: false,
                        };
                        let selected = self.view == View::Settings;
                        if nav_button(ui, &settings_item, selected).clicked() {
                            self.view = View::Settings;
                            self.logger.log(LogLevel::Debug, "View switched: Settings");
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
                    View::Services => "Службы",
                    View::Settings => "Настройки",
                };
                let beta_view = matches!(self.view, View::Memory | View::Cleanup | View::Services);
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
                    View::Services => self.draw_services(ui, &ctx),
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
                        UpdateState::Error(e) => {
                            (e.clone(), egui::Color32::from_rgb(220, 120, 120))
                        }
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
                            egui::RichText::new("Используется сохранённый GitHub-токен.")
                                .size(11.0)
                                .italics()
                                .color(egui::Color32::from_gray(140)),
                        );
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if let UpdateState::Available { .. } = self.update_state {
                            let btn = egui::Button::new(egui::RichText::new("Обновить").size(13.0))
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
                            let btn =
                                egui::Button::new(egui::RichText::new("Добавить токен").size(13.0))
                                    .min_size(egui::vec2(150.0, 32.0))
                                    .fill(egui::Color32::from_rgb(120, 90, 56));
                            if ui.add_enabled(!busy, btn).clicked() {
                                self.token_input =
                                    self.config.github_token.clone().unwrap_or_default();
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
                    let save_btn = egui::Button::new(egui::RichText::new("Сохранить").size(13.0))
                        .min_size(egui::vec2(110.0, 30.0))
                        .fill(egui::Color32::from_rgb(56, 130, 90));
                    if ui.add(save_btn).clicked() {
                        save = true;
                    }

                    let cancel_btn = egui::Button::new(egui::RichText::new("Отмена").size(13.0))
                        .min_size(egui::vec2(110.0, 30.0));
                    if ui.add(cancel_btn).clicked() {
                        close = true;
                    }

                    if self.config.github_token.is_some() {
                        let clear_btn =
                            egui::Button::new(egui::RichText::new("Удалить").size(13.0))
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
                self.token_dialog_error = Some("Поле токена пустое.".to_string());
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
                        self.token_dialog_error = Some(format!("Не удалось сохранить: {e}"));
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
                    self.token_dialog_error = Some(format!("Не удалось сохранить: {e}"));
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
                            &format!("{}  ({}%)", format_bytes(used), info.memory_load),
                        );
                        info_row(ui, "Доступно", &format_bytes(info.avail_bytes));
                        info_row(ui, "Свободно", &format_bytes(info.free_bytes));
                        info_row(ui, "Standby (ожидание)", &format_bytes(info.standby_bytes));
                        info_row(ui, "Modified", &format_bytes(info.modified_bytes));
                        ui.add_space(6.0);

                        let bar_h = 10.0;
                        let bar_w = ui.available_width().min(420.0);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(bar_w, bar_h), egui::Sense::hover());
                        let total = info.total_bytes.max(1) as f32;
                        let used_w = bar_w * (used as f32 / total);
                        let standby_w = bar_w * (info.standby_bytes as f32 / total);
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(4),
                            egui::Color32::from_rgb(48, 48, 56),
                        );
                        let used_rect =
                            egui::Rect::from_min_size(rect.left_top(), egui::vec2(used_w, bar_h));
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
                        let btn = egui::Button::new(egui::RichText::new("Обновить").size(13.0))
                            .min_size(egui::vec2(140.0, 30.0))
                            .fill(egui::Color32::from_rgb(56, 90, 170));
                        if ui.add_enabled(!self.mem_refresh_in_flight, btn).clicked() {
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
                                egui::RichText::new(format!("обновлено {secs} с назад"))
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
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
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
            let btn = egui::Button::new(egui::RichText::new("Пересчитать размеры").size(13.0))
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

    fn draw_services(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut to_disable: Option<ServiceId> = None;
        let mut to_enable: Option<ServiceId> = None;

        ui.label(
            egui::RichText::new(
                "Управление сервисами Windows. Требуются права администратора.\n\
                 Отключение ненужных служб снижает фоновую нагрузку и повышает приватность.",
            )
            .size(12.0)
            .color(egui::Color32::from_gray(170)),
        );
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for item in &self.services {
                    match draw_service_card(ui, item) {
                        ServiceAction::None => {}
                        ServiceAction::Disable => to_disable = Some(item.id),
                        ServiceAction::Enable => to_enable = Some(item.id),
                    }
                    ui.add_space(8.0);
                }
            });

        if let Some(id) = to_disable {
            self.start_service_op(id, true, ctx.clone());
        }
        if let Some(id) = to_enable {
            self.start_service_op(id, false, ctx.clone());
        }
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.settings.show_hidden_features {
                    info_card(ui, "Задачи", |ui| {
                        if self.tasks.is_empty() {
                            ui.label(
                                egui::RichText::new("Нет выполненных задач.")
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(150)),
                            );
                        } else {
                            for task in &self.tasks {
                                let (status_label, status_color) = match task.status {
                                    TaskStatus::Running => {
                                        ("Выполняется", egui::Color32::from_rgb(220, 180, 100))
                                    }
                                    TaskStatus::Done => {
                                        ("Готово", egui::Color32::from_rgb(120, 200, 140))
                                    }
                                    TaskStatus::Failed => {
                                        ("Ошибка", egui::Color32::from_rgb(220, 120, 120))
                                    }
                                };
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&task.name)
                                            .size(13.0)
                                            .strong()
                                            .color(egui::Color32::from_gray(225)),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("• {status_label}"))
                                            .size(11.0)
                                            .color(status_color),
                                    );
                                });
                                if !task.log.is_empty() {
                                    ui.label(
                                        egui::RichText::new(&task.log)
                                            .size(11.0)
                                            .italics()
                                            .color(egui::Color32::from_gray(150)),
                                    );
                                }
                                ui.add_space(4.0);
                            }
                        }
                    });
                    ui.add_space(10.0);
                }

                setting_row(
                    ui,
                    "Отображать скрытые функции",
                    "Показывает панель задач и дополнительные диагностические секции.",
                    |ui| {
                        let mut checked = self.settings.show_hidden_features;
                        if ui.checkbox(&mut checked, "").changed() {
                            self.settings.show_hidden_features = checked;
                            if let Err(e) = save_settings(&self.settings, &self.logger) {
                                self.logger.log(
                                    LogLevel::Normal,
                                    &format!("Не удалось сохранить settings.conf: {e}"),
                                );
                            }
                        }
                    },
                );
                ui.add_space(10.0);

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
                        if let Some(level) = new_level
                            && level != self.settings.log_level
                        {
                            self.settings.log_level = level;
                            self.logger.set_level(level);
                            if let Err(e) = save_settings(&self.settings, &self.logger) {
                                self.logger.log(
                                    LogLevel::Normal,
                                    &format!("Не удалось сохранить settings.conf: {e}"),
                                );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new_initial_state() {
        let logger = Arc::new(Logger::new());
        let app = App::new(logger);

        assert!(matches!(app.view, View::Home));
        assert_eq!(app.nav_items.len(), 5);
        assert_eq!(app.cards.len(), uwp_apps().len());
        assert_eq!(app.telemetry.len(), telemetry_items().len());
        assert_eq!(app.cleanup_items.len(), cleanup_items().len());
        assert!(matches!(app.update_state, UpdateState::Idle));
        assert!(app.sys_info.is_none());
        assert!(app.mem_info.is_none());
        assert!(!app.mem_busy);
        assert!(!app.mem_refresh_in_flight);
        assert!(!app.cleanup_refresh_in_flight);
        assert!(!app.cleanup_sizes_loaded);
        assert!(!app.show_token_dialog);
        assert!(app.token_input.is_empty());
        assert!(app.token_dialog_error.is_none());
    }
}
