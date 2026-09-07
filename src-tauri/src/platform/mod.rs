mod sound;

use crate::{
    settings::{ApprovalSound, CompletionSound, QuotaSound, Settings},
    sources::Runtime,
    status::{
        Snapshot,
        alerts::{AlertKind, Alerts},
    },
};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{
    Emitter, Manager, PhysicalPosition, PhysicalSize, Window,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_notification::NotificationExt;

#[cfg(target_os = "macos")]
fn configure_presence(app: &mut tauri::App) {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

#[cfg(not(target_os = "macos"))]
fn configure_presence(_app: &mut tauri::App) {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Sound {
    ApprovalBell,
    CompletionBell,
    CompletionDing,
    QuotaAlert,
    QuotaBattery,
}

#[derive(Debug, PartialEq, Eq)]
struct Delivery {
    notification: bool,
    sound: Option<Sound>,
}

fn delivery(settings: &Settings, kind: AlertKind) -> Delivery {
    let sound = if settings.muted {
        None
    } else {
        match kind {
            AlertKind::Approval => match settings.approval_sound {
                ApprovalSound::Off => None,
                ApprovalSound::Bell => Some(Sound::ApprovalBell),
            },
            AlertKind::Completion => match settings.completion_sound {
                CompletionSound::Off => None,
                CompletionSound::Bell => Some(Sound::CompletionBell),
                CompletionSound::Ding => Some(Sound::CompletionDing),
            },
            AlertKind::Quota => match settings.quota_sound {
                QuotaSound::Off => None,
                QuotaSound::Alert => Some(Sound::QuotaAlert),
                QuotaSound::Battery => Some(Sound::QuotaBattery),
            },
            _ => None,
        }
    };
    Delivery {
        notification: settings.notifications && !settings.muted,
        sound,
    }
}

struct AppState {
    runtime: Arc<Runtime>,
    tray: AtomicBool,
    visibility: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    alerts: Mutex<Alerts>,
}

#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Snapshot {
    state.runtime.snapshot()
}
#[tauri::command]
fn preferences(state: tauri::State<AppState>) -> Settings {
    state.runtime.preferences()
}
#[tauri::command]
fn refresh(state: tauri::State<AppState>) {
    state.runtime.request_refresh();
}
#[tauri::command]
fn save_preferences(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    settings: Settings,
) -> Result<(), String> {
    settings.validate()?;
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(settings.always_on_top)
            .map_err(|_| "window-update-failed")?;
    }
    state.runtime.update_settings(settings)
}
#[tauri::command]
fn resize(app: tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    if !width.is_finite()
        || !height.is_finite()
        || !(240.0..=600.0).contains(&width)
        || !(40.0..=900.0).contains(&height)
    {
        return Err("invalid-size".into());
    }
    if let Some(window) = app.get_webview_window("main") {
        let limit = window
            .current_monitor()
            .ok()
            .flatten()
            .map(|m| m.work_area().size.height as f64 / m.scale_factor() * 0.7)
            .unwrap_or(600.0);
        window
            .set_size(tauri::LogicalSize::new(width, height.min(limit)))
            .map_err(|_| "window-update-failed")?;
        ensure_visible(&window).map_err(|_| "window-update-failed")?;
    }
    Ok(())
}
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn set_visible(app: &tauri::AppHandle, visible: bool) {
    if let Some(window) = app.get_webview_window("main") {
        if visible {
            let _ = ensure_visible(&window);
            let _ = window.show();
            let _ = window.set_focus();
        } else {
            let _ = window.hide();
        }
    }
    let state = app.state::<AppState>();
    let was_hidden = state.runtime.hidden.swap(!visible, Ordering::Relaxed);
    if let Ok(item) = state.visibility.lock()
        && let Some(item) = item.as_ref()
    {
        let _ = item.set_checked(visible);
    }
    if visible && was_hidden {
        let _ = app.emit("status", state.runtime.snapshot());
    }
}

fn show(app: &tauri::AppHandle) {
    set_visible(app, true);
}

fn toggle(app: &tauri::AppHandle) {
    let visible = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    set_visible(app, !visible);
}

fn clamp(
    desired: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    monitors: &[tauri::Monitor],
) -> Option<PhysicalPosition<i32>> {
    let monitor = monitors
        .iter()
        .find(|monitor| {
            let start = monitor.position();
            let end_x = i64::from(start.x) + i64::from(monitor.size().width);
            let end_y = i64::from(start.y) + i64::from(monitor.size().height);
            i64::from(desired.x) >= i64::from(start.x)
                && i64::from(desired.x) < end_x
                && i64::from(desired.y) >= i64::from(start.y)
                && i64::from(desired.y) < end_y
        })
        .or_else(|| monitors.first())?;
    let area = monitor.work_area();
    Some(clamp_to_area(desired, size, area.position, area.size))
}

fn clamp_to_area(
    desired: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    start: PhysicalPosition<i32>,
    available: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let max_x = i64::from(start.x) + i64::from(available.width.saturating_sub(size.width));
    let max_y = i64::from(start.y) + i64::from(available.height.saturating_sub(size.height));
    PhysicalPosition::new(
        i64::from(desired.x).clamp(i64::from(start.x), max_x.max(i64::from(start.x))) as i32,
        i64::from(desired.y).clamp(i64::from(start.y), max_y.max(i64::from(start.y))) as i32,
    )
}

fn ensure_visible(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let desired = window.outer_position()?;
    let position = clamp(desired, window.outer_size()?, &window.available_monitors()?);
    if let Some(position) = position.filter(|position| *position != desired) {
        window.set_position(position)?;
    }
    Ok(())
}

fn install_tray(app: &tauri::App) -> tauri::Result<bool> {
    let visibility_item =
        CheckMenuItem::with_id(app, "visibility", "Show / Hide", true, true, None::<&str>)?;
    let refresh_item = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &visibility_item,
            &refresh_item,
            &settings_item,
            &separator,
            &quit_item,
        ],
    )?;
    let Some(icon) = app.default_window_icon().cloned() else {
        return Ok(false);
    };
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Codex Status")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "visibility" => toggle(app),
            "refresh" => app.state::<AppState>().runtime.request_refresh(),
            "settings" => {
                show(app);
                let _ = app.emit("open-settings", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                toggle(tray.app_handle());
            }
        })
        .build(app)?;
    if let Ok(mut item) = app.state::<AppState>().visibility.lock() {
        *item = Some(visibility_item);
    }
    Ok(true)
}

