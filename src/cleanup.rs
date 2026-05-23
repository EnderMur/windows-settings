use crate::logger::{Logger, LogLevel};
use crate::powershell::run_powershell;
use crate::types::{CleanupId, CleanupItem, CleanupSize};

pub fn cleanup_items() -> Vec<CleanupItem> {
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

pub fn query_cleanup_sizes(logger: &Logger) -> Vec<(CleanupId, CleanupSize)> {
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

pub fn cleanup_script(id: CleanupId) -> &'static str {
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

pub fn run_cleanup_op(id: CleanupId, logger: &Logger) -> (bool, String) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_items_count() {
        let items = cleanup_items();
        assert_eq!(items.len(), 28, "expected 28 cleanup categories, got {}", items.len());
    }

    #[test]
    fn test_cleanup_items_all_ids_covered() {
        let items = cleanup_items();
        for id in [
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
        ] {
            assert!(items.iter().any(|i| i.id == id), "missing CleanupId {:?}", id);
        }
    }

    #[test]
    fn test_cleanup_script_all_ids_covered() {
        for id in [
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
        ] {
            let script = cleanup_script(id);
            assert!(!script.is_empty(), "empty cleanup script for {:?}", id);
        }
    }

    #[test]
    fn test_dangerous_items() {
        let items = cleanup_items();
        let danger_ids: Vec<CleanupId> = items.iter().filter(|i| i.danger).map(|i| i.id).collect();
        assert!(danger_ids.contains(&CleanupId::WindowsOld), "WindowsOld should be dangerous");
        assert!(danger_ids.contains(&CleanupId::WinSxSComponentCleanup), "WinSxS should be dangerous");
        assert!(danger_ids.contains(&CleanupId::OldRestorePoints), "OldRestorePoints should be dangerous");
        assert!(danger_ids.contains(&CleanupId::HiberfilOff), "HiberfilOff should be dangerous");
        assert_eq!(danger_ids.len(), 4, "expected exactly 4 dangerous items, got {}", danger_ids.len());
    }
}
