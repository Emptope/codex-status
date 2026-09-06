use super::Sound;
#[cfg(target_os = "macos")]
use objc2::{AnyThread, rc::Retained};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSSound;
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
#[cfg(target_os = "macos")]
use std::{cell::RefCell, path::Path};
#[cfg(target_os = "macos")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};

const EVENT: &str = "play-sound";

#[cfg(target_os = "macos")]
struct Players {
    completion: Option<Retained<NSSound>>,
    quota_alert: Option<Retained<NSSound>>,
    quota_battery: Option<Retained<NSSound>>,
}

#[cfg(target_os = "macos")]
impl Players {
    fn load(root: &Path) -> Self {
        let load = |name| {
            let path = root.join("sounds").join(name);
            path.is_file()
                .then(|| NSString::from_str(&path.to_string_lossy()))
                .and_then(|path| {
                    NSSound::initWithContentsOfFile_byReference(NSSound::alloc(), &path, false)
                })
        };
        Self {
            completion: load("ding.mp3"),
            quota_alert: load("quota-alert.mp3"),
            quota_battery: load("quota-low-battery.mp3"),
        }
    }

    fn play(&self, sound: Sound) -> bool {
        let player = match sound {
            Sound::Completion => &self.completion,
            Sound::QuotaAlert => &self.quota_alert,
            Sound::QuotaBattery => &self.quota_battery,
        };
        player.as_ref().is_some_and(|player| {
            player.setCurrentTime(0.0);
            player.isPlaying() || player.play()
        })
    }
}

#[cfg(target_os = "macos")]
thread_local! {
    static PLAYERS: RefCell<Option<Players>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
pub fn prepare(app: &AppHandle) {
    if let Ok(root) = app.path().resource_dir() {
        PLAYERS.with(|players| *players.borrow_mut() = Some(Players::load(&root)));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn prepare(_app: &AppHandle) {}

pub fn play(app: &AppHandle, sound: Sound) {
    #[cfg(target_os = "macos")]
    {
        let fallback = app.clone();
        if app
            .run_on_main_thread(move || {
                let played = PLAYERS.with(|players| {
                    players
                        .borrow()
                        .as_ref()
                        .is_some_and(|players| players.play(sound))
                });
                if !played {
                    let _ = fallback.emit(EVENT, sound);
                }
            })
            .is_err()
        {
            let _ = app.emit(EVENT, sound);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app.emit(EVENT, sound);
}
