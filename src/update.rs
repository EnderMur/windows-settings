use std::fs;
use std::sync::Arc;

use crate::logger::{Logger, LogLevel};
use crate::time_win::local_timestamp_filename;
use crate::types::UpdateState;

pub const REPO_OWNER: &str = "EnderMur";
pub const REPO_NAME: &str = "windows-settings";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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

pub fn check_latest_release(logger: &Logger, token: Option<&str>) -> Result<String, String> {
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

pub fn is_rate_limit_error(state: &UpdateState) -> bool {
    if let UpdateState::Error(e) = state {
        e.contains("403")
    } else {
        false
    }
}

pub fn friendly_github_error(raw: &str) -> String {
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

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (semver::Version::parse(latest), semver::Version::parse(current)) {
        (Ok(l), Ok(c)) => l > c,

        _ => latest != current,
    }
}

pub fn do_self_update(logger: &Logger, token: Option<&str>) -> Result<String, String> {
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

pub fn download_asset_bytes(url: &str, token: Option<&str>) -> Result<Vec<u8>, String> {

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0");
    }

    #[test]
    fn test_format_bytes_bytes() {
        assert_eq!(format_bytes(500), "500 Б");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        assert_eq!(format_bytes(1024), "1 КБ");
    }

    #[test]
    fn test_format_bytes_megabytes() {
        let result = format_bytes(1024 * 1024);
        assert!(result.contains("МБ"), "expected МБ in {result}");
    }

    #[test]
    fn test_format_bytes_gigabytes() {
        let result = format_bytes(1024 * 1024 * 1024 * 4);
        assert!(result.contains("ГБ"), "expected ГБ in {result}");
    }

    #[test]
    fn test_format_bytes_terabytes() {
        let result = format_bytes(1024u64.pow(4));
        assert!(result.contains("ТБ"), "expected ТБ in {result}");
    }

    #[test]
    fn test_is_newer_major() {
        assert!(is_newer("2.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "2.0.0"));
    }

    #[test]
    fn test_is_newer_minor() {
        assert!(is_newer("1.1.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.1.0"));
    }

    #[test]
    fn test_is_newer_patch() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn test_is_newer_equal() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_invalid() {
        assert!(is_newer("abc", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_rate_limit_error() {
        assert!(is_rate_limit_error(&UpdateState::Error("HTTP 403".into())));
        assert!(!is_rate_limit_error(&UpdateState::Error("HTTP 404".into())));
        assert!(!is_rate_limit_error(&UpdateState::Idle));
    }

    #[test]
    fn test_friendly_github_error_403() {
        let msg = friendly_github_error("403 Forbidden");
        assert!(msg.contains("403"));
        assert!(msg.contains("лимит"));
    }

    #[test]
    fn test_friendly_github_error_404() {
        let msg = friendly_github_error("404 Not Found");
        assert!(msg.contains("404"));
    }

    #[test]
    fn test_friendly_github_error_network() {
        let msg = friendly_github_error("dns resolve failed");
        assert!(msg.contains("интернет"));
    }

    #[test]
    fn test_friendly_github_error_timeout() {
        let msg = friendly_github_error("connection timed out");
        assert!(msg.contains("интернет"));
    }
}
