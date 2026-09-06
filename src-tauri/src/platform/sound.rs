use super::Sound;
#[cfg(target_os = "macos")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};

const EVENT: &str = "play-sound";

pub fn play(app: &AppHandle, sound: Sound) {
    #[cfg(target_os = "macos")]
    {
        use std::process::Stdio;

        let Ok(root) = app.path().resource_dir() else {
            let _ = app.emit(EVENT, sound);
            return;
        };
        let name = match sound {
            Sound::Completion => "ding.mp3",
            Sound::QuotaAlert => "quota-alert.mp3",
            Sound::QuotaBattery => "quota-low-battery.mp3",
        };
        let path = root.join("sounds").join(name);
        if !path.is_file() {
            let _ = app.emit(EVENT, sound);
            return;
        }
        tauri::async_runtime::spawn(async move {
            let mut command = tokio::process::Command::new("/usr/bin/afplay");
            command
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            let _ = command.status().await;
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app.emit(EVENT, sound);
}
