use crate::{PROXY, UserEvent, util::to_wide};

use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use image::Rgba;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_EVENT, WAIT_FAILED, WAIT_OBJECT_0},
        System::{
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_DWORD, REG_NOTIFY_CHANGE_LAST_SET,
                RRF_RT_REG_DWORD, RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue,
                RegOpenKeyExW,
            },
            Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects},
        },
    },
    core::PCWSTR,
};

const PERSONALIZE_REGISTRY_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const SYSTEM_USES_LIGHT_THEME_REGISTRY_KEY: &str = "SystemUsesLightTheme";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemTheme {
    Light,
    Dark,
}

impl SystemTheme {
    pub fn get() -> Self {
        let path = to_wide(PERSONALIZE_REGISTRY_KEY);
        let name = to_wide(SYSTEM_USES_LIGHT_THEME_REGISTRY_KEY);

        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut reg_dword = REG_DWORD;

        let ret = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_DWORD,
                Some(&mut reg_dword),
                Some(&mut value as *mut _ as *mut _),
                Some(&mut size as *mut _),
            )
        };

        if ret.is_err() {
            SystemTheme::Light
        } else {
            match value {
                0 => SystemTheme::Dark,
                _ => SystemTheme::Light,
            }
        }
    }

    pub fn get_font_color(&self) -> Rgba<u8> {
        match self {
            Self::Dark => Rgba([255, 255, 255, 255]),
            Self::Light => Rgba([31, 31, 31, 255]),
        }
    }
}

pub struct ThemeWatcher {
    exit_threads: Arc<AtomicBool>,
    system_theme: Arc<RwLock<SystemTheme>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    shut_down_handle: HANDLE,
}

impl ThemeWatcher {
    pub fn new(exit_threads: Arc<AtomicBool>) -> Self {
        let shut_down_handle =
            unsafe { CreateEventW(None, true, false, None).expect("Shutdown event create failed") };

        let system_theme = Arc::new(RwLock::new(SystemTheme::get()));

        Self {
            exit_threads,
            system_theme,
            thread_handle: None,
            shut_down_handle,
        }
    }

    pub fn stop(&mut self) {
        log::info!("Stopping the watch theme thread...");
        let _ = unsafe { SetEvent(self.shut_down_handle) };
        if let Some(handle) = self.thread_handle.take() {
            self.exit_threads.store(true, Ordering::Relaxed);
            handle.join().expect("Failed to join theme watcher thread");
        }
        let _ = unsafe { CloseHandle(self.shut_down_handle) };
    }

    pub fn start(&mut self) {
        let shut_down_handle = self.shut_down_handle.0 as isize;
        let thread_handle = {
            let exit_threads = self.exit_threads.clone();
            let system_theme = self.system_theme.clone();

            std::thread::spawn(move || {
                let mut hkey = HKEY::default();
                let path = to_wide(PERSONALIZE_REGISTRY_KEY);

                if let Err(e) = unsafe {
                    RegOpenKeyExW(
                        HKEY_CURRENT_USER,
                        PCWSTR(path.as_ptr()),
                        None,
                        KEY_NOTIFY,
                        &mut hkey,
                    )
                }
                .ok()
                {
                    log::error!("Failed to open registry key: {e}");
                    return;
                }

                while !exit_threads.load(Ordering::Relaxed) {
                    let registry_event = unsafe { CreateEventW(None, true, false, None) };

                    let Ok(watch_handle) = registry_event else {
                        log::error!("Failed to create event");
                        break;
                    };

                    let status = unsafe {
                        RegNotifyChangeKeyValue(
                            hkey,
                            false,
                            REG_NOTIFY_CHANGE_LAST_SET,
                            Some(watch_handle),
                            true, // 异步模式
                        )
                    };

                    if status.is_err() {
                        log::error!("RegNotifyChangeKeyValue failed: {}", status.0);
                        let _ = unsafe { CloseHandle(watch_handle) };
                        break;
                    }

                    let handles = [watch_handle, HANDLE(shut_down_handle as _)];
                    let wait_event = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };

                    let _ = unsafe { CloseHandle(watch_handle) };

                    match wait_event {
                        // registry changed
                        WAIT_OBJECT_0 => {
                            let original_system_theme = {
                                let system_theme = system_theme.read().unwrap();
                                *system_theme
                            };

                            let current_system_theme = SystemTheme::get();

                            if original_system_theme != current_system_theme {
                                log::info!("System Theme changed = {current_system_theme:?}");

                                let mut system_theme = system_theme.write().unwrap();
                                *system_theme = current_system_theme;

                                PROXY
                                    .get()
                                    .unwrap()
                                    .send_event(UserEvent::UpdateTray)
                                    .expect("Failed to send UpdateTray Event");
                            }
                        }
                        // exit
                        WAIT_EVENT(1) => {
                            log::info!("Watcher theme thread has stopped");
                            let _ = unsafe { RegCloseKey(hkey) };
                            break;
                        }
                        WAIT_FAILED => {
                            log::error!("WaitForMultipleObjects failed: {wait_event:?}");
                            break;
                        }
                        _ => {
                            log::error!("WaitForMultipleObjects unexpected result: {wait_event:?}");
                            break;
                        }
                    }
                }
            })
        };

        self.thread_handle = Some(thread_handle);
    }
}

impl Drop for ThemeWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
