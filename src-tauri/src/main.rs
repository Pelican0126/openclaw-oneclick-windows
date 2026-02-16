// Always use the Windows GUI subsystem to avoid spawning extra console windows.
// All diagnostics should go to `%APPDATA%\\OpenClawInstaller\\logs`.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod commands;
mod models;
mod modules;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

use modules::{logger, paths, process};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_MENU_TOGGLE_ID: &str = "tray_toggle";
const TRAY_MENU_STOP_OPENCLAW_ID: &str = "tray_stop_openclaw";
const TRAY_MENU_EXIT_ID: &str = "tray_exit";

fn reveal_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let visible = window.is_visible().unwrap_or(true);
        if visible {
            let _ = window.hide();
        } else {
            reveal_main_window(app);
        }
    }
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    // Keep tray menu labels ASCII-only to avoid any source encoding issues on Windows.
    let toggle_item = MenuItem::with_id(
        app,
        TRAY_MENU_TOGGLE_ID,
        "Show/Hide Window",
        true,
        None::<&str>,
    )?;
    let stop_openclaw_item = MenuItem::with_id(
        app,
        TRAY_MENU_STOP_OPENCLAW_ID,
        "Stop OpenClaw",
        true,
        None::<&str>,
    )?;
    let exit_item = MenuItem::with_id(app, TRAY_MENU_EXIT_ID, "Exit", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&toggle_item, &stop_openclaw_item, &exit_item])?;

    let mut tray_builder = TrayIconBuilder::with_id("openclaw-installer-tray")
        .tooltip("OpenClaw Installer")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_TOGGLE_ID => toggle_main_window(app),
            TRAY_MENU_STOP_OPENCLAW_ID => {
                // Best effort: stop OpenClaw but keep the installer running in tray.
                match process::end_openclaw() {
                    Ok(result) => logger::info(&format!("Tray stop OpenClaw: {}", result.message)),
                    Err(err) => logger::warn(&format!("Tray stop OpenClaw failed: {err}")),
                }
            }
            TRAY_MENU_EXIT_ID => {
                // Exit the installer UI. OpenClaw is managed explicitly (Maintenance or tray stop item),
                // so we do not forcibly stop it here.
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
                toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    tray_builder.build(app)?;
    Ok(())
}

fn main() {
    if let Err(err) = paths::ensure_dirs() {
        eprintln!("Failed to initialize directories: {err}");
    }
    logger::info("OpenClaw Installer started.");

    tauri::Builder::default()
        .setup(|app| {
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(err) = window.hide() {
                    logger::error(&format!("Failed to hide window to tray: {err}"));
                } else {
                    logger::info("Main window hidden to system tray.");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_env,
            commands::install_env,
            commands::release_port,
            commands::get_install_lock_info,
            commands::install_openclaw,
            commands::uninstall_openclaw,
            commands::configure,
            commands::get_current_config,
            commands::update_provider_api_key,
            commands::start,
            commands::stop,
            commands::end_openclaw,
            commands::restart,
            commands::health_check,
            commands::get_status,
            commands::backup,
            commands::list_backups,
            commands::rollback,
            commands::upgrade,
            commands::switch_model,
            commands::security_check,
            commands::list_logs,
            commands::read_log,
            commands::export_log,
            commands::clear_cache,
            commands::clear_sessions,
            commands::reload_config,
            commands::open_management_url,
            commands::open_path,
            commands::logs_dir_path,
            commands::donate_wechat_qr,
            commands::list_skill_catalog,
            commands::list_model_catalog,
            commands::setup_telegram_pair
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

