use crate::{config::CONFIG, language::LOC};

use std::sync::Arc;

use dashmap::DashSet;
use tauri_winrt_notification::{Duration, Result, Scenario, Sound, Toast};

// HKEY_CLASSES_ROOT\AppUserModelId\Windows.SystemToast.BthQuickPair
const BLUETOOTH_APP_ID: &str = "Windows.SystemToast.BthQuickPair";

pub fn notify(text: impl AsRef<str>) {
    notify_default(text);
}

fn notify_default(text: impl AsRef<str>) {
    Toast::new(BLUETOOTH_APP_ID)
        .title("BlueGauge")
        .text1(text.as_ref())
        .sound(Some(Sound::Default))
        .duration(Duration::Short)
        .show()
        .expect("Failied to send notification");
}

fn notify_stay(text: impl AsRef<str>) {
    Toast::new(BLUETOOTH_APP_ID)
        .title("BlueGauge")
        .text1(text.as_ref())
        .sound(Some(Sound::Default))
        .scenario(Scenario::Reminder)
        .add_button("OK", "OK")
        .on_activated(|_| Result::Ok(()))
        .show()
        .expect("Failied to send notification");
}

#[derive(Debug)]
pub enum NotifyEvent {
    LowBattery(String, u8, u64),
    Added(String),
    Removed(String),
    Reconnect(String),
    Disconnect(String),
}

impl NotifyEvent {
    pub fn send(&self, notifyed_devices: Arc<DashSet<u64>>) {
        let config = CONFIG.read().unwrap();
        let notify = if config.notify_options.stay_on_screen() {
            notify_stay
        } else {
            notify_default
        };
        match self {
            NotifyEvent::LowBattery(name, battery, address) => {
                let low_threshold = config.notify_options.low_battery.value() as i32;

                if !config.notify_options.low_battery.notify {
                    return;
                }

                let current_battery = *battery as i32;
                let diff = current_battery - low_threshold;

                if diff <= 0 {
                    if notifyed_devices.insert(*address) {
                        let message = format!("{name}: {} {battery}", LOC.bluetooth_battery_below);
                        notify(message);
                    }
                } else if diff > 10 {
                    notifyed_devices.remove(address);
                }
                // else {
                //   // 电量在 (low_threshold, low_threshold + 10] 范围内：
                //   // 处于“防抖缓冲区”，不通知也不清除，避免反复触发
                // }
            }
            NotifyEvent::Added(name) if config.notify_options.added() => {
                notify(format!("{name}: {}", LOC.new_bluetooth_device_add));
            }
            NotifyEvent::Removed(name) if config.notify_options.removed() => {
                notify(format!("{name}: {}", LOC.old_bluetooth_device_removed));
            }
            NotifyEvent::Reconnect(name) if config.notify_options.reconnection() => {
                notify(format!("{name}: {}", LOC.bluetooth_device_reconnected));
            }
            NotifyEvent::Disconnect(name) if config.notify_options.disconnection() => {
                notify(format!("{name}: {}", LOC.bluetooth_device_disconnected));
            }
            _ => (),
        }
    }
}
