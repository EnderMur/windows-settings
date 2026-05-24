#[derive(Clone, Copy)]
pub enum Status {
    Unknown,
    Installed,
    NotInstalled,
}

pub struct Card {
    pub title: String,
    pub description: String,
    pub package: String,
    pub status: Status,
    pub busy: bool,
    pub log: Option<String>,
}

pub struct NavItem {
    pub icon: &'static str,
    pub label: &'static str,
    pub beta: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Home,
    Uwp,
    Telemetry,
    Memory,
    Cleanup,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CleanupId {
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
    pub fn key(self) -> &'static str {
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

    pub fn from_key(s: &str) -> Option<Self> {
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
pub enum CleanupSize {
    Unknown,
    NotApplicable,
    Bytes(u64),
}

pub struct CleanupItem {
    pub id: CleanupId,
    pub title: String,
    pub description: String,
    pub size: CleanupSize,
    pub danger: bool,
    pub busy: bool,
    pub log: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TelemetryStatus {
    Unknown,
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TelemetryId {
    Office,
    Firefox,
    Chrome,
    Nvidia,
    VisualStudio,
    Windows,
}

impl TelemetryId {
    pub fn from_key(s: &str) -> Option<Self> {
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

pub struct TelemetryItem {
    pub id: TelemetryId,
    pub title: String,
    pub description: String,
    pub status: TelemetryStatus,
    pub busy: bool,
    pub log: Option<String>,
}

#[derive(Clone, Default)]
pub struct SysInfo {
    pub os: String,
    pub build: String,
    pub arch: String,
    pub hostname: String,
    pub user: String,
    pub is_admin: Option<bool>,
    pub cpu: String,
    pub gpu: String,
    pub ram_gb: String,
}

pub enum Msg {
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
pub struct MemInfo {
    pub total_bytes: u64,
    pub avail_bytes: u64,
    pub standby_bytes: u64,
    pub modified_bytes: u64,
    pub free_bytes: u64,
    pub memory_load: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemOp {
    PurgeStandby,
    PurgeLowPriorityStandby,
    EmptyWorkingSets,
    FlushModified,
}

impl MemOp {
    pub fn command(self) -> i32 {

        match self {
            MemOp::EmptyWorkingSets => 2,
            MemOp::FlushModified => 3,
            MemOp::PurgeStandby => 4,
            MemOp::PurgeLowPriorityStandby => 5,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            MemOp::PurgeStandby => "Очистить ожидающую память",
            MemOp::PurgeLowPriorityStandby => "Очистить low-priority standby",
            MemOp::EmptyWorkingSets => "Сбросить рабочие наборы",
            MemOp::FlushModified => "Сбросить изменённые страницы",
        }
    }

    pub fn description(self) -> &'static str {
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
pub enum UpdateState {
    Idle,
    Checking,
    Installing,
    UpToDate { latest: String },
    Available { latest: String },
    Done { from: String, to: String },
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_id_key_roundtrip() {
        let ids = [
            CleanupId::RecycleBin, CleanupId::UserTemp, CleanupId::SystemTemp,
            CleanupId::CrashDumps, CleanupId::WerReports, CleanupId::MinidumpAndLkr,
            CleanupId::SoftwareDistribution, CleanupId::Catroot2, CleanupId::DeliveryOptimization,
            CleanupId::WindowsOld, CleanupId::UpgradeLeftovers, CleanupId::LastGood,
            CleanupId::Prefetch, CleanupId::FontCache, CleanupId::IconCache,
            CleanupId::ThumbnailCache, CleanupId::DnsCache, CleanupId::StoreCache,
            CleanupId::SearchCache, CleanupId::CbsDismLogs, CleanupId::PrintQueue,
            CleanupId::RecentFiles, CleanupId::EdgeCache, CleanupId::ChromeCache,
            CleanupId::FirefoxCache, CleanupId::WinSxSComponentCleanup,
            CleanupId::OldRestorePoints, CleanupId::HiberfilOff,
        ];
        for id in ids {
            let key = id.key();
            assert!(!key.is_empty(), "key should not be empty for {:?}", id);
            assert_eq!(CleanupId::from_key(key), Some(id), "roundtrip failed for {:?}", id);
        }
    }

    #[test]
    fn test_cleanup_id_from_key_invalid() {
        assert_eq!(CleanupId::from_key("nonexistent"), None);
        assert_eq!(CleanupId::from_key(""), None);
    }

    #[test]
    fn test_telemetry_id_from_key() {
        assert_eq!(TelemetryId::from_key("office"), Some(TelemetryId::Office));
        assert_eq!(TelemetryId::from_key("firefox"), Some(TelemetryId::Firefox));
        assert_eq!(TelemetryId::from_key("chrome"), Some(TelemetryId::Chrome));
        assert_eq!(TelemetryId::from_key("nvidia"), Some(TelemetryId::Nvidia));
        assert_eq!(TelemetryId::from_key("vs"), Some(TelemetryId::VisualStudio));
        assert_eq!(TelemetryId::from_key("windows"), Some(TelemetryId::Windows));
        assert_eq!(TelemetryId::from_key("unknown"), None);
    }

    #[test]
    fn test_mem_op_commands() {
        assert_eq!(MemOp::EmptyWorkingSets.command(), 2);
        assert_eq!(MemOp::FlushModified.command(), 3);
        assert_eq!(MemOp::PurgeStandby.command(), 4);
        assert_eq!(MemOp::PurgeLowPriorityStandby.command(), 5);
    }

    #[test]
    fn test_mem_op_titles_not_empty() {
        for op in [MemOp::PurgeStandby, MemOp::PurgeLowPriorityStandby, MemOp::EmptyWorkingSets, MemOp::FlushModified] {
            assert!(!op.title().is_empty());
            assert!(!op.description().is_empty());
        }
    }
}
