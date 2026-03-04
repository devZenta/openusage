//! Linux/Wayland window management.
//!
//! Replaces the macOS NSPanel behaviour with standard Tauri window operations.
//! The window is kept always-on-top and is positioned near the system-tray icon
//! so that it feels like a dropdown panel, consistent with the macOS experience.

use tauri::{AppHandle, Manager, Position, Size, WindowEvent};

/// Show the main window and bring it to the foreground.
pub fn show_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(true);
    }
}

/// Hide the main window.
pub fn hide_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// Toggle main window visibility.
pub fn toggle_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.set_always_on_top(true);
        }
    }
}

/// Returns whether the main window is currently visible.
pub fn is_visible(app_handle: &AppHandle) -> bool {
    app_handle
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false)
}

/// Register a focus-lost handler so the popup auto-hides when the user
/// clicks outside it — mirroring the macOS NSPanel behaviour.
///
/// Call this once from the Tauri `setup` block.
pub fn setup_focus_hide(app_handle: &AppHandle) {
    let window = match app_handle.get_webview_window("main") {
        Some(w) => w,
        None => {
            log::warn!("linux_window::setup_focus_hide: main window not found");
            return;
        }
    };

    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            log::debug!("linux_window: focus lost, hiding window");
            let _ = window.hide();
        }
    });
}

/// Position the window so it appears just below the tray icon.
///
/// On Wayland the tray-icon position reported by Tauri is in physical pixels.
/// We convert to physical coordinates and place the window centred under the icon.
pub fn position_window_at_tray_icon(app_handle: &AppHandle, icon_position: Position, icon_size: Size) {
    let window = match app_handle.get_webview_window("main") {
        Some(w) => w,
        None => return,
    };

    let (icon_x, icon_y) = match &icon_position {
        Position::Physical(pos) => (pos.x as f64, pos.y as f64),
        Position::Logical(pos) => (pos.x, pos.y),
    };
    let (icon_w, icon_h) = match &icon_size {
        Size::Physical(s) => (s.width as f64, s.height as f64),
        Size::Logical(s) => (s.width, s.height),
    };

    let scale = window.scale_factor().unwrap_or(1.0);

    let panel_width = match (window.outer_size(), window.scale_factor()) {
        (Ok(s), Ok(win_scale)) => s.width as f64 / win_scale,
        _ => {
            let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
                .expect("tauri.conf.json must be valid JSON");
            conf["app"]["windows"][0]["width"]
                .as_f64()
                .expect("width must be set in tauri.conf.json")
        }
    };

    let icon_center_x = icon_x + (icon_w / 2.0);
    let panel_x = icon_center_x - (panel_width * scale / 2.0);
    let panel_y = icon_y + icon_h;

    let _ = window.set_position(tauri::PhysicalPosition::new(panel_x as i32, panel_y as i32));
}
