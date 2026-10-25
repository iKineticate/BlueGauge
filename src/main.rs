#![allow(non_snake_case)]
#![cfg(target_os = "windows")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bluetooth;
mod config;
mod language;
mod notify;
mod single_instance;
mod startup;
mod theme;
mod tray;
mod util;

use crate::bluetooth::{
    info::{BT_INFO_MAP, init_bluetooth_info},
    watch::Watcher,
};
use crate::config::{CONFIG, EXE_PATH, TrayIconStyle};
use crate::notify::{NotifyEvent, notify};
use crate::single_instance::SingleInstance;
use crate::theme::{SystemTheme, ThemeWatcher};
use crate::tray::{
    convert_tray_info, create_tray, create_tray_icon,
    menu::{
        MenuGroup, about,
        handler::handle_menu_event,
        item::{MenuAction, create_menu},
        registry::MenuRegistry,
    },
};

use std::collections::HashSet;
use std::ffi::OsString;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::anyhow;
use log::{error, info};
use tray_icon::{TrayIcon, menu::MenuEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PM_REMOVE, PeekMessageW, TranslateMessage,
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::WindowId,
};

pub static PROXY: OnceLock<EventLoopProxy<UserEvent>> = OnceLock::new();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _single_instance = SingleInstance::new()?;

    std::panic::set_hook(Box::new(|info| {
        error!("⚠️ Panic: {info}");
        notify(format!("⚠️ Panic: {info}"));
    }));

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    init_bluetooth_info().await?;

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;

    let proxy = event_loop.create_proxy();

    PROXY.get_or_init(|| proxy.clone());

    MenuEvent::set_event_handler(Some(move |event| {
        proxy
            .send_event(UserEvent::MenuEvent(event))
            .expect("Failed to send MenuEvent");
    }));

    let mut app = App::new().await;
    event_loop.run_app(&mut app)?;

    Ok(())
}

struct App {
    bluetooth_watcher: Option<Watcher>,
    exit_threads: Arc<AtomicBool>,
    /// 存储已经通知过的低电量设备（地址），避免再次通知
    notified_devices: Arc<Mutex<HashSet</* Address */ u64>>>,
    menu_registry: MenuRegistry<MenuGroup>,
    system_theme: Arc<RwLock<SystemTheme>>,
    theme_watcher: Option<ThemeWatcher>,
    tray: TrayIcon,
    /// 托盘菜单更新轮询标志，避免在菜单打开时强制刷新导致体验不佳
    tray_menu_update_polling: Arc<AtomicBool>,
}

impl App {
    async fn new() -> Self {
        {
            Self::send_low_battery_notification();
            Self::show_lowest_battery_device();
        }

        let mut menu_registry = MenuRegistry::new();

        let tray = create_tray(&mut menu_registry).expect("Failed to create tray");

        Self {
            bluetooth_watcher: None,
            exit_threads: Arc::new(AtomicBool::new(false)),
            notified_devices: Arc::new(Mutex::new(HashSet::new())),
            menu_registry,
            system_theme: Arc::new(RwLock::new(SystemTheme::get())),
            theme_watcher: None,
            tray,
            tray_menu_update_polling: Arc::new(AtomicBool::new(false)),
        }
    }

    fn send_low_battery_notification() {
        BT_INFO_MAP.iter().for_each(|entry| {
            let _ = PROXY
                .get()
                .unwrap()
                .send_event(UserEvent::Notify(NotifyEvent::LowBattery(
                    entry.name.clone(),
                    entry.battery,
                    entry.address,
                )));
        });
    }

    fn show_lowest_battery_device() {
        let should_show_lowest_battery_device = CONFIG
            .read()
            .unwrap()
            .tray_options
            .show_lowest_battery_device;

        if should_show_lowest_battery_device
            && let Some(entry) = BT_INFO_MAP
                .iter()
                .filter(|entry| entry.status)
                .min_by_key(|entry| entry.battery)
        {
            let (address, info) = entry.pair();
            info!("Show Lowest Battery Device: {}", info.name);

            let mut config = CONFIG.write().unwrap();

            let tray_icon_style = &mut config.tray_options.tray_icon_style;

            if !tray_icon_style.update_address(*address) {
                *tray_icon_style = TrayIconStyle::number_icon(*address, None);
            }

            config.save_toml();
        }
    }
}

#[derive(Debug)]
pub enum UserEvent {
    Exit,
    MenuEvent(MenuEvent),
    Notify(NotifyEvent),
    UnCheckAboutIconMenu,
    UnCheckDeviceMenu,
    UpdateTray,
    UpdateTrayMenu,
    UpdateTrayIcon,
    UpdateTrayTooltip,
    Refresh,
    Restart,
    ShowAboutDialog,
}

impl App {
    fn start_watch_devices(&mut self) {
        self.stop_watch_devices();
        let mut watch = Watcher::new();
        watch.start();
        self.bluetooth_watcher = Some(watch);
    }

    fn stop_watch_devices(&mut self) {
        if let Some(mut bluetooth_watcher) = self.bluetooth_watcher.take() {
            bluetooth_watcher.stop()
        }
    }

    fn start_watch_theme(&mut self) {
        let exit_threads = Arc::clone(&self.exit_threads);
        let system_theme = Arc::clone(&self.system_theme);
        let mut theme_watcher = ThemeWatcher::new(exit_threads, system_theme);
        theme_watcher.start();
        self.theme_watcher = Some(theme_watcher);
    }

