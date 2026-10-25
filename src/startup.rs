use super::config::{EXE_NAME, EXE_PATH_STRING};

use anyhow::{Context, Result, anyhow};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ},
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn set_startup(enabled: bool) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _disp) = hkcu.create_subkey(RUN_KEY)?;

    if enabled {
        run_key
            .set_value(&*EXE_NAME, &*EXE_PATH_STRING)
            .with_context(|| "Failed to set the autostart registry key")?;
    } else {
        run_key
            .delete_value(&*EXE_NAME)
            .with_context(|| "Failed to delete the autostart registry key")?;
    }

    Ok(())
}

pub fn get_startup_status() -> Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .map_err(|e| anyhow!("Failed to open HKEY_CURRENT_USER\\...\\Run - {e}"))?;

    match run_key.get_value::<String, _>(&*EXE_NAME) {
        Ok(value) => Ok(value.eq_ignore_ascii_case(&*EXE_PATH_STRING)),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(anyhow!("Failed to get the autostart registry key - {e}")),
    }
}
