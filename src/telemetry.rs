use std::fs;

use crate::types::{
    CleanupId,
    CleanupItem,
    CleanupSize,
    MemInfo,
    MemOp,
    UpdateState,
};

use crate::update::{
    APP_VERSION,
    REPO_NAME,
    REPO_OWNER,
};

use crate::time_win::local_timestamp_filename;

use crate::logger::{Logger, LogLevel};
use crate::powershell::run_powershell;
use crate::types::{TelemetryId, TelemetryItem, TelemetryStatus};

pub fn telemetry_items() -> Vec<TelemetryItem> {
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

pub fn query_telemetry_status(logger: &Logger) -> Vec<(TelemetryId, TelemetryStatus)> {
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

pub fn run_telemetry_op(id: TelemetryId, disable: bool, logger: &Logger) -> (bool, String) {
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

pub fn format_bytes(bytes: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_items_count() {
        let items = telemetry_items();
        assert_eq!(items.len(), 6, "expected 6 telemetry categories");
    }

    #[test]
    fn test_telemetry_items_all_ids() {
        let items = telemetry_items();
        let ids: Vec<TelemetryId> = items.iter().map(|i| i.id).collect();
        assert!(ids.contains(&TelemetryId::Office));
        assert!(ids.contains(&TelemetryId::Firefox));
        assert!(ids.contains(&TelemetryId::Chrome));
        assert!(ids.contains(&TelemetryId::Nvidia));
        assert!(ids.contains(&TelemetryId::VisualStudio));
        assert!(ids.contains(&TelemetryId::Windows));
    }

    #[test]
    fn test_telemetry_items_no_empty_fields() {
        for item in telemetry_items() {
            assert!(!item.title.is_empty(), "empty title");
            assert!(!item.description.is_empty(), "empty description for {}", item.title);
        }
    }

    #[test]
    fn test_telemetry_script_all_ids_covered() {
        for id in [TelemetryId::Office, TelemetryId::Firefox, TelemetryId::Chrome, TelemetryId::Nvidia, TelemetryId::VisualStudio, TelemetryId::Windows] {
            let disable = telemetry_script(id, true);
            let enable = telemetry_script(id, false);
            assert!(!disable.is_empty(), "empty disable script for {:?}", id);
            assert!(!enable.is_empty(), "empty enable script for {:?}", id);
        }
    }
}
