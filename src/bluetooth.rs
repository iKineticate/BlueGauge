use windows::{
    core::{Error, GUID},
    Devices::Bluetooth::GenericAttributeProfile::{GattCharacteristicUuids, GattServiceUuids},
    Devices::Bluetooth::{BluetoothConnectionStatus,BluetoothLEDevice,BluetoothDevice},
    Devices::Enumeration::DeviceInformation,
    Storage::Streams::DataReader,
};

pub struct BluetoothInfo {
    pub name: String,
    pub battery: u8,
    pub status: bool,
}

pub fn find_bluetooth_devices() -> windows::core::Result<(Vec<BluetoothDevice>, Vec<BluetoothLEDevice>)> {
    let bt_aqs_filter = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let bt_le_aqs_filter = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;

    let bt_devices_info_collection = 
        DeviceInformation::FindAllAsyncAqsFilter(&bt_aqs_filter)?.get()?;
    let ble_devices_info_collection = 
        DeviceInformation::FindAllAsyncAqsFilter(&bt_le_aqs_filter)?.get()?;

    Ok((
        bt_devices_info_collection
            .into_iter()
            .filter_map(|device_info| {
                BluetoothDevice::FromIdAsync(&device_info.Id().ok()?)
                    .ok()?
                    .get()
                    .ok()
            })
            .collect(),

        ble_devices_info_collection
            .into_iter()
            .filter_map(|device_info| {
                BluetoothLEDevice::FromIdAsync(&device_info.Id().ok()?)
                    .ok()?
                    .get()
                    .ok()
            })
            .collect(),
    ))
}

pub fn get_bluetooth_info(
    bt_devices: Vec<BluetoothDevice>,
    ble_devices: Vec<BluetoothLEDevice>,
) -> windows::core::Result<Vec<BluetoothInfo>> {
    let mut devices_info: Vec<BluetoothInfo> = Vec::new();

    if bt_devices.len() > 0 {
        let pnp_bt_devices_info: Vec<(String, u8)> = get_pnp_bt_devices_info();

        for bt_device in bt_devices {
            let name = bt_device.Name()?.to_string();
            for (n, battery) in &pnp_bt_devices_info {
                // e.g. 
                // bluetooth name: HUAWEI FreeBuds Pro
                // pnp device name: HUAWEI FreeBuds Pro Hands-Free AG
                if n.contains(&name) {
                    if bt_device.ConnectionStatus()? == BluetoothConnectionStatus::Connected {
                        devices_info.push(BluetoothInfo {
                            name,
                            battery: *battery,
                            status: true,
                        });
                    } else {
                        devices_info.push(BluetoothInfo {
                            name,
                            battery: *battery,
                            status: false,
                        });
                    };
                    break;
                };
            }
        }
    };

    if ble_devices.len() > 0 {
        for ble_device in ble_devices {
            let name = ble_device.Name()?.to_string();
            let battery = get_ble_battery_level(&ble_device).unwrap_or(0);
            let status = ble_device
                .ConnectionStatus()
                .map(|status| matches!(status, BluetoothConnectionStatus::Connected))
                .unwrap_or(false);

            devices_info.push(BluetoothInfo {
                name,
                battery,
                status,
            });
        }
    };


    Ok(devices_info)
}

pub fn get_ble_battery_level(bt_le_device: &BluetoothLEDevice) -> windows::core::Result<u8> {
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


use scalefs_windowspnp::{PnpDeviceNodeInfo,PnpDevicePropertyValue,PnpEnumerator};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_SYSTEM;
use windows_sys::Win32::Devices::Properties::{DEVPKEY_Device_FriendlyName, DEVPROPKEY};

#[allow(non_upper_case_globals)]
const DEVPKEY_Bluetooth_Battery: DEVPROPKEY = DEVPROPKEY { fmtid: windows_sys::core::GUID::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5), pid:2 };
const BT_INSTANCE_ID: &str = "BTHENUM\\";

fn get_pnp_bt_devices_info() -> Vec<(String, u8)> {
    let mut pnp_bt_devices_info: Vec<(String, u8)> = Vec::new();
    let bt_devices = get_pnp_bt_devices(GUID_DEVCLASS_SYSTEM);

    let filter_bt_devices_properties = bt_devices.into_iter().filter_map(|i| {
        match i.device_instance_id.contains(BT_INSTANCE_ID) {
            true => i.device_instance_properties,
            false => None,
        }
    });

    for bt_device_properties in filter_bt_devices_properties {
        let (mut name, mut battery_level) = (None, None);
        for (key, value) in bt_device_properties {
            if key == DEVPKEY_Device_FriendlyName.into() {
                if let PnpDevicePropertyValue::String(v) = value {
                    name = Some(v);
                };
            } else if key == DEVPKEY_Bluetooth_Battery.into() {
                if let PnpDevicePropertyValue::Byte(v) = value {
                    battery_level = Some(v);
                };
            } else if name.is_some() && battery_level.is_some() {
                pnp_bt_devices_info.push((name.unwrap(), battery_level.unwrap()));
                break;
            };
        }
    }

    pnp_bt_devices_info
}

fn get_pnp_bt_devices(guid: windows_sys::core::GUID) -> Vec<PnpDeviceNodeInfo> {
    match PnpEnumerator::enumerate_present_devices_by_device_setup_class(guid) {
        Ok(devices) => devices,
        _ => panic!("scalefs windowspnp can't find pnp devices"),
    } 
}