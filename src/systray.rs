use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tray_icon::menu::{MenuEvent, MenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};
use image;

use crate::{find_bluetooth_devices, get_bluetooth_info};

const ICON_DATA: &[u8] = include_bytes!("../resources/logo.ico");

pub fn show_systray() -> windows::core::Result<()> {
    loop_systray()
}

fn loop_systray() -> windows::core::Result<()> {
    let mut event_loop = EventLoopBuilder::new().build();

    let icon = load_icon();

    let quit_i = MenuItem::new("Quit", true, None);

    let devices = find_bluetooth_devices()?;

    let (tooltip, tray_menu) = get_bluetooth_info(devices.clone()).unwrap();

    tray_menu.append(&quit_i).unwrap();

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(tooltip.join("\n"))
        .with_icon(icon)
        .build()
        .unwrap();

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    let return_code = event_loop.run_return(|_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        
        if let Ok(event) = tray_channel.try_recv() {
            if let TrayIconEvent::Enter { id, .. } = event {
                if id == tray_icon.id() {
                    if let Ok((new_tooltip, tray_menu)) = get_bluetooth_info(devices.clone()) {    // 目前获取蓝牙信息较慢，导致右键菜单阻塞事件循环
                        if new_tooltip != tooltip {
                            tray_menu.append(&quit_i).unwrap();
                            tray_icon.set_tooltip(Some(new_tooltip.join("\n"))).unwrap();
                            tray_icon.set_menu(Some(Box::new(tray_menu)));
                        }
                    }
                }
            }
        }

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_i.id()  {
                *control_flow = ControlFlow::Exit;
            }
        }
    });

    if return_code != 0 {
        std::process::exit(return_code);
    }

    Ok(())
}

fn load_icon() -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(ICON_DATA)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}