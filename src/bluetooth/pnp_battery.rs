use std::collections::HashMap;

use anyhow::{Result, anyhow};
use windows::{
    Win32::{
        Devices::{
            DeviceAndDriverInstallation::{
                CM_Get_DevNode_PropertyW, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CR_SUCCESS,
                DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA,
                SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
                SetupDiGetDeviceInstanceIdW, SetupDiGetDevicePropertyW,
            },
            Properties::{DEVPROP_TYPE_BYTE, DEVPROP_TYPE_STRING, DEVPROPTYPE},
        },
        Foundation::{
            DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError, HWND,
        },
    },
    core::{GUID, PCWSTR},
};

/// GUID_DEVCLASS_SYSTEM = {4D36E967-E325-11CE-BFC1-08002BE10318}
const GUID_DEVCLASS_SYSTEM: GUID = GUID::from_u128(0x4D36E967_E325_11CE_BFC1_08002BE10318);

/// DEVPKEY_Bluetooth_Battery = {104EA319-6EE2-4701-BD47-8DDBF425BBE5}, pid 2, Byte
const DEVPKEY_BLUETOOTH_BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5),
    pid: 2,
};

/// DEVPKEY_Bluetooth_DeviceAddress = {2BD67D8B-8BEB-48D5-87E0-6CDA3428040A}, pid 1, String
const DEVPKEY_BLUETOOTH_DEVICE_ADDRESS: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x2BD67D8B_8BEB_48D5_87E0_6CDA3428040A),
    pid: 1,
};

/// 蓝牙设备实例 ID 前缀
pub const BT_INSTANCE_ID_PREFIX: &str = "BTHENUM\\";

#[derive(Debug, Clone)]
pub struct PnpDeviceInfo {
    pub battery: u8,
    pub instance_id: String,
}

pub fn enumerate_bt_batteries() -> Result<HashMap<u64, PnpDeviceInfo>> {
    let hdev = open_device_info_set(GUID_DEVCLASS_SYSTEM)?;
    let _guard = DevInfoGuard(hdev);

    let mut result = HashMap::new();

    for index in 0..u32::MAX {
        let Some(devinfo) = next_devinfo(hdev, index)? else {
            break;
        };

        let instance_id = match get_instance_id(hdev, &devinfo) {
            Ok(id) => id,
            Err(e) => {
                log::warn!("Skipping device (instance id read failed): {e}");
                continue;
            }
        };

        if !instance_id.contains(BT_INSTANCE_ID_PREFIX) {
            continue;
        }

        let Some(battery) = read_byte_property(hdev, &devinfo, &DEVPKEY_BLUETOOTH_BATTERY) else {
            continue;
        };

        let Some(address_str) =
            read_string_property(hdev, &devinfo, &DEVPKEY_BLUETOOTH_DEVICE_ADDRESS)
        else {
            continue;
        };

        let Ok(address) = u64::from_str_radix(&address_str, 16) else {
            continue;
        };

        result.insert(
            address,
            PnpDeviceInfo {
                battery,
                instance_id,
            },
        );
    }

    Ok(result)
}

/// 没法用 `CM_Register_Notification`，因为 `CM_NOTIFY_ACTION`不支持Pnp设备的属性变化(可能仅支持连接和断开)
/// https://learn.microsoft.com/zh-cn/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_register_notification
/// https://learn.microsoft.com/zh-cn/windows/win32/api/cfgmgr32/ne-cfgmgr32-cm_notify_action
pub fn read_bt_battery_by_instance_id(instance_id: &str) -> Option<u8> {
    let mut utf16: Vec<u16> = instance_id.encode_utf16().collect();
    utf16.push(0);

    unsafe {
        let mut devnode: u32 = 0;
        // https://learn.microsoft.com/zh-cn/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_locate_devnodew
        let cr = CM_Locate_DevNodeW(
            &mut devnode,
            PCWSTR(utf16.as_ptr()),
            CM_LOCATE_DEVNODE_NORMAL,
        );
        if cr != CR_SUCCESS {
            log::error!("CM_Locate_DevNodeW failed [{instance_id}] - CR={cr:?}");
            return None;
        }

        let mut battery: u8 = 0;
        let mut size: u32 = std::mem::size_of::<u8>() as u32;
        let mut prop_type: DEVPROPTYPE = DEVPROP_TYPE_BYTE;

        // https://learn.microsoft.com/zh-cn/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_get_devnode_propertyw
        let cr = CM_Get_DevNode_PropertyW(
            devnode,
            &DEVPKEY_BLUETOOTH_BATTERY,
            &mut prop_type,
            Some(&mut battery as *mut u8),
            &mut size,
            0,
        );
        if cr != CR_SUCCESS {
            log::error!("CM_Get_DevNode_PropertyW failed [{instance_id}] - CR={cr:?}");
            return None;
        }
        if prop_type != DEVPROP_TYPE_BYTE || size != 1 {
            return None;
        }

        Some(battery)
    }
}

