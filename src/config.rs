use std::fs;

use crate::logger::{LogLevel, Logger};
use crate::time_win::{appdata_config_path, appdata_settings_path};

#[derive(Default)]
pub struct Config {
    pub github_token: Option<String>,
}

pub fn load_config(logger: &Logger) -> Config {
    let path = appdata_config_path();
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            logger.log(LogLevel::Debug, "Config file not found, using defaults");
            return Config::default();
        }
        Err(e) => {
            logger.log(LogLevel::Normal, &format!("Failed to read config: {e}"));
            return Config::default();
        }
    };
    let cfg = parse_config(&content);
    logger.log(
        LogLevel::Debug,
        &format!(
            "Config loaded from {}: github_token_present={}",
            path.display(),
            cfg.github_token.is_some()
        ),
    );
    cfg
}

pub fn save_config(cfg: &Config, logger: &Logger) -> Result<(), String> {
    let path = appdata_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("создать каталог: {e}"))?;
    }
    let content = match &cfg.github_token {
        Some(t) => format!("{{\n  \"github_token\": \"{}\"\n}}\n", json_escape(t)),
        None => "{}\n".to_string(),
    };
    fs::write(&path, content).map_err(|e| format!("записать файл: {e}"))?;
    logger.log(
        LogLevel::Normal,
        &format!(
            "Config saved to {}: github_token_present={}",
            path.display(),
            cfg.github_token.is_some()
        ),
    );
    Ok(())
}

fn parse_config(content: &str) -> Config {
    let mut cfg = Config::default();
    if let Some(token) = extract_json_string(content, "github_token") {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            cfg.github_token = Some(trimmed.to_string());
        }
    }
    cfg
}

fn extract_json_string(content: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let pos = content.find(&pattern)?;
    let after = &content[pos + pattern.len()..];
    let colon = after.find(':')?;
    let after_colon = &after[colon + 1..];
    let quote_pos = after_colon.find('"')?;
    let value_start = &after_colon[quote_pos + 1..];

    let mut out = String::new();
    let mut chars = value_start.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000C}'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 {
                        return None;
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    if let Some(c) = char::from_u32(code) {
                        out.push(c);
                    }
                }
                other => out.push(other),
            },
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[derive(Clone)]
pub struct AppSettings {
    pub log_level: LogLevel,
    pub show_hidden_features: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            log_level: LogLevel::Normal,
            show_hidden_features: false,
        }
    }
}

pub fn log_level_to_str(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Normal => "normal",
        LogLevel::Debug => "debug",
    }
}

pub fn log_level_from_str(s: &str) -> Option<LogLevel> {
    match s.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(LogLevel::Normal),
        "debug" => Some(LogLevel::Debug),
        _ => None,
    }
}

pub fn load_settings(logger: &Logger) -> AppSettings {
    let path = appdata_settings_path();
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            logger.log(LogLevel::Debug, "Settings file not found, using defaults");
            return AppSettings::default();
        }
        Err(e) => {
            logger.log(LogLevel::Normal, &format!("Failed to read settings: {e}"));
            return AppSettings::default();
        }
    };
    let settings = parse_settings(&content);
    logger.log(
        LogLevel::Debug,
        &format!(
            "Settings loaded from {}: log_level={}",
            path.display(),
            log_level_to_str(settings.log_level)
        ),
    );
    settings
}

pub fn save_settings(settings: &AppSettings, logger: &Logger) -> Result<(), String> {
    let path = appdata_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("создать каталог: {e}"))?;
    }
    let content = format!(
        "# Windows Settings — пользовательские настройки\n\
         # Формат: key=value, одна пара на строку.\n\
         log_level={}\n\
         show_hidden_features={}\n",
        log_level_to_str(settings.log_level),
        settings.show_hidden_features
    );
    fs::write(&path, content).map_err(|e| format!("записать файл: {e}"))?;
    logger.log(
        LogLevel::Normal,
        &format!(
            "Settings saved to {}: log_level={}",
            path.display(),
            log_level_to_str(settings.log_level)
        ),
    );
    Ok(())
}

fn parse_settings(content: &str) -> AppSettings {
    let mut settings = AppSettings::default();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "log_level"
            && let Some(level) = log_level_from_str(value)
        {
            settings.log_level = level;
        }
        if key == "show_hidden_features" {
            settings.show_hidden_features = value == "true";
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_escape_basic() {
        assert_eq!(json_escape("hello"), "hello");
        assert_eq!(json_escape(r#""quoted""#), r#"\"quoted\""#);
        assert_eq!(json_escape("back\\slash"), "back\\\\slash");
        assert_eq!(json_escape("line\nbreak"), "line\\nbreak");
    }

    #[test]
    fn test_json_escape_unicode() {
        assert_eq!(json_escape("\t"), "\\t");
        assert_eq!(json_escape("\r"), "\\r");
    }

    #[test]
    fn test_extract_json_string_simple() {
        let json = r#"{"github_token": "ghp_abc123"}"#;
        assert_eq!(
            extract_json_string(json, "github_token"),
            Some("ghp_abc123".to_string())
        );
    }

    #[test]
    fn test_extract_json_string_empty() {
        let json = r#"{"github_token": ""}"#;
        assert_eq!(
            extract_json_string(json, "github_token"),
            Some("".to_string())
        );
    }

    #[test]
    fn test_extract_json_string_missing() {
        let json = r#"{"other": "value"}"#;
        assert_eq!(extract_json_string(json, "github_token"), None);
    }

    #[test]
    fn test_parse_config_with_token() {
        let json = r#"{"github_token": "ghp_test"}"#;
        let cfg = parse_config(json);
        assert_eq!(cfg.github_token, Some("ghp_test".to_string()));
    }

    #[test]
    fn test_parse_config_without_token() {
        let cfg = parse_config("{}");
        assert!(cfg.github_token.is_none());
    }

    #[test]
    fn test_parse_config_trims_token() {
        let json = r#"{"github_token": "  ghp_test  "}"#;
        let cfg = parse_config(json);
        assert_eq!(cfg.github_token, Some("ghp_test".to_string()));
    }

    #[test]
    fn test_parse_settings_normal() {
        let content = "log_level=normal\n";
        let s = parse_settings(content);
        assert_eq!(s.log_level, LogLevel::Normal);
    }

    #[test]
    fn test_parse_settings_debug() {
        let content = "# comment\nlog_level=debug\n";
        let s = parse_settings(content);
        assert_eq!(s.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_parse_settings_empty() {
        let s = parse_settings("");
        assert_eq!(s.log_level, LogLevel::Normal);
    }

    #[test]
    fn test_parse_settings_ignore_unknown() {
        let content = "unknown_key=value\nlog_level=debug\n";
        let s = parse_settings(content);
        assert_eq!(s.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_log_level_roundtrip() {
        assert_eq!(
            log_level_from_str(log_level_to_str(LogLevel::Normal)),
            Some(LogLevel::Normal)
        );
        assert_eq!(
            log_level_from_str(log_level_to_str(LogLevel::Debug)),
            Some(LogLevel::Debug)
        );
    }

    #[test]
    fn test_log_level_from_str_case_insensitive() {
        assert_eq!(log_level_from_str("NORMAL"), Some(LogLevel::Normal));
        assert_eq!(log_level_from_str("Debug"), Some(LogLevel::Debug));
        assert_eq!(log_level_from_str("unknown"), None);
    }
}
