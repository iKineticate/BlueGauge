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

pub fn find_bluetooth_le_devices() -> windows::core::Result<Vec<BluetoothLEDevice>> {
    let bt_le_aqs_filter = BluetoothLEDevice::GetDeviceSelector()?;
    let devices_info = DeviceInformation::FindAllAsyncAqsFilter(&bt_le_aqs_filter)?.get()?;

    Ok(devices_info
        .into_iter()
        .filter_map(|device_info| {
            BluetoothLEDevice::FromIdAsync(&device_info.Id().ok()?)
                .ok()?
                .get()
                .ok()
        })
        .collect())
}

pub fn get_bluetooth_le_info(
    bt_le_devices_info: Vec<BluetoothLEDevice>,
) -> windows::core::Result<Vec<BLEInfo>> {
    bt_le_devices_info
        .into_iter()
        .map(|bt_le_device_info| {
            let name = bt_le_device_info.Name()?.to_string();
            let battery = get_battery_level(&bt_le_device_info).unwrap_or(0);
            let status = bt_le_device_info
                .ConnectionStatus()
                .map(|status| matches!(status, BluetoothConnectionStatus::Connected))
                .unwrap_or(false);

            Ok(BLEInfo {
                name,
                battery,
                status,
            })
        })
        .collect()
}

pub fn get_battery_level(bt_le_device: &BluetoothLEDevice) -> windows::core::Result<u8> {
    let battery_services_uuid: GUID = GattServiceUuids::Battery()?;
    let battery_level_uuid: GUID = GattCharacteristicUuids::BatteryLevel()?;

    let battery_services = bt_le_device
        .GetGattServicesForUuidAsync(battery_services_uuid)
        .and_then(|op_gatt_services_result| op_gatt_services_result.get())
        .and_then(|gatt_services_result| gatt_services_result.Services())?;

    let battery_service = battery_services
        .into_iter()
        .next()
        .ok_or_else(|| Error::empty())?;

    let battery_gatt_chars = battery_service
        .GetCharacteristicsForUuidAsync(battery_level_uuid)
        .and_then(|op_gatt_chars_result| op_gatt_chars_result.get())
        .and_then(|gatt_chars_result| gatt_chars_result.Characteristics())?;

    let battery_gatt_char = battery_gatt_chars
        .into_iter()
        .next()
        .ok_or_else(|| Error::empty())?;

    let battery_level = match battery_gatt_char.Uuid()? == battery_level_uuid {
        true => battery_gatt_char
            .ReadValueAsync()
            .and_then(|op_gatt_read_result| op_gatt_read_result.get())
            .and_then(|gatt_read_result| gatt_read_result.Value())
            .and_then(|buffer| DataReader::FromBuffer(&buffer))
            .and_then(|date_reader| date_reader.ReadByte()),
        false => Err(Error::empty()),
    };

    return battery_level;
}
