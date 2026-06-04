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

fn parse_sys_info_output(out: &str) -> SysInfo {
    let mut info = SysInfo::default();
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

pub fn collect_sys_info(logger: &Logger) -> SysInfo {
    let (ok, out) = run_powershell(SYSINFO_SCRIPT, logger);
    if !ok {
        logger.log(
            LogLevel::Normal,
            &format!("System info query failed: {out}"),
        );
        return SysInfo::default();
    }
    parse_sys_info_output(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sys_info_output_full() {
        let info = parse_sys_info_output(
            "os=Windows 11 Pro\nbuild=26100\narch=64-bit\nhostname=DESKTOP\nuser=Admin\nadmin=True\ncpu=  Intel   Core   i7  \ngpu=NVIDIA RTX 4060\nram_gb=31.9\n",
        );
        assert_eq!(info.os, "Windows 11 Pro");
        assert_eq!(info.build, "26100");
        assert_eq!(info.arch, "64-bit");
        assert_eq!(info.hostname, "DESKTOP");
        assert_eq!(info.user, "Admin");
        assert_eq!(info.is_admin, Some(true));
        assert_eq!(info.cpu, "Intel Core i7");
        assert_eq!(info.gpu, "NVIDIA RTX 4060");
        assert_eq!(info.ram_gb, "31.9");
    }

    #[test]
    fn test_parse_sys_info_output_ignores_unknown_and_invalid_admin() {
        let info = parse_sys_info_output(
            "unknown=value\nadmin=maybe\nuser=Tester\nthis is noise\n",
        );
        assert_eq!(info.user, "Tester");
        assert_eq!(info.is_admin, None);
        assert!(info.os.is_empty());
    }

    #[test]
    fn test_parse_sys_info_output_false_admin() {
        let info = parse_sys_info_output("admin=False\n");
        assert_eq!(info.is_admin, Some(false));
    }
}
