use crate::logger::{Logger, LogLevel};
use crate::powershell::run_powershell;
use crate::types::SysInfo;

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

pub fn collect_sys_info(logger: &Logger) -> SysInfo {
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
