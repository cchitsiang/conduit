use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::provider::VpnStatus;

pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItemBuilder::with_id("quit", "Quit Conduit").build(app)?;
    let open = MenuItemBuilder::with_id("open", "Open Dashboard").build(app)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open)
        .item(&separator)
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
        .icon_as_template(true)
        .menu(&menu)
        .tooltip("Conduit - VPN Manager")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "quit" => {
                app.exit(0);
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
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
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

pub fn update_tray_menu(
    app: &AppHandle,
    statuses: &[VpnStatus],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = MenuBuilder::new(app);

    for status in statuses {
        let dot = if status.connected { "●" } else { "○" };
        let state_text = if status.connected {
            "Connected"
        } else {
            "Disconnected"
        };
        let label = format!("{} {}  {}", dot, status.provider, state_text);

        let item_id = format!("toggle_{}", status.provider.to_lowercase());
        let item = MenuItemBuilder::with_id(&item_id, &label).build(app)?;
        builder = builder.item(&item);

        if status.connected {
            if let Some(ip) = &status.ip {
                let detail = MenuItemBuilder::with_id(
                    format!("detail_{}", status.provider.to_lowercase()),
                    format!("   {}", ip),
                )
                .enabled(false)
                .build(app)?;
                builder = builder.item(&detail);
            }
        }

        builder = builder.item(&tauri::menu::PredefinedMenuItem::separator(app)?);
    }

    let open = MenuItemBuilder::with_id("open", "Open Dashboard").build(app)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Conduit").build(app)?;

    let menu = builder
        .item(&open)
        .item(&separator)
        .item(&quit)
        .build()?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
    }

    Ok(())
}
