#![allow(non_snake_case)]

use windows::{
    Devices::Bluetooth::{BluetoothLEDevice, BluetoothConnectionStatus},
    Devices::Bluetooth::GenericAttributeProfile::GattCharacteristicUuids,
    Devices::Enumeration::DeviceInformation,
    Storage::Streams::DataReader,
    core::GUID,
};

fn main() -> windows::core::Result<()> {
    let devices = find_bluetooth_devices()?;

    for device in devices {
        if let Ok(le_device) = BluetoothLEDevice::FromIdAsync(&device.Id()?)?.get() {
            if le_device.ConnectionStatus()? == BluetoothConnectionStatus::Connected {
                println!("Found device: {}", device.Name()?);

                if let Ok(battery_level) = get_battery_level(&le_device) {
                    println!("Battery level: {}%", battery_level);
                } else {
                    println!("Battery level: Not available");
                }
            }
        }
    }

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