use crate::types::Status;
use crate::logger::Logger;
use crate::powershell::run_powershell;
use crate::types::Card;

pub fn uwp_apps() -> Vec<Card> {
    let apps: &[(&str, &str, &str)] = &[
        ("Microsoft Store", "Microsoft.WindowsStore", "Официальный магазин приложений Windows."),
        ("Калькулятор", "Microsoft.WindowsCalculator", "Стандартный калькулятор Windows."),
        ("Камера", "Microsoft.WindowsCamera", "Фото и видео с веб-камеры."),
        ("Часы", "Microsoft.WindowsAlarms", "Будильники, таймеры, секундомер и мировое время."),
        ("Календарь и Почта", "microsoft.windowscommunicationsapps", "Почтовый клиент и календарь."),
        ("Карты", "Microsoft.WindowsMaps", "Карты, поиск мест и маршруты."),
        ("Новости", "Microsoft.BingNews", "Лента новостей Microsoft."),
        ("Microsoft To Do", "Microsoft.Todos", "Списки задач и напоминания."),
        ("Кино и ТВ", "Microsoft.ZuneVideo", "Просмотр видео и фильмов."),
        ("Microsoft Solitaire Collection", "Microsoft.MicrosoftSolitaireCollection", "Коллекция пасьянсов."),
        ("OneNote для Windows 10", "Microsoft.Office.OneNote", "Цифровой блокнот OneNote."),
        ("Paint", "Microsoft.Paint", "Графический редактор Paint."),
        ("Люди", "Microsoft.People", "Адресная книга и контакты."),
        ("Связь с телефоном", "Microsoft.YourPhone", "Phone Link: связь с Android/iPhone."),
        ("Фотографии", "Microsoft.Windows.Photos", "Просмотр и редактирование фото."),
        ("Быстрая помощь", "MicrosoftCorporationII.QuickAssist", "Quick Assist: удалённая помощь."),
        ("Ножницы", "Microsoft.ScreenSketch", "Snipping Tool: снимки и запись экрана."),
        ("Запись голоса", "Microsoft.WindowsSoundRecorder", "Диктофон."),
        ("Записки", "Microsoft.MicrosoftStickyNotes", "Sticky Notes: заметки."),
        ("Советы", "Microsoft.Getstarted", "Подсказки и руководства по Windows."),
        ("Погода", "Microsoft.BingWeather", "Прогноз погоды."),
        ("Безопасность Windows", "Microsoft.SecHealthUI", "Windows Defender: антивирус."),
        ("Терминал Windows", "Microsoft.WindowsTerminal", "Современный терминал Windows."),
        ("Xbox", "Microsoft.GamingApp", "Игровой клиент Xbox и Game Pass."),
        ("Xbox Game Bar", "Microsoft.XboxGamingOverlay", "Игровая панель Xbox."),
        ("Clipchamp", "Clipchamp.Clipchamp", "Видеоредактор Clipchamp."),
        ("Microsoft Teams", "MSTeams", "Чат, звонки, видеоконференции."),
        ("Блокнот", "Microsoft.WindowsNotepad", "Текстовый редактор Notepad."),
        ("Проигрыватель Windows Media", "Microsoft.ZuneMusic", "Media Player для музыки и видео."),
        ("Microsoft Family", "MicrosoftCorporationII.MicrosoftFamily", "Родительский контроль."),
        ("Power Automate", "Microsoft.PowerAutomateDesktop", "Автоматизация задач."),
        ("Получение справки", "Microsoft.GetHelp", "Get Help: справка Microsoft."),
        ("Центр отзывов", "Microsoft.WindowsFeedbackHub", "Feedback Hub."),
        ("Cortana", "Microsoft.549981C3F5F10", "Голосовой помощник Cortana."),
        ("App Installer", "Microsoft.DesktopAppInstaller", "winget и установщик пакетов."),
        ("Photon (File Explorer)", "MicrosoftWindows.Client.Photon", "Современный проводник Windows 11."),
        ("Параметры", "windows.immersivecontrolpanel", "Системные настройки Windows."),
    ];

    apps.iter()
        .map(|(title, package, desc)| Card {
            title: (*title).to_string(),
            description: (*desc).to_string(),
            package: (*package).to_string(),
            status: Status::Unknown,
            busy: false,
            log: None,
        })
        .collect()
}

