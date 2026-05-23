use std::path::PathBuf;

pub struct SystemTimeWin {
    pub w_year: u16,
    pub w_month: u16,
    pub w_day_of_week: u16,
    pub w_day: u16,
    pub w_hour: u16,
    pub w_minute: u16,
    pub w_second: u16,
    pub w_milliseconds: u16,
}

unsafe extern "system" {
    pub fn GetLocalTime(lp_system_time: *mut SystemTimeWin);
}

pub fn get_local_time() -> SystemTimeWin {
    let mut st = SystemTimeWin::default();
    unsafe { GetLocalTime(&mut st) };
    st
}

pub fn local_timestamp_filename() -> String {
    let t = get_local_time();
    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        t.w_year, t.w_month, t.w_day, t.w_hour, t.w_minute, t.w_second
    )
}

pub fn local_timestamp_pretty() -> String {
    let t = get_local_time();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.w_year, t.w_month, t.w_day, t.w_hour, t.w_minute, t.w_second, t.w_milliseconds
    )
}


pub fn appdata_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("WindowsSettings")
}

pub fn appdata_logs_dir() -> PathBuf { appdata_dir().join("logs") }
pub fn appdata_config_path() -> PathBuf { appdata_dir().join("config.json") }
pub fn appdata_settings_path() -> PathBuf { appdata_dir().join("settings.conf") }
