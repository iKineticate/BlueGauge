#![allow(non_snake_case)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod systray;
use crate::systray::show_systray;

use windows::{
    Devices::Bluetooth::{BluetoothLEDevice, BluetoothConnectionStatus},
    Devices::Bluetooth::GenericAttributeProfile::GattCharacteristicUuids,
    Devices::Enumeration::DeviceInformation,
    Storage::Streams::DataReader,
    core::GUID,
};
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

fn main() -> windows::core::Result<()> {
    let _ = show_systray();

    Ok(())
}

fn find_bluetooth_devices() -> windows::core::Result<Vec<DeviceInformation>> {
    let devices = 
        windows::Devices::Enumeration::DeviceInformation::FindAllAsync()?.get()?;

    let mut discovered_devices = Vec::new();

    Ok(devices
        .into_iter()
        .filter_map(|device| {
            device.Name().ok().and_then(|n| {
                let name = n.to_string();
                if discovered_devices.contains(&name) {
                    None
                } else {
                    discovered_devices.push(name);
                    Some(device)
                }
            })
        })
        .collect()
    )
}

fn get_battery_level(device: &BluetoothLEDevice) -> windows::core::Result<u8> {
    let services = device.GetGattServicesAsync()?.get()?.Services()?;

    let battery_level_guid: GUID = GattCharacteristicUuids::BatteryLevel()?;

    for service in services {
        let characteristics = service.GetCharacteristicsAsync()?.get()?.Characteristics()?;

        for characteristic in characteristics {
            if characteristic.Uuid()? == battery_level_guid {
                let result = characteristic.ReadValueAsync()?.get()?;
                let reader = DataReader::FromBuffer(&result.Value()?);
                return Ok(reader?.ReadByte()?);
            }
        }
    }

    Err(windows::core::Error::from_win32())
}

fn get_bluetooth_info(devices: Vec<DeviceInformation>) -> windows::core::Result<(Vec<String>, Menu)> {
    let menu = Menu::new();
    let mut tooltip: Vec<String> = Vec::new();

    for device in devices {
        if let Ok(le_device) = BluetoothLEDevice::FromIdAsync(&device.Id()?)?.get() {
            let status = le_device.ConnectionStatus().expect("Failed to get link status");

            let battery_level = match get_battery_level(&le_device) {
                Ok(level) => level.to_string(),
                Err(_) => "None".to_string(),
            };

            if status == BluetoothConnectionStatus::Connected {
                let menu_text = format!("✅ {} - {}%", device.Name().unwrap(), battery_level);
                let tooltip_text = format!("🟢 {} - {}%", device.Name().unwrap(), battery_level);
                menu.prepend(&MenuItem::new(menu_text, true, None)).unwrap();
                tooltip.insert(0, tooltip_text);
            } else {
                let menu_text = format!("❎ {} - {}%", device.Name().unwrap(), battery_level);
                let tooltip_text = format!("🔴 {} - {}%", device.Name().unwrap(), battery_level);
                menu.append(&MenuItem::new(menu_text, true, None)).unwrap();
                tooltip.push(tooltip_text);
            };
        }
    };

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    Ok((tooltip, menu))
}