use crate::logger::{LogLevel, Logger};
use crate::powershell::run_powershell;
use crate::types::{ServiceId, ServiceItem, ServiceStatus};

pub fn service_items() -> Vec<ServiceItem> {
    let defs: &[(ServiceId, &str, &str)] = &[
        (
            ServiceId::DiagTrack,
            "Диагностическое отслеживание (DiagTrack)",
            "Сбор телеметрии Windows. Отключение безопасно для большинства пользователей.",
        ),
        (
            ServiceId::Dmwappushservice,
            "WAP-PUSH сообщения (dmwappushservice)",
            "Сервис для push-уведомлений. Может быть отключён для повышения приватности.",
        ),
        (
            ServiceId::WSearch,
            "Поиск Windows (WSearch)",
            "Индексация файлов для поиска. Отключение замедлит поиск, но снизит нагрузку.",
        ),
        (
            ServiceId::Dosvc,
            "Оптимизация доставки (DoSvc)",
            "Delivery Optimization — обновления от других ПК в локальной сети.",
        ),
        (
            ServiceId::RetailDemo,
            "Розничная демонстрация (RetailDemo)",
            "Служба демонстрационного режима. Отключение безопасно.",
        ),
        (
            ServiceId::XblGameSave,
            "Xbox Game Save (XblGameSave)",
            "Сохранение игровых данных Xbox. Можно отключить, если не используете Xbox.",
        ),
        (
            ServiceId::DcpSvc,
            "Профилактика компонентов (DcpSvc)",
            "Планировщик обслуживания Windows. Отключение снизит фоновую нагрузку.",
        ),
        (
            ServiceId::PcaSvc,
            "Проект совместимости программ (PcaSvc)",
            "Program Compatibility Assistant — отслеживает проблемы совместимости. Отключение безопасно.",
        ),
        (
            ServiceId::Bits,
            "Фоновый интеллектуальный сервис передачи (BITS)",
            "Служба для фоновой загрузки и обновлений. Отключение может прервать Windows Update и другие фоновые загрузки.",
        ),
    ];

    defs.iter()
        .map(|(id, title, desc)| ServiceItem {
            id: *id,
            title: (*title).to_string(),
            description: (*desc).to_string(),
            status: ServiceStatus::Unknown,
            busy: false,
            log: None,
        })
        .collect()
}

pub fn service_script(id: ServiceId, disable: bool) -> &'static str {
    match (id, disable) {
        (ServiceId::DiagTrack, true) => DIAGTRACK_DISABLE,
        (ServiceId::DiagTrack, false) => DIAGTRACK_ENABLE,
        (ServiceId::Dmwappushservice, true) => DMWAPPUSH_DISABLE,
        (ServiceId::Dmwappushservice, false) => DMWAPPUSH_ENABLE,
        (ServiceId::WSearch, true) => WSEARCH_DISABLE,
        (ServiceId::WSearch, false) => WSEARCH_ENABLE,
        (ServiceId::Dosvc, true) => DOSVC_DISABLE,
        (ServiceId::Dosvc, false) => DOSVC_ENABLE,
        (ServiceId::RetailDemo, true) => RETAILDEMO_DISABLE,
        (ServiceId::RetailDemo, false) => RETAILDEMO_ENABLE,
        (ServiceId::XblGameSave, true) => XBLGAMESAVE_DISABLE,
        (ServiceId::XblGameSave, false) => XBLGAMESAVE_ENABLE,
        (ServiceId::DcpSvc, true) => DCPSVC_DISABLE,
        (ServiceId::DcpSvc, false) => DCPSVC_ENABLE,
        (ServiceId::Bits, true) => BITS_DISABLE,
        (ServiceId::Bits, false) => BITS_ENABLE,
        (ServiceId::PcaSvc, true) => PCASVC_DISABLE,
        (ServiceId::PcaSvc, false) => PCASVC_ENABLE,
    }
}

const DIAGTRACK_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'DiagTrack' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DiagTrack' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'DiagTrack отключена.'
"#;

const DIAGTRACK_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'DiagTrack' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'DiagTrack' -ErrorAction SilentlyContinue
Write-Output 'DiagTrack включена.'
"#;

const DMWAPPUSH_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'dmwappushservice' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'dmwappushservice' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'dmwappushservice отключена.'
"#;

const DMWAPPUSH_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'dmwappushservice' -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'dmwappushservice включена.'
"#;

const WSEARCH_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'WSearch' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'WSearch' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'WSearch отключена.'
"#;

const WSEARCH_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'WSearch' -StartupType Automatic -ErrorAction SilentlyContinue
Start-Service -Name 'WSearch' -ErrorAction SilentlyContinue
Write-Output 'WSearch включена.'
"#;

const DOSVC_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'DoSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DoSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'DoSvc отключена.'
"#;

const DOSVC_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'DoSvc' -StartupType Manual -ErrorAction SilentlyContinue
Start-Service -Name 'DoSvc' -ErrorAction SilentlyContinue
Write-Output 'DoSvc включена.'
"#;

const RETAILDEMO_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'RetailDemo' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'RetailDemo' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'RetailDemo отключена.'
"#;

const RETAILDEMO_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'RetailDemo' -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'RetailDemo включена.'
"#;

const XBLGAMESAVE_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'XblGameSave' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'XblGameSave' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'XblGameSave отключена.'
"#;

