mod dsh;

use dsh::DshManager;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WebviewWindow,
};

struct AppState {
    manager: DshManager,
    exit_stops_dsh: Mutex<bool>,
}

#[tauri::command]
fn dsh_status(state: State<'_, AppState>) -> dsh::DshStatus {
    state.manager.status()
}

#[tauri::command]
fn dsh_start(state: State<'_, AppState>) -> Result<dsh::DshStatus, String> {
    state.manager.start()
}

#[tauri::command]
fn dsh_stop(state: State<'_, AppState>) -> Result<dsh::DshStatus, String> {
    state.manager.stop()
}

#[tauri::command]
fn dsh_launcher() -> Option<String> {
    DshManager::resolve_dsh().map(|p| p.display().to_string())
}

#[tauri::command]
fn open_harness(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let url = state.manager.url();
    let parsed = url.parse::<tauri::Url>().map_err(|e| e.to_string())?;
    let window = main_window(&app)?;
    window.navigate(parsed).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_dashboard(app: AppHandle) -> Result<(), String> {
    let window = main_window(&app)?;
    navigate_to_dashboard(&window).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_exit_stops_dsh(app: AppHandle, enabled: bool) {
    let state = app.state::<AppState>();
    *state.exit_stops_dsh.lock().unwrap() = enabled;
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main").ok_or_else(|| "找不到主窗口".to_string())
}

fn navigate_to_dashboard(window: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(debug_assertions)]
    {
        let dev_url = std::env::var("BETTER_DSH_DEV_URL").unwrap_or_else(|_| "http://localhost:1420".into());
        window.navigate(dev_url.parse::<tauri::Url>().unwrap())
    }
    #[cfg(not(debug_assertions))]
    {
        window.navigate("tauri://localhost".parse::<tauri::Url>().unwrap())
    }
}

fn show_main(app: &AppHandle) {
    if let Ok(w) = main_window(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_main(app: &AppHandle) {
    if let Ok(w) = main_window(app) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show_main(app);
        }
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏窗口", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "打开 Harness", true, None::<&str>)?;
    let dashboard = MenuItem::with_id(app, "dashboard", "控制面板", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &open, &dashboard, &sep, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("missing default window icon");

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_main(app),
            "open" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = open_harness(app.clone(), state);
                }
            }
            "dashboard" => {
                let _ = open_dashboard(app.clone());
            }
            "quit" => {
                let should_stop = app
                    .try_state::<AppState>()
                    .map(|s| *s.exit_stops_dsh.lock().unwrap())
                    .unwrap_or(true);
                if should_stop {
                    if let Some(s) = app.try_state::<AppState>() {
                        let _ = s.manager.stop();
                    }
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let manager = DshManager::new(dsh::DEFAULT_PORT, data_dir);
            app.manage(AppState {
                manager,
                exit_stops_dsh: Mutex::new(true),
            });
            setup_tray(app.handle())?;

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let state = handle.state::<AppState>();
                let _ = state.manager.start();
                let status = state.manager.status();
                let _ = handle.emit("dsh-status", status);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            dsh_status,
            dsh_start,
            dsh_stop,
            dsh_launcher,
            open_harness,
            open_dashboard,
            set_exit_stops_dsh
        ])
        .run(tauri::generate_context!())
        .expect("error while running better-dsh");
}

