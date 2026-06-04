use crate::logger::{LogLevel, Logger};
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

fn parse_telemetry_status_output(out: &str) -> Vec<(TelemetryId, TelemetryStatus)> {
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
    parse_telemetry_status_output(&out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
            assert!(
                !item.description.is_empty(),
                "empty description for {}",
                item.title
            );
        }
    }

    #[test]
    fn test_telemetry_script_all_ids_covered() {
        for id in [
            TelemetryId::Office,
            TelemetryId::Firefox,
            TelemetryId::Chrome,
            TelemetryId::Nvidia,
            TelemetryId::VisualStudio,
            TelemetryId::Windows,
        ] {
            let disable = telemetry_script(id, true);
            let enable = telemetry_script(id, false);
            assert!(!disable.is_empty(), "empty disable script for {:?}", id);
            assert!(!enable.is_empty(), "empty enable script for {:?}", id);
        }
    }

    #[test]
    fn test_parse_telemetry_status_output() {
        let statuses =
            parse_telemetry_status_output("office=disabled\nfirefox=enabled\nwindows=unknown\ninvalid=line\n");
        assert_eq!(statuses.len(), 3);
        assert!(statuses.contains(&(TelemetryId::Office, TelemetryStatus::Disabled)));
        assert!(statuses.contains(&(TelemetryId::Firefox, TelemetryStatus::Enabled)));
        assert!(statuses.contains(&(TelemetryId::Windows, TelemetryStatus::Unknown)));
    }

    #[test]
    fn test_telemetry_items_unique_and_roundtrip() {
        let items = telemetry_items();
        let mut seen = HashSet::new();
        for item in items {
            let key = item.id.key();
            assert!(seen.insert(key), "duplicate telemetry key: {key}");
            assert_eq!(TelemetryId::from_key(key), Some(item.id));
            assert!(!item.title.trim().is_empty());
            assert!(!item.description.trim().is_empty());
        }
    }

    #[test]
    fn test_telemetry_scripts_match_expected_operations() {
        let cases = [
            (TelemetryId::Office, "OfficeTelemetryAgent", "Enable-ScheduledTask"),
            (TelemetryId::Firefox, "DisableTelemetry", "Remove-ItemProperty"),
            (TelemetryId::Chrome, "GoogleUpdateTask", "Enable-ScheduledTask"),
            (TelemetryId::Nvidia, "NvTelemetryContainer", "Set-Service"),
            (TelemetryId::VisualStudio, "VSCommon", "Remove-ItemProperty"),
            (TelemetryId::Windows, "DiagTrack", "Set-Service"),
        ];
        for (id, disable_marker, enable_marker) in cases {
            let disable_script = telemetry_script(id, true);
            let enable_script = telemetry_script(id, false);
            assert!(!disable_script.is_empty());
            assert!(!enable_script.is_empty());
            assert_ne!(disable_script, enable_script);
            assert!(
                disable_script.contains(disable_marker),
                "missing disable marker for {:?}",
                id
            );
            assert!(
                enable_script.contains(enable_marker),
                "missing enable marker for {:?}",
                id
            );
        }
    }
}
