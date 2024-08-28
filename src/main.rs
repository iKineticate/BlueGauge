#![allow(non_snake_case)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod systray;
mod bluetooth;
use crate::systray::show_systray;

fn main() -> windows::core::Result<()> {
    show_systray().unwrap();

    Ok(())
}