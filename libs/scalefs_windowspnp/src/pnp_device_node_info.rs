// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use crate::{
    PnpDevicePropertyKey,
    PnpDevicePropertyValue,
};
use scalefs_uuid::Uuid;
use std::collections::HashMap;

pub struct PnpDeviceNodeInfo {
    // device instance id (applies to all devices)
    pub device_instance_id: String,
    // NOTE: the BaseContainerId should be available for virutally all devices, but not for bus drivers or special edge cases (e.g. a volume devnode that spans multiple containers); see: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/overview-of-container-ids
    pub base_container_id: Option<Uuid>,
    //
    // device instance properties (optional; these should be available for all devices)
    pub device_instance_properties: Option<HashMap<PnpDevicePropertyKey, PnpDevicePropertyValue>>,
    //
    // device setup class properties (optional) (also, they are available for most devices but not ALL devices)
    pub device_setup_class_properties: Option<HashMap<PnpDevicePropertyKey, PnpDevicePropertyValue>>,
    //
    // device path (only applies to device interfaces; will be None otherwise)
    pub device_path: Option<String>,
    // interface properties (optional) (also, they only apply to device interfaces; will be None otherwise)
    pub device_interface_properties: Option<HashMap<PnpDevicePropertyKey, PnpDevicePropertyValue>>,
    // interface class properties (optional) (also, they only apply to device interfaces; will be None otherwise)
    pub device_interface_class_properties: Option<HashMap<PnpDevicePropertyKey, PnpDevicePropertyValue>>,
}
