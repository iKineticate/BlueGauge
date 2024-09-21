// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use windows::Win32::Foundation::GetLastError;

pub fn get_last_error_as_win32_error() -> windows::Win32::Foundation::WIN32_ERROR {
    let get_last_error_result = unsafe { GetLastError().ok() };
    
    match get_last_error_result {
        Ok(()) => windows::Win32::Foundation::WIN32_ERROR(0),
        Err(last_error) => windows::Win32::Foundation::WIN32_ERROR::from_error(&last_error).unwrap(),
    }
}