pub fn run() {
    let closing = Arc::new(AtomicBool::new(false));
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            preferences,
            refresh,
            save_preferences,
            resize,
            quit
        ])
        .setup(|app| {
            configure_presence(app);
            let handle = app.handle().clone();
            sound::prepare(&handle);
            let path = app.path().app_config_dir()?.join("settings.json");
            let runtime = Runtime::new(path, move |snapshot| {
                if let Some(state) = handle.try_state::<AppState>() {
                    if !state.runtime.hidden.load(Ordering::Relaxed) {
                        let _ = handle.emit("status", &snapshot);
                    }
                    let deliveries = state
                        .runtime
                        .settings
                        .lock()
                        .ok()
                        .and_then(|preferences| {
                            state.alerts.lock().ok().map(|mut alerts| {
                                alerts
                                    .observe(&snapshot, preferences.low_quota)
                                    .into_iter()
                                    .map(|alert| {
                                        let delivery = delivery(&preferences, alert.kind);
                                        (alert, delivery)
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .unwrap_or_default();
                    for (alert, delivery) in deliveries {
                        if let Some(sound) = delivery.sound {
                            sound::play(&handle, sound);
                        }
                        if delivery.notification {
                            let _ = handle
                                .notification()
                                .builder()
                                .title(alert.title)
                                .body(alert.body)
                                .show();
                        }
                    }
                }
            });
            app.manage(AppState {
                runtime: runtime.clone(),
                tray: AtomicBool::new(false),
                visibility: Mutex::new(None),
                alerts: Mutex::new(Alerts::default()),
            });
            tauri::async_runtime::spawn(async move {
                runtime.start();
            });
            if let Some(window) = app.get_webview_window("main") {
                let preferences = app.state::<AppState>().runtime.preferences();
                window.set_always_on_top(preferences.always_on_top)?;
                if let Some((x, y)) = preferences.position {
                    window.set_position(PhysicalPosition::new(x, y))?;
                }
                let _ = ensure_visible(&window);
                window.show()?;
            }
            let tray = install_tray(app).unwrap_or(false);
            app.state::<AppState>().tray.store(tray, Ordering::Relaxed);
            Ok(())
        })
        .on_window_event(|window: &Window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let state = window.app_handle().state::<AppState>();
                    if state.tray.load(Ordering::Relaxed) {
                        api.prevent_close();
                        set_visible(window.app_handle(), false);
                    }
                }
                tauri::WindowEvent::Moved(position) => {
                    window
                        .app_handle()
                        .state::<AppState>()
                        .runtime
                        .remember_position((position.x, position.y));
                }
                tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(window) = window.app_handle().get_webview_window("main") {
                        let _ = ensure_visible(&window);
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("Application initialization failed");
    application.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && !closing.swap(true, Ordering::Relaxed)
        {
            api.prevent_exit();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let runtime = app.state::<AppState>().runtime.clone();
                let _ = runtime.persist();
                if runtime.close().await {
                    app.exit(0);
                } else {
                    std::process::exit(0);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_position_stays_inside_an_available_monitor() {
        let monitors = vec![];
        assert_eq!(
            clamp(
                PhysicalPosition::new(4, 5),
                PhysicalSize::new(300, 160),
                &monitors
            ),
            None
        );
        assert_eq!(
            clamp_to_area(
                PhysicalPosition::new(1_900, 1_000),
                PhysicalSize::new(540, 720),
                PhysicalPosition::new(0, 0),
                PhysicalSize::new(2_560, 1_400),
            ),
            PhysicalPosition::new(1_900, 680),
        );
    }

    #[test]
    fn completion_sound_has_an_independent_setting_and_respects_mute() {
        let mut settings = Settings {
            notifications: false,
            ..Settings::default()
        };
        assert_eq!(
            delivery(&settings, AlertKind::Completion),
            Delivery {
                notification: false,
                sound: Some(Sound::CompletionBell),
            }
        );
        assert_eq!(delivery(&settings, AlertKind::Status).sound, None);
        settings.completion_sound = CompletionSound::Ding;
        assert_eq!(
            delivery(&settings, AlertKind::Completion).sound,
            Some(Sound::CompletionDing)
        );
        settings.completion_sound = CompletionSound::Off;
        assert_eq!(delivery(&settings, AlertKind::Completion).sound, None);
        settings.completion_sound = CompletionSound::Bell;
        settings.muted = true;
        assert_eq!(delivery(&settings, AlertKind::Completion).sound, None);
    }

    #[test]
    fn approval_sound_can_be_disabled_and_respects_mute() {
        let mut settings = Settings {
            notifications: false,
            ..Settings::default()
        };
        assert_eq!(
            delivery(&settings, AlertKind::Approval),
            Delivery {
                notification: false,
                sound: Some(Sound::ApprovalBell),
            }
        );
        settings.approval_sound = ApprovalSound::Off;
        assert_eq!(delivery(&settings, AlertKind::Approval).sound, None);
        settings.approval_sound = ApprovalSound::Bell;
        settings.muted = true;
        assert_eq!(delivery(&settings, AlertKind::Approval).sound, None);
    }

    #[test]
    fn quota_sound_is_selectable_and_independent_of_visual_notifications() {
        let mut settings = Settings {
            notifications: false,
            ..Settings::default()
        };
        assert_eq!(
            delivery(&settings, AlertKind::Quota),
            Delivery {
                notification: false,
                sound: Some(Sound::QuotaBattery),
            }
        );
        settings.quota_sound = QuotaSound::Alert;
        assert_eq!(
            delivery(&settings, AlertKind::Quota).sound,
            Some(Sound::QuotaAlert)
        );
        settings.quota_sound = QuotaSound::Off;
        assert_eq!(delivery(&settings, AlertKind::Quota).sound, None);
    }
}
