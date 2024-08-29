use windows::{
    Devices::Bluetooth::{BluetoothLEDevice, BluetoothConnectionStatus},
    Devices::Bluetooth::GenericAttributeProfile::GattCharacteristicUuids,
    Devices::Enumeration::DeviceInformation,
    Storage::Streams::DataReader,
    core::GUID,
};

pub fn find_bluetooth_devices() -> windows::core::Result<Vec<DeviceInformation>> {
    let bt_le_aqs_filter = BluetoothLEDevice::GetDeviceSelector().unwrap();

    let devices = DeviceInformation::FindAllAsyncAqsFilter(&bt_le_aqs_filter)?.get()?;

    Ok(devices.into_iter().collect())
}

pub fn get_battery_level(device: &BluetoothLEDevice) -> windows::core::Result<u8> {
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

pub fn get_bluetooth_info(devices: Vec<DeviceInformation>) -> windows::core::Result<(Vec<String>, Vec<String>)> {
    let mut tooltip: Vec<String> = Vec::new();
    let mut menu_items: Vec<String> = Vec::new();

    for device in devices {
        if let Ok(le_device) = BluetoothLEDevice::FromIdAsync(&device.Id()?)?.get() {
            let device_name = device.Name()?.to_string();

            let status = le_device.ConnectionStatus().expect("Failed to get link status");

            let battery_level = match get_battery_level(&le_device) {
                Ok(level) => level.to_string(),
                Err(_) => "None".to_string(),
            };

            if status == BluetoothConnectionStatus::Connected {
                let menu_text = format!("🔗 {} - {}%", &device_name, battery_level);
                let tooltip_text = format!("🟢 {} - {}%", &device_name, battery_level);
                menu_items.insert(0, menu_text);
                tooltip.insert(0, tooltip_text);
                // println!("{:?}", device.Properties()?)
            } else {
                let menu_text = format!("     {} - {}%", &device_name, battery_level);
                let tooltip_text = format!("🔴 {} - {}%", &device_name, battery_level);
                menu_items.push(menu_text);
                tooltip.push(tooltip_text);
            };
        }
    };

    Ok((tooltip, menu_items))
}