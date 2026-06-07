use crate::logger::{LogLevel, Logger};
use crate::powershell::run_powershell;
use crate::types::WindowsUpdateAction;

const SCRIPT_QUERY: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$wuPath = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate'
$auPath = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'

# 1. Проверка служб — если wuauserv или UsoSvc отключены, обновления не работают
$wuService = Get-Service -Name wuauserv -ErrorAction SilentlyContinue
$usoService = Get-Service -Name UsoSvc -ErrorAction SilentlyContinue
if ($wuService -and $wuService.StartType -eq 'Disabled') {
    Write-Output 'disable'
    exit
}
if ($usoService -and $usoService.StartType -eq 'Disabled') {
    Write-Output 'disable'
    exit
}

# 2. Проверка реестра на NoAutoUpdate
$noAuto = (Get-ItemProperty -Path $auPath -Name 'NoAutoUpdate' -ErrorAction SilentlyContinue).NoAutoUpdate
if ($noAuto -eq 1) {
    Write-Output 'disable'
    exit
}

# 3. Проверка режима безопасности (отложенные обновления)
$featDays = (Get-ItemProperty -Path $wuPath -Name 'FeatureUpdatesMaxDaysBeforeDeferral' -ErrorAction SilentlyContinue).FeatureUpdatesMaxDaysBeforeDeferral
$secDays = (Get-ItemProperty -Path $wuPath -Name 'SecurityUpdatesMaxDaysBeforeDeferral' -ErrorAction SilentlyContinue).SecurityUpdatesMaxDaysBeforeDeferral
$noDrivers = (Get-ItemProperty -Path $wuPath -Name 'ExcludeWUDriversInQualityUpdate' -ErrorAction SilentlyContinue).ExcludeWUDriversInQualityUpdate

if ($featDays -eq 365 -and $secDays -eq 4 -and $noDrivers -eq 1) {
    Write-Output 'security'
    exit
}

# 4. Если ни политик, ни служба не тронуты — по умолчанию
Write-Output 'default'
"#;

const SCRIPT_DEFAULT: &str = r#"
$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$paths = @(
    'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate',
    'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'
)
foreach ($path in $paths) {
    if (Test-Path $path) {
        Remove-Item -Path $path -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Set-Service -Name wuauserv -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name wuauserv -ErrorAction SilentlyContinue
Set-Service -Name UsoSvc -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name UsoSvc -ErrorAction SilentlyContinue
Set-Service -Name WaaSMedicSvc -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'Настройки обновлений Windows сброшены на стандартные.'
"#;

const SCRIPT_SECURITY: &str = r#"
$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$paths = @(
    'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate',
    'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'
)
foreach ($path in $paths) {
    if (-not (Test-Path $path)) {
        $null = New-Item -Path $path -Force -ErrorAction SilentlyContinue
    }
}
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -Name 'FeatureUpdatesMaxDaysBeforeDeferral' -Value 365 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -Name 'SecurityUpdatesMaxDaysBeforeDeferral' -Value 4 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -Name 'ExcludeWUDriversInQualityUpdate' -Value 1 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'NoAutoUpdate' -Value 0 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'AUOptions' -Value 4 -Type DWord -ErrorAction SilentlyContinue
Set-Service -Name wuauserv -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name wuauserv -ErrorAction SilentlyContinue
Set-Service -Name UsoSvc -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name UsoSvc -ErrorAction SilentlyContinue
Set-Service -Name WaaSMedicSvc -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'Настройки безопасности обновлений применены.'
"#;

const SCRIPT_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$paths = @(
    'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate',
    'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'
)
foreach ($path in $paths) {
    if (-not (Test-Path $path)) {
        $null = New-Item -Path $path -Force -ErrorAction SilentlyContinue
    }
}
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'NoAutoUpdate' -Value 1 -Type DWord -ErrorAction SilentlyContinue
Set-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -Name 'DisableWindowsUpdateAccess' -Value 1 -Type DWord -ErrorAction SilentlyContinue
Set-Service -Name wuauserv -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name wuauserv -Force -ErrorAction SilentlyContinue
Set-Service -Name UsoSvc -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name UsoSvc -Force -ErrorAction SilentlyContinue
Set-Service -Name WaaSMedicSvc -StartupType Disabled -ErrorAction SilentlyContinue
Stop-Service -Name WaaSMedicSvc -Force -ErrorAction SilentlyContinue
Write-Output 'Все обновления Windows отключены.'
"#;

fn script_for(action: WindowsUpdateAction) -> &'static str {
    match action {
        WindowsUpdateAction::Default => SCRIPT_DEFAULT,
        WindowsUpdateAction::Security => SCRIPT_SECURITY,
        WindowsUpdateAction::Disable => SCRIPT_DISABLE,
    }
}

pub fn run_windows_update_op(action: WindowsUpdateAction, logger: &Logger) -> (bool, String) {
    let script = script_for(action);
    run_powershell(script, logger)
}

pub fn query_windows_update_status(logger: &Logger) -> WindowsUpdateAction {
    let (ok, out) = run_powershell(SCRIPT_QUERY, logger);
    if !ok {
        logger.log(LogLevel::Normal, &format!("Windows Update status query failed: {out}"));
    }
    match out.trim() {
        "disable" => WindowsUpdateAction::Disable,
        "security" => WindowsUpdateAction::Security,
        _ => WindowsUpdateAction::Default,
    }
}

pub fn windows_update_actions() -> Vec<WindowsUpdateAction> {
    vec![
        WindowsUpdateAction::Default,
        WindowsUpdateAction::Security,
        WindowsUpdateAction::Disable,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_update_actions_scripts() {
        for action in windows_update_actions() {
            let script = script_for(action);
            assert!(!script.is_empty(), "script should not be empty for {:?}", action);
            assert!(script.contains("ErrorActionPreference"));
        }
    }
}
