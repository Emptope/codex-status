use super::Sound;
#[cfg(target_os = "macos")]
use objc2::{AnyThread, rc::Retained};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSSound;
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
#[cfg(target_os = "macos")]
use std::{cell::RefCell, path::Path, thread, time::Duration};
#[cfg(target_os = "macos")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};

const EVENT: &str = "play-sound";

#[cfg(target_os = "macos")]
struct Players {
    approval_bell: Option<Retained<NSSound>>,
    completion_bell: Option<Retained<NSSound>>,
    completion_ding: Option<Retained<NSSound>>,
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
            approval_bell: load("approval-bell.mp3"),
            completion_bell: load("completion-bell.mp3"),
            completion_ding: load("completion-ding.mp3"),
            quota_alert: load("quota-alert.mp3"),
            quota_battery: load("quota-low-battery.mp3"),
        }
    }

    fn player(&self, sound: Sound) -> Option<&NSSound> {
        match sound {
            Sound::ApprovalBell => &self.approval_bell,
            Sound::CompletionBell => &self.completion_bell,
            Sound::CompletionDing => &self.completion_ding,
            Sound::QuotaAlert => &self.quota_alert,
            Sound::QuotaBattery => &self.quota_battery,
        }
        .as_deref()
    }

    fn warm(&self) {
        for sound in [
            Sound::ApprovalBell,
            Sound::CompletionBell,
            Sound::CompletionDing,
            Sound::QuotaAlert,
            Sound::QuotaBattery,
        ] {
            if let Some(player) = self.player(sound) {
                let volume = player.volume();
                player.setVolume(0.0);
                player.setCurrentTime(0.0);
                if player.play() {
                    // NSSound decodes compressed data and starts CoreAudio lazily. Give the
                    // silent warm-up enough time to move that work out of the first alert.
                    thread::sleep(Duration::from_millis(20));
                    player.stop();
                }
                player.setCurrentTime(0.0);
                player.setVolume(volume);
            }
        }
    }

    fn play(&self, sound: Sound) -> bool {
        self.player(sound).is_some_and(|player| {
            if player.isPlaying() {
                player.stop();
            }
            player.setCurrentTime(0.0);
            player.play()
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
        PLAYERS.with(|players| {
            let loaded = Players::load(&root);
            loaded.warm();
            *players.borrow_mut() = Some(loaded);
        });
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
