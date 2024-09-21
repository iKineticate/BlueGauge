use image;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::run_return::EventLoopExtRunReturn;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder}; // TrayIconEvent

use crate::bluetooth::{find_bluetooth_devices, get_bluetooth_info, BluetoothInfo};

use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, sleep};
use std::time::Duration;

const ICON_DATA: &[u8] = include_bytes!("../resources/logo.ico");

pub fn show_systray() -> windows::core::Result<()> {
    loop_systray()
}

fn loop_systray() -> windows::core::Result<()> {
    let mut event_loop = EventLoopBuilder::new().build();
    let event_loop_proxy = event_loop.create_proxy();

    let bluetooth_devices = find_bluetooth_devices()?;
    let bluetooth_devices_info = get_bluetooth_info(bluetooth_devices.0, bluetooth_devices.1)?;

    let (tooltip, items) = convert_tray_info(bluetooth_devices_info);
    let tray_tooltip = Arc::new(Mutex::new(tooltip));
    let menu_items = Arc::new(Mutex::new(items));

    let mut tray_icon = TrayIconBuilder::new()
        .with_menu_on_left_click(true)
        .with_icon(load_icon())
        .build()
        .unwrap();

    let menu_separator = PredefinedMenuItem::separator();
    let menu_quit = MenuItem::new("Quit", true, None);

    tray_icon = update_tray_icon(
        tray_icon.clone(),
        &menu_quit,
        &menu_separator,
        tray_tooltip.lock().unwrap(),
        menu_items.lock().unwrap(),
    );

    let tray_tooltip_clone = Arc::clone(&tray_tooltip);
    let menu_items_clone = Arc::clone(&menu_items);
    thread_update_info(tray_tooltip_clone, menu_items_clone, event_loop_proxy)?;

    let menu_channel = MenuEvent::receiver();
    // let tray_channel = TrayIconEvent::receiver();

    let return_code = event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == menu_quit.id() {
                println!("process exist");
                *control_flow = ControlFlow::Exit;
            };
        };

        match event {
            tao::event::Event::UserEvent(()) => {
                println!("Update tray information");
                tray_icon = update_tray_icon(
                    tray_icon.clone(),
                    &menu_quit,
                    &menu_separator,
                    tray_tooltip.lock().unwrap(),
                    menu_items.lock().unwrap(),
                );
            }
            _ => (),
        };
    });

    if return_code != 0 {
        std::process::exit(return_code);
    };

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

fn convert_tray_info(bluetooth_devices_info: Vec<BluetoothInfo>) -> (Vec<String>, Vec<String>) {
    let mut tray_tooltip_result = Vec::new();
    let mut menu_items_result = Vec::new();
    for blue_info in bluetooth_devices_info {
        match blue_info.status {
            true => {
                tray_tooltip_result
                    .insert(0, format!("🟢 {} - {}%", blue_info.name, blue_info.battery));
                menu_items_result
                    .insert(0, format!("🔗 {} - {}%", blue_info.name, blue_info.battery))
            }
            false => {
                tray_tooltip_result.push(format!("🔴 {} - {}%", blue_info.name, blue_info.battery));
                menu_items_result.push(format!("     {} - {}%", blue_info.name, blue_info.battery))
            }
        }
    }
    (tray_tooltip_result, menu_items_result)
}

fn thread_update_info(
    tray_tooltip_clone: Arc<Mutex<Vec<String>>>,
    menu_items_clone: Arc<Mutex<Vec<String>>>,
    event_loop_proxy: EventLoopProxy<()>,
) -> windows::core::Result<()> {
    thread::spawn(move || loop {
        println!("thread: wait");
        sleep(Duration::from_secs(30));
        println!("thread: running");
        let bluetooth_devices = find_bluetooth_devices().unwrap();
        let bluetooth_devices_info = get_bluetooth_info(bluetooth_devices.0, bluetooth_devices.1).unwrap();
        let (tooltip, items) = convert_tray_info(bluetooth_devices_info);

        match (tray_tooltip_clone.lock(), menu_items_clone.lock()) {
            (Ok(mut tray_tooltip), Ok(mut menu_items)) => {
                *tray_tooltip = tooltip;
                *menu_items = items;
                println!("thread: update");
                event_loop_proxy.send_event(()).ok();
            }
            _ => println!("thread: locked"),
        };
    });

    Ok(())
}

fn update_tray_icon(
    tray_icon: TrayIcon,
    menu_quit: &MenuItem,
    menu_separator: &PredefinedMenuItem,
    tray_tooltip_lock: MutexGuard<Vec<String>>,
    menu_items_lock: MutexGuard<Vec<String>>,
) -> TrayIcon {
    let tray_menu = Menu::new();
    menu_items_lock.iter().for_each(|i| {
        let item = MenuItem::new(i, true, None);
        tray_menu.append(&item).unwrap();
    });
    tray_menu.append(menu_separator).unwrap();
    tray_menu.append(menu_quit).unwrap();

    tray_icon
        .set_tooltip(Some(tray_tooltip_lock.join("\n")))
        .unwrap();
    tray_icon.set_menu(Some(Box::new(tray_menu)));

    tray_icon
}
