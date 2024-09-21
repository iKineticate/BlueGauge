// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

#[derive(Clone)]
pub enum EnumerateOption {
    IncludeInstanceProperties,
    IncludeDeviceInterfaceClassProperties,
    IncludeDeviceInterfaceProperties,
    IncludeSetupClassProperties,
}
