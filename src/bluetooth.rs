use windows::{
    core::{Error, GUID},
    Devices::Bluetooth::GenericAttributeProfile::{GattCharacteristicUuids, GattServiceUuids},
    Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothLEDevice},
    Devices::Enumeration::DeviceInformation,
    Storage::Streams::DataReader,
};

pub struct BLEInfo {
    pub name: String,
    pub battery: u8,
    pub status: bool,
}

pub fn find_bluetooth_le_devices() -> windows::core::Result<Vec<DeviceInformation>> {
    let bt_le_aqs_filter = BluetoothLEDevice::GetDeviceSelector().unwrap();
    let bt_le_devices_info = DeviceInformation::FindAllAsyncAqsFilter(&bt_le_aqs_filter)?.get()?;
    Ok(bt_le_devices_info.into_iter().collect())
}

pub fn get_battery_level(bt_le_device: &BluetoothLEDevice) -> windows::core::Result<u8> {
    let battery_services_uuid: GUID = GattServiceUuids::Battery()?;
    let battery_level_uuid: GUID = GattCharacteristicUuids::BatteryLevel()?;

    let services = bt_le_device
        .GetGattServicesForUuidAsync(battery_services_uuid)
        .and_then(|gatt_services_result| gatt_services_result.get())
        .and_then(|gatt_services| gatt_services.Services())?;

    let service = services
        .into_iter()
        .next()
        .ok_or_else(|| Error::empty())?;

    let gatt_chars = service
        .GetCharacteristicsForUuidAsync(battery_level_uuid)
        .and_then(|op_gatt_chars_result| op_gatt_chars_result.get())
        .and_then(|gatt_chars_result| gatt_chars_result.Characteristics())?;

    let gatt_char = gatt_chars
        .into_iter()
        .next()
        .ok_or_else(|| Error::empty())?;

    let battery_level = if gatt_char.Uuid()? == battery_level_uuid {
        let result = gatt_char.ReadValueAsync()?.get()?;
        let reader = DataReader::FromBuffer(&result.Value()?);
        Ok(reader?.ReadByte()?)
    } else {
        Err(Error::empty())
    };

    return battery_level;
}

pub fn get_bluetooth_le_info(
    bt_le_devices_info: Vec<DeviceInformation>,
) -> windows::core::Result<Vec<BLEInfo>> {
    let mut info = Vec::<BLEInfo>::new();

    for bt_le_device_info in bt_le_devices_info {
        if let Ok(bt_le_device) = BluetoothLEDevice::FromIdAsync(&bt_le_device_info.Id()?)?.get() {
            let name = bt_le_device.Name()?.to_string();

            let battery = get_battery_level(&bt_le_device).unwrap_or(0);

            let status = match bt_le_device
                .ConnectionStatus()
                .expect("Failed to get link status")
            {
                BluetoothConnectionStatus::Connected => true,
                _ => false,
            };

            info.push(BLEInfo {
                name,
                battery,
                status,
            });
        };
    }

    Ok(info)
}
