use crate::{
    settings::Settings,
    sources::Runtime,
    status::{Snapshot, alerts::Alerts},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{
    Emitter, Manager, PhysicalPosition, PhysicalSize, Window,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_notification::NotificationExt;

struct AppState {
    runtime: Arc<Runtime>,
    tray: AtomicBool,
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
            .map(|m| m.size().height as f64 / m.scale_factor() * 0.7)
            .unwrap_or(600.0);
        window
            .set_size(tauri::LogicalSize::new(width, height.min(limit)))
            .map_err(|_| "window-update-failed")?;
    }
    Ok(())
}
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = ensure_visible(&window);
        let _ = window.show();
        let _ = window.set_focus();
        app.state::<AppState>()
            .runtime
            .hidden
            .store(false, Ordering::Relaxed);
    }
}

fn toggle(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            app.state::<AppState>()
                .runtime
                .hidden
                .store(true, Ordering::Relaxed);
        } else {
            show(app);
        }
    }
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
    let start = monitor.position();
    let max_x = i64::from(start.x) + i64::from(monitor.size().width.saturating_sub(size.width));
    let max_y = i64::from(start.y) + i64::from(monitor.size().height.saturating_sub(size.height));
    Some(PhysicalPosition::new(
        i64::from(desired.x).clamp(i64::from(start.x), max_x.max(i64::from(start.x))) as i32,
        i64::from(desired.y).clamp(i64::from(start.y), max_y.max(i64::from(start.y))) as i32,
    ))
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
    let toggle_item = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
    let refresh_item = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle_item,
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
            "toggle" => toggle(app),
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
            let handle = app.handle().clone();
            let path = app.path().app_config_dir()?.join("settings.json");
            let runtime = Runtime::new(path, move |snapshot| {
                let _ = handle.emit("status", &snapshot);
                if let Some(state) = handle.try_state::<AppState>() {
                    let preferences = state.runtime.preferences();
                    let alerts = state
                        .alerts
                        .lock()
                        .map(|mut alerts| alerts.observe(&snapshot, preferences.low_quota))
                        .unwrap_or_default();
                    if preferences.notifications && !preferences.muted {
                        for alert in alerts {
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
                        let _ = window.hide();
                        state.runtime.hidden.store(true, Ordering::Relaxed);
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
    }
}