pub fn query_installed_packages(packages: &[String], logger: &Logger) -> Vec<(String, bool)> {
    let script = "Get-AppxPackage | ForEach-Object { $_.Name }";
    let (ok, out) = run_powershell(script, logger);
    let mut installed: Vec<String> = Vec::new();
    if ok {
        for line in out.lines() {
            let l = line.trim();
            if !l.is_empty() {
                installed.push(l.to_string());
            }
        }
    }
    packages
        .iter()
        .map(|p| {
            let present = installed.iter().any(|n| n.eq_ignore_ascii_case(p));
            (p.clone(), present)
        })
        .collect()
}

pub fn run_remove_package(pkg: &str, logger: &Logger) -> (bool, String) {
    let escaped = pkg.replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop';\
         $p = Get-AppxPackage -Name '{escaped}' -ErrorAction SilentlyContinue;\
         if ($null -eq $p) {{ Write-Output 'Пакет не найден (уже удалён).'; exit 0 }}\
         try {{ $p | Remove-AppxPackage -ErrorAction Stop; Write-Output 'Удалено успешно.' }}\
         catch {{ Write-Output ('Ошибка: ' + $_.Exception.Message); exit 1 }}"
    );
    run_powershell(&script, logger)
}

pub fn run_restore_package(pkg: &str, logger: &Logger) -> (bool, String) {
    let escaped = pkg.replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop';\
         $name = '{escaped}';\
         $pkg = Get-AppxPackage -AllUsers -Name $name -ErrorAction SilentlyContinue | Select-Object -First 1;\
         if ($pkg -ne $null -and $pkg.InstallLocation) {{\
             try {{\
                 Add-AppxPackage -DisableDevelopmentMode -Register (Join-Path $pkg.InstallLocation 'AppXManifest.xml') -ErrorAction Stop;\
                 Write-Output ('Восстановлено из ' + $pkg.InstallLocation);\
                 exit 0\
             }} catch {{ Write-Output ('Re-register failed: ' + $_.Exception.Message) }}\
         }}\
         $prov = Get-AppxProvisionedPackage -Online -ErrorAction SilentlyContinue | Where-Object {{ $_.DisplayName -ieq $name }} | Select-Object -First 1;\
         if ($prov -ne $null) {{\
             try {{\
                 Add-AppxPackage -Path $prov.PackagePath -ErrorAction Stop;\
                 Write-Output 'Восстановлено из provisioned-пакета.';\
                 exit 0\
             }} catch {{ Write-Output ('Provisioned install failed: ' + $_.Exception.Message) }}\
         }}\
         if (Get-Command winget -ErrorAction SilentlyContinue) {{\
             $w = winget install --id $name --accept-source-agreements --accept-package-agreements --silent 2>&1;\
             if ($LASTEXITCODE -eq 0) {{ Write-Output 'Установлено через winget.'; exit 0 }}\
             else {{ Write-Output ('winget: ' + ($w | Out-String)) }}\
         }}\
         Start-Process ('ms-windows-store://search/?query=' + [uri]::EscapeDataString($name));\
         Write-Output 'Открыт Microsoft Store для ручной установки.';\
         exit 1"
    );
    run_powershell(&script, logger)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uwp_apps_count() {
        let apps = uwp_apps();
        assert!(apps.len() >= 35, "expected at least 35 UWP apps, got {}", apps.len());
    }

    #[test]
    fn test_uwp_apps_no_empty_fields() {
        for app in uwp_apps() {
            assert!(!app.title.is_empty(), "empty title");
            assert!(!app.package.is_empty(), "empty package for {}", app.title);
            assert!(!app.description.is_empty(), "empty description for {}", app.title);
        }
    }

    #[test]
    fn test_uwp_apps_unique_packages() {
        let apps = uwp_apps();
        let mut seen = std::collections::HashSet::new();
        for app in &apps {
            assert!(seen.insert(app.package.to_ascii_lowercase()), "duplicate package: {}", app.package);
        }
    }
}
