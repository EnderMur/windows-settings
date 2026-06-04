use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use crate::time_win::{appdata_logs_dir, local_timestamp_filename, local_timestamp_pretty};

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum LogLevel {
    #[default]
    Normal,
    Debug,
}

pub struct Logger {
    pub file: Mutex<Option<File>>,
    pub path: PathBuf,
    level: AtomicU8,
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger {
    pub fn new() -> Self {
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

    pub fn current_level(&self) -> LogLevel {
        match self.level.load(Ordering::Relaxed) {
            1 => LogLevel::Debug,
            _ => LogLevel::Normal,
        }
    }

    pub fn set_level(&self, level: LogLevel) {
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

    pub fn log(&self, msg_level: LogLevel, msg: &str) {

        if msg_level == LogLevel::Debug && self.current_level() != LogLevel::Debug {
            return;
        }
        let stamp = local_timestamp_pretty();
        let line = format!("[{stamp}] [{:?}] {msg}\n", msg_level);
        if let Ok(mut guard) = self.file.lock()
            && let Some(f) = guard.as_mut() {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
    }
}