const XBLGAMESAVE_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'XblGameSave' -StartupType Manual -ErrorAction SilentlyContinue
Start-Service -Name 'XblGameSave' -ErrorAction SilentlyContinue
Write-Output 'XblGameSave включена.'
"#;

const DCPSVC_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'DcpSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'DcpSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'DcpSvc отключена.'
"#;

const DCPSVC_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'DcpSvc' -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'DcpSvc включена.'
"#;

const PCASVC_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'PcaSvc' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'PcaSvc' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'PcaSvc отключена.'
"#;

const PCASVC_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'PcaSvc' -StartupType Manual -ErrorAction SilentlyContinue
Write-Output 'PcaSvc включена.'
"#;

const BITS_DISABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Stop-Service -Name 'BITS' -Force -ErrorAction SilentlyContinue
Set-Service -Name 'BITS' -StartupType Disabled -ErrorAction SilentlyContinue
Write-Output 'BITS отключена.'
"#;

const BITS_ENABLE: &str = r#"
$ErrorActionPreference = 'Continue'
Set-Service -Name 'BITS' -StartupType Manual -ErrorAction SilentlyContinue
Start-Service -Name 'BITS' -ErrorAction SilentlyContinue
Write-Output 'BITS включена.'
"#;

const SERVICE_STATUS_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$results = @()

foreach ($svc in @(
    @{Key='diagtrack'; Name='DiagTrack'},
    @{Key='dmwappushservice'; Name='dmwappushservice'},
    @{Key='wsearch'; Name='WSearch'},
    @{Key='dosvc'; Name='DoSvc'},
    @{Key='retaildemo'; Name='RetailDemo'},
    @{Key='xblgamesave'; Name='XblGameSave'},
    @{Key='dcpsvc'; Name='DcpSvc'},
    @{Key='pcasvc'; Name='PcaSvc'},
    @{Key='bits'; Name='BITS'}
)) {
    $s = Get-Service -Name $svc.Name -ErrorAction SilentlyContinue
    if ($s) {
        $status = if ($s.Status -eq 'Running') { 'running' } else { 'stopped' }
        $startup = switch ($s.StartType) {
            'Disabled' { 'disabled' }
            'Automatic' { 'automatic' }
            'Manual' { 'manual' }
            default { 'unknown' }
        }
        Write-Output "$($svc.Key)=$status`:$startup"
    } else {
        Write-Output "$($svc.Key)=unknown:unknown"
    }
}
"#;

pub fn query_service_status(logger: &Logger) -> Vec<(ServiceId, ServiceStatus)> {
    let (ok, out) = run_powershell(SERVICE_STATUS_SCRIPT, logger);
    if !ok && out.is_empty() {
        logger.log(
            LogLevel::Normal,
            &format!("Service status query failed: {out}"),
        );
    }
    let mut result = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && let Some(id) = ServiceId::from_key(k.trim())
        {
            let status = parse_service_status_value(v.trim());
            result.push((id, status));
        }
    }
    result
}

fn parse_service_status_value(v: &str) -> ServiceStatus {
    let parts: Vec<&str> = v.split(':').collect();
    let status_str = parts.first().copied().unwrap_or("");
    let startup_str = parts.get(1).copied().unwrap_or("");

    match (status_str, startup_str) {
        (_, "disabled") => ServiceStatus::Disabled,
        ("running", _) => ServiceStatus::Running,
        ("stopped", _) => ServiceStatus::Stopped,
        _ => ServiceStatus::Unknown,
    }
}

pub fn run_service_op(id: ServiceId, disable: bool, logger: &Logger) -> (bool, String) {
    let script = service_script(id, disable);
    run_powershell(script, logger)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_items_count() {
        let items = service_items();
        assert_eq!(items.len(), 9);
    }

    #[test]
    fn test_service_id_key_roundtrip() {
        let ids = [
            ServiceId::DiagTrack,
            ServiceId::Dmwappushservice,
            ServiceId::WSearch,
            ServiceId::Dosvc,
            ServiceId::RetailDemo,
            ServiceId::XblGameSave,
            ServiceId::DcpSvc,
            ServiceId::PcaSvc,
            ServiceId::Bits,
        ];
        for id in ids {
            let key = id.key();
            assert!(!key.is_empty());
            assert_eq!(ServiceId::from_key(key), Some(id));
        }
    }

    #[test]
    fn test_parse_service_status_running_automatic() {
        assert_eq!(
            parse_service_status_value("running:automatic"),
            ServiceStatus::Running
        );
    }

    #[test]
    fn test_parse_service_status_stopped_disabled() {
        assert_eq!(
            parse_service_status_value("stopped:disabled"),
            ServiceStatus::Disabled
        );
    }

    #[test]
    fn test_parse_service_status_stopped_manual() {
        assert_eq!(
            parse_service_status_value("stopped:manual"),
            ServiceStatus::Stopped
        );
    }

    #[test]
    fn test_parse_service_status_unknown() {
        assert_eq!(
            parse_service_status_value("unknown:unknown"),
            ServiceStatus::Unknown
        );
    }

    #[test]
    fn test_parse_service_status_empty() {
        assert_eq!(parse_service_status_value(""), ServiceStatus::Unknown);
    }

    #[test]
    fn test_parse_service_status_running_disabled() {
        assert_eq!(
            parse_service_status_value("running:disabled"),
            ServiceStatus::Disabled
        );
    }
}
