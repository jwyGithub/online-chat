mod chat;
mod config;
mod secret;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use chat::CancelState;

/// 切换主窗口显隐：已显示则隐藏；否则居中显示并聚焦。
fn toggle_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if matches!(win.is_visible(), Ok(true)) {
            let _ = win.hide();
        } else {
            let _ = win.center();
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// 强制显示并聚焦主窗口。
fn show_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// 运行时更新全局快捷键（设置里改快捷键后立即生效）。
#[tauri::command]
fn update_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(hotkey.as_str())
        .map_err(|e| format!("快捷键无效或被占用: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(CancelState::default())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::save_config,
            secret::set_api_key,
            secret::has_api_key,
            chat::send_chat,
            chat::cancel_chat,
            update_hotkey,
        ])
        .setup(|app| {
            let cfg = config::read_config(app.handle());

            // macOS：设为 Accessory，只在托盘显示、不占 Dock
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // 全局快捷键：任何应用内按下都会触发显隐切换
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |app, _shortcut, event| {
                        if event.state() == ShortcutState::Pressed {
                            toggle_window(app);
                        }
                    })
                    .build(),
            )?;
            if let Err(e) = app.global_shortcut().register(cfg.hotkey.as_str()) {
                eprintln!("注册快捷键失败: {e}");
            }

            // 托盘图标 + 菜单
            let toggle_i = MenuItem::with_id(app, "toggle", "显示/隐藏", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle_i, &settings_i, &quit_i])?;

            // 状态栏（托盘）图标：单色 template 图，macOS 会随亮/暗菜单栏自动反色，
            // 并在点击时高亮。图标被强制缩放到 18pt 高，故用 44px 方图保证 Retina 清晰。
            // include_image! 在编译期解码 PNG（路径相对 src-tauri/），无需额外 feature。
            let tray_icon = tauri::include_image!("icons/tray-icon.png");

            TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("QuickChat")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "settings" => {
                        show_window(app);
                        let _ = app.emit("navigate", "settings");
                    }
                    "toggle" => toggle_window(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // 失焦自动隐藏（Raycast 风格），可在设置里关闭
            if let WindowEvent::Focused(false) = event {
                let cfg = config::read_config(window.app_handle());
                if cfg.hide_on_blur {
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
