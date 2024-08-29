use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;    // TrayIconEvent
use image;

use crate::bluetooth::{find_bluetooth_devices, get_bluetooth_info};

use std::sync::{Arc, Mutex};
use std::thread;

const ICON_DATA: &[u8] = include_bytes!("../resources/logo.ico");

pub fn show_systray() -> windows::core::Result<()> {
    loop_systray()
}

fn loop_systray() -> windows::core::Result<()> {
    let mut event_loop = EventLoopBuilder::new().build();
    let event_loop_proxy = event_loop.create_proxy();

    let icon = load_icon();

    let tray_menu = Menu::new();
    let menu_separator = PredefinedMenuItem::separator();
    let menu_quit = MenuItem::new("Quit", true, None);

    let devices = find_bluetooth_devices()?;

    let (t, m) = get_bluetooth_info(devices.clone()).unwrap();
    let tooltip = Arc::new(Mutex::new(t));
    let menu_items = Arc::new(Mutex::new(m));

    {
        let menu_items_lock = menu_items.lock().unwrap();
        
        menu_items_lock.iter().for_each(|i| {
            let item = MenuItem::new(i, true, None);
            tray_menu.append(&item).unwrap();
        });
        tray_menu.append(&menu_separator).unwrap();
        tray_menu.append(&menu_quit).unwrap();
    }
    
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(tooltip.lock().unwrap().join("\n"))
        .with_icon(icon)
        .build()
        .unwrap();

    let menu_channel = MenuEvent::receiver();
    // let tray_channel = TrayIconEvent::receiver();

    {
        let devices_clone = devices.clone();
        let tooltip_clone = Arc::clone(&tooltip);
        let menu_items_clone = Arc::clone(&menu_items);

        thread::spawn(move || {
            loop {
                println!("thread: wait");
                thread::sleep(std::time::Duration::from_millis(60000));
                println!("thread: running");
                let devices_clone = devices_clone.clone();
                let (tooltip_result, menu_items_result) = 
                    get_bluetooth_info(devices_clone).unwrap();
                
                match (tooltip_clone.try_lock(), menu_items_clone.try_lock()) {
                    (Ok(mut tooltip), Ok(mut menu_items)) => {
                        *tooltip = tooltip_result;
                        *menu_items = menu_items_result.clone();
                        println!("thread: update");
                        event_loop_proxy.send_event(()).ok();
                    },
                    _ => println!("thread: locked"),
                };                
            }
        });
    }

    let return_code = event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == menu_quit.id()  {
                println!("process exist");
                *control_flow = ControlFlow::Exit;
            };
        };

        // if let Ok(tray_event) = tray_channel.try_recv() {
        //     if tray_event.id() == tray_icon.id() {
        //         return
        //     };
        // };

        if event == tao::event::Event::UserEvent(()) {
            println!("update tooltip and menu");
            let tray_menu = Menu::new();

            let tooltip_lock = tooltip.lock().unwrap();
            let menu_items_lock = menu_items.lock().unwrap();

            menu_items_lock.iter().for_each(|i| {
                let item = MenuItem::new(i, true, None);
                tray_menu.append(&item).unwrap();
            });
            tray_menu.append(&menu_separator).unwrap();
            tray_menu.append(&menu_quit).unwrap();

            tray_icon.set_tooltip(Some(tooltip_lock.join("\n"))).unwrap();
            tray_icon.set_menu(Some(Box::new(tray_menu)));
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
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .expect("Failed to open icon")
}