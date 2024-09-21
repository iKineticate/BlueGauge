// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use windows_sys::core::GUID;

#[derive(Clone)]
pub enum EnumerateSpecifier {
    AllDevices,
    DeviceInterfaceClassGuid(/*device_interface_class_guid: */GUID),
    DeviceSetupClassGuid(/*device_setup_class_guid: */GUID),
    PnpDeviceInstanceId(/*device_instance_id: */String, /*device_interface_class_guid: */Option<GUID>),
    PnpEnumeratorId(/*enumerator_id: */String),
}