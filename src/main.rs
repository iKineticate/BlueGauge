#![allow(non_snake_case)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bluetooth;
mod systray;
use crate::systray::show_systray;

fn main() {
    if let Err(_err) = show_systray() {
        () // windows_toast_notify
    }
}
