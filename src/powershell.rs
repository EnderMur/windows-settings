use std::os::windows::process::CommandExt;
use std::process::Command;

use crate::logger::{LogLevel, Logger};

pub fn run_powershell(script: &str, logger: &Logger) -> (bool, String) {
    logger.log(LogLevel::Debug, &format!("PS> {script}"));

    let encoded = encode_powershell_script(script);

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let child = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match child {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();

            let mut s = stdout;
            if !stderr.is_empty() {
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(&stderr);
            }

            let result = s.trim().to_string();
            logger.log(LogLevel::Debug, &format!("PS< output={result}"));
            (o.status.success() || !result.is_empty(), result)
        }
        Err(e) => {
            let msg = format!("Не удалось запустить PowerShell: {e}");
            logger.log(LogLevel::Normal, &msg);
            (false, msg)
        }
    }
}

fn encode_powershell_script(script: &str) -> String {
    let bytes = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect::<Vec<u8>>();
    base64_encode(&bytes)
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
