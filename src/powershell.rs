use std::os::windows::process::CommandExt;
use std::process::Command;

use crate::logger::{Logger, LogLevel};

pub fn run_powershell(script: &str, logger: &Logger) -> (bool, String) {

    let wrapped = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;\
         $OutputEncoding = [System.Text.Encoding]::UTF8;\
         {script}"
    );
    logger.log(LogLevel::Debug, &format!("PS> {script}"));

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-NonInteractive",
            "-Command",
            &wrapped,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(o) => {
            let mut s = String::new();
            s.push_str(&String::from_utf8_lossy(&o.stdout));
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(&err);
            }
            let result = s.trim().to_string();
            logger.log(
                LogLevel::Debug,
                &format!("PS< status={:?}, output={}", o.status.code(), result),
            );
            (o.status.success(), result)
        }
        Err(e) => {
            let msg = format!("Не удалось запустить PowerShell: {e}");
            logger.log(LogLevel::Normal, &msg);
            (false, msg)
        }
    }
}