    fn stop_watch_theme(&mut self) {
        if let Some(mut theme_watcher) = self.theme_watcher.take() {
            theme_watcher.stop()
        }
    }

    fn exit(&mut self) {
        self.exit_threads.store(true, Ordering::Relaxed);
        self.stop_watch_devices();
        self.stop_watch_theme();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        self.start_watch_devices();
        self.start_watch_theme();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if event == WindowEvent::CloseRequested {
            self.exit();
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::UnCheckDeviceMenu => {
                if let Some(radio_group) = self
                    .menu_registry
                    .get_radio_menu_from_group(&MenuGroup::RadioDevice)
                {
                    radio_group.iter().for_each(|m| m.set_checked(false));
                }
            }
            // 取消勾选 [显示最低电量设备] 和 [设置连接配色]
            UserEvent::UnCheckAboutIconMenu => {
                if let Some(menu_control) = self
                    .menu_registry
                    .get_menu_kind_from_id(&MenuAction::ShowLowestBatteryDevice.id())
                    .and_then(|menu_kind| menu_kind.as_check_menuitem())
                {
                    menu_control.set_checked(false);
                }

                if let Some(menu_control) = self
                    .menu_registry
                    .get_menu_kind_from_id(&MenuAction::SetIconConnectColor.id())
                    .and_then(|menu_kind| menu_kind.as_check_menuitem())
                {
                    menu_control.set_checked(false);
                }
            }
            UserEvent::Exit => {
                self.exit();
                event_loop.exit();
            }
            UserEvent::MenuEvent(event) => {
                if let Err(e) = self
                    .menu_registry
                    .handle_event(event.id())
                    .map_err(|e| anyhow!("{e}"))
                    .and_then(handle_menu_event)
                {
                    error!("Failed to handle menu event: {e}");
                }
            }
            UserEvent::Notify(notify_event) => notify_event.send(self.notified_devices.clone()),
            UserEvent::UpdateTrayIcon => {
                Self::show_lowest_battery_device();

                let Some(icon) = create_tray_icon() else {
                    return;
                };

                let _ = self.tray.set_icon(Some(icon));
            }
            UserEvent::UpdateTrayMenu => {
                pump_messages();

                // 如果托盘菜单正在显示，则推迟更新，避免菜单被强制关闭刷新影响体验
                if self.tray.is_menu_showing().unwrap_or_default() {
                    info!("Tray menu is showing, deferring update");

                    if !self.tray_menu_update_polling.swap(true, Ordering::Relaxed) {
                        let tray_menu_update_polling = self.tray_menu_update_polling.clone();
                        std::thread::spawn(move || {
                            let proxy = PROXY.get().unwrap();
                            while tray_menu_update_polling.load(Ordering::Relaxed) {
                                std::thread::sleep(std::time::Duration::from_millis(400));
                                let _ = proxy.send_event(UserEvent::UpdateTrayMenu);
                            }
                        });
                    }

                    return;
                }

                self.tray_menu_update_polling
                    .store(false, Ordering::Relaxed);

                match create_menu(&mut self.menu_registry) {
                    Ok(tray_menu) => {
                        self.tray.set_menu(Some(Box::new(tray_menu)));
                    }
                    Err(e) => {
                        error!("Failed to create tray menu: {e}");
                        notify(format!("Failed to create tray menu - {e}"));
                    }
                }
            }
            UserEvent::UpdateTrayTooltip => {
                let bluetooth_tooltip_info = convert_tray_info();
                let _ = self.tray.set_tooltip(Some(bluetooth_tooltip_info));
            }
            UserEvent::UpdateTray => {
                // 不创建 UserEvent::ShowLowestBatteryDevice 事件，是因为 .send_event 是非阻塞的
                // 如果 UpdateTrayIcon 和 UpdateTrayMenu 同时发起，图标更新了但菜单可能未能及时更新
                Self::show_lowest_battery_device();

                let proxy = PROXY.get().unwrap();
                let _ = proxy.send_event(UserEvent::UpdateTrayIcon);
                let _ = proxy.send_event(UserEvent::UpdateTrayMenu);
                let _ = proxy.send_event(UserEvent::UpdateTrayTooltip);
            }
            UserEvent::Refresh => {
                CONFIG.write().unwrap().update();

                futures::executor::block_on(async {
                    init_bluetooth_info()
                        .await
                        .expect("Failed to init bt devices info")
                });

                Self::send_low_battery_notification();

                let _ = PROXY.get().unwrap().send_event(UserEvent::UpdateTray);
            }
            UserEvent::Restart => {
                let mut args_os: Vec<OsString> = std::env::args_os().collect();
                args_os.push("--restart".into()); // 添加重启标志（避免与单实例冲突）

                if let Err(e) = Command::new(&*EXE_PATH)
                    .args(args_os.iter().skip(1))
                    .spawn()
                {
                    notify(format!("Failed to restart app: {e}"));
                }

                let _ = PROXY.get().unwrap().send_event(UserEvent::Exit);
            }
            UserEvent::ShowAboutDialog => {
                let hwnd = self.tray.window_handle();
                about::show_about_dialog(hwnd as isize);
            }
        }
    }
}

/// Pump the Win32 message loop so tray icon events are dispatched.
fn pump_messages() {
    // 防止无限 re-entry
    let mut count = 0;
    let start = std::time::Instant::now();
    unsafe {
        let mut msg = std::mem::zeroed();
        while count < 10 && PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message != 0 {
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }
            count += 1;
            if start.elapsed().as_millis() > 10 {
                break;
            }
        }
    }
}