struct DevInfoGuard(HDEVINFO);
impl Drop for DevInfoGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

fn open_device_info_set(class: GUID) -> Result<HDEVINFO> {
    let hdev = unsafe {
        SetupDiGetClassDevsW(
            Some(&class),
            PCWSTR::null(),
            Some(HWND::default()),
            DIGCF_PRESENT | DIGCF_ALLCLASSES,
        )
    }?;

    if hdev.0 == -1 {
        return Err(anyhow!(
            "SetupDiGetClassDevsW failed: {}",
            unsafe { GetLastError() }.0
        ));
    }
    Ok(hdev)
}

fn next_devinfo(hdev: HDEVINFO, index: u32) -> Result<Option<SP_DEVINFO_DATA>> {
    let mut devinfo: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
    devinfo.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;

    if unsafe { SetupDiEnumDeviceInfo(hdev, index, &mut devinfo) }.is_ok() {
        return Ok(Some(devinfo));
    }

    let err = unsafe { GetLastError() };
    if err == ERROR_NO_MORE_ITEMS {
        Ok(None)
    } else {
        Err(anyhow!("SetupDiEnumDeviceInfo failed: {}", err.0))
    }
}

fn read_property(
    hdev: HDEVINFO,
    devinfo: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<(DEVPROPTYPE, Vec<u8>)> {
    let mut prop_type: DEVPROPTYPE = DEVPROPTYPE(0);
    let mut required: u32 = 0;

    let _ = unsafe {
        SetupDiGetDevicePropertyW(
            hdev,
            devinfo,
            key,
            &mut prop_type,
            None,
            Some(&mut required),
            0,
        )
    };
    if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }

    let mut buf = vec![0u8; required as usize];
    let ok = unsafe {
        SetupDiGetDevicePropertyW(
            hdev,
            devinfo,
            key,
            &mut prop_type,
            Some(buf.as_mut_slice()),
            Some(&mut required),
            0,
        )
    };

    if !ok.is_ok() {
        return None;
    }

    Some((prop_type, buf))
}

fn read_byte_property(hdev: HDEVINFO, devinfo: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<u8> {
    let (t, buf) = read_property(hdev, devinfo, key)?;
    if t != DEVPROP_TYPE_BYTE || buf.is_empty() {
        return None;
    }
    Some(buf[0])
}

fn read_string_property(
    hdev: HDEVINFO,
    devinfo: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<String> {
    let (t, buf) = read_property(hdev, devinfo, key)?;
    if t != DEVPROP_TYPE_STRING || buf.len() % 2 != 0 {
        return None;
    }
    let mut u16s: Vec<u16> = buf
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_ne_bytes([c[0], c[1]]))
        .collect();
    while u16s.last() == Some(&0) {
        u16s.pop();
    }
    String::from_utf16(&u16s).ok()
}

fn get_instance_id(hdev: HDEVINFO, devinfo: &SP_DEVINFO_DATA) -> Result<String> {
    let mut required: u32 = 0;
    let _ = unsafe { SetupDiGetDeviceInstanceIdW(hdev, devinfo, None, Some(&mut required)) };
    if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(anyhow!(
            "SetupDiGetDeviceInstanceIdW: {}",
            unsafe { GetLastError() }.0
        ));
    }

    let mut buf = vec![0u16; required as usize];
    let ok = unsafe {
        SetupDiGetDeviceInstanceIdW(hdev, devinfo, Some(buf.as_mut_slice()), Some(&mut required))
    };
    if !ok.is_ok() {
        return Err(anyhow!("SetupDiGetDeviceInstanceIdW failed"));
    }
    while buf.last() == Some(&0) {
        buf.pop();
    }
    Ok(String::from_utf16_lossy(&buf))
}
