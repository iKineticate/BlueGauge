use windows::{
    core::{Error, GUID},
    Devices::Bluetooth::GenericAttributeProfile::{GattCharacteristicUuids, GattServiceUuids},
    Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothLEDevice},
    Devices::Enumeration::DeviceInformation,
    Storage::Streams::DataReader,
};

pub struct BlueInfo {
    pub name: String,
    pub battery: u8,
    pub status: bool,
}

pub fn find_bluetooth_le_devices() -> windows::core::Result<Vec<DeviceInformation>> {
    let bt_le_aqs_filter = BluetoothLEDevice::GetDeviceSelector().unwrap();
    let devices_info = DeviceInformation::FindAllAsyncAqsFilter(&bt_le_aqs_filter)?.get()?;
    Ok(devices_info.into_iter().collect())
}

pub fn get_battery_level(device: &BluetoothLEDevice) -> windows::core::Result<u8> {
    let battery_guid: GUID = GattServiceUuids::Battery()?;
    let battery_level_guid: GUID = GattCharacteristicUuids::BatteryLevel()?;

    let services = device
        .GetGattServicesForUuidAsync(battery_guid)?
        .get()?
        .Services()?;

    for service in services {
        let battery_level = service
            .GetCharacteristicsForUuidAsync(battery_level_guid)
            .and_then(|op_gatt_chars_result| op_gatt_chars_result.get())
            .and_then(|gatt_chars_result| gatt_chars_result.Characteristics())
            .and_then(|gatt_chars| {
                for gatt_char in gatt_chars {
                    if gatt_char.Uuid()? == battery_level_guid {
                        let result = gatt_char.ReadValueAsync()?.get()?;
                        let reader = DataReader::FromBuffer(&result.Value()?);
                        return Ok(reader?.ReadByte()?);
                    }
                }
                Err(Error::from_win32())
            });
        if battery_level.is_ok() {
            return battery_level;
        };
    }
    Err(Error::from_win32())
}

pub fn get_bluetooth_le_info(
    devices_info: Vec<DeviceInformation>,
) -> windows::core::Result<Vec<BlueInfo>> {
    let mut info = Vec::<BlueInfo>::new();

    for device_info in devices_info {
        if let Ok(bt_le_device) = BluetoothLEDevice::FromIdAsync(&device_info.Id()?)?.get() {
            let name = bt_le_device.Name()?.to_string();

            let battery = get_battery_level(&bt_le_device).unwrap_or(0);

            let status = match bt_le_device
                .ConnectionStatus()
                .expect("Failed to get link status")
            {
                BluetoothConnectionStatus::Connected => true,
                _ => false,
            };

            info.push(BlueInfo {
                name,
                battery,
                status,
            });
        };
    }

    Ok(info)
}
