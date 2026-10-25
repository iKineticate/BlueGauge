use super::{MenuGroup, item::*, registry::MenuItemMeta};
use crate::{
    PROXY, UserEvent,
    config::{CONFIG, CONFIG_PATH, TrayIconStyle},
    startup::set_startup,
};

use std::{process::Command, str::FromStr};

use anyhow::{Context, Result, anyhow};
use tray_icon::menu::MenuItemKind;

// 处理菜单事件
pub fn handle_menu_event(return_menu_meta: &MenuItemMeta<MenuGroup>) -> Result<()> {
    let return_menu_group = return_menu_meta.group();
    let return_menu_kind = return_menu_meta.kind();
    let return_menu_id = return_menu_kind.id();
    let return_menu_action = match MenuAction::from_str(return_menu_id.as_ref()) {
        Ok(m) => m,
        Err(e) => {
            if matches!(return_menu_group, Some(MenuGroup::RadioDevice)) {
                MenuAction::DeviceMenu
            } else {
                return Err(anyhow!("No match menu [{}] - {e}", return_menu_id.0));
            }
        }
    };

    let mut config = CONFIG.write().unwrap();

    let proxy = PROXY.get().unwrap();

    match return_menu_group {
        // 无分组的独立菜单
        None => match return_menu_kind {
            MenuItemKind::MenuItem(_m) => match return_menu_action {
                MenuAction::Quit => proxy
                    .send_event(UserEvent::Exit)
                    .context("Failed to send 'Exit' event"),
                MenuAction::About => proxy
                    .send_event(UserEvent::ShowAboutDialog)
                    .context("Failed to send 'Show About Dialog' event"),
                MenuAction::Refresh => proxy
                    .send_event(UserEvent::Refresh)
                    .context("Failed to send 'Refresh' event"),
                MenuAction::Restart => proxy
                    .send_event(UserEvent::Restart)
                    .context("Failed to send 'Restart' event"),
                MenuAction::OpenConfig => Command::new("notepad.exe")
                    .arg(&*CONFIG_PATH)
                    .spawn()
                    .map(|_| ())
                    .context("Failed to open config file"),
                _ => Err(anyhow!("No match normal menu: {return_menu_action:?}")),
            },
            MenuItemKind::Check(m) => {
                let check_state = m.is_checked();

                match return_menu_action {
                    MenuAction::Startup => set_startup(check_state),
                    MenuAction::ShowLowestBatteryDevice => {
                        config
                            .tray_options
                            .set_show_lowest_battery_device(check_state);

                        config.save_toml();

                        drop(config);

                        proxy
                            .send_event(UserEvent::UpdateTray)
                            .context("Failed to send 'Update Tray' event")
                    }
                    MenuAction::SetIconConnectColor => {
                        config.tray_options.tray_icon_style.set_status(check_state);

                        config.save_toml();

                        drop(config);

                        proxy
                            .send_event(UserEvent::UpdateTrayIcon)
                            .context("Failed to send 'Update Tray Icon' event")
                    }
                    _ => Err(anyhow!(
                        "No match single check menu: {return_menu_action:?}"
                    )),
                }
            }
            _ => Err(anyhow!("Unsupported menu kind: {return_menu_action:?}")),
        },
        // 有分组的菜单（Radio、CheckBox）
        Some(group) => {
            let return_menu = return_menu_kind
                .as_check_menuitem()
                .ok_or_else(|| anyhow!("Menu is not a check menu: {return_menu_action:?}"))?;

            let return_menu_state = return_menu.is_checked();

            match group {
                // CheckBox
                MenuGroup::CheckBoxNotify => {
                    let notify_options = &mut config.notify_options;

                    match return_menu_action {
                        MenuAction::NotifyDeviceChangeDisconnection => {
                            notify_options.set_disconnection(return_menu_state);
                        }
                        MenuAction::NotifyDeviceChangeReconnection => {
                            notify_options.set_reconnection(return_menu_state);
                        }
                        MenuAction::NotifyDeviceChangeAdded => {
                            notify_options.set_added(return_menu_state);
                        }
                        MenuAction::NotifyDeviceChangeRemoved => {
                            notify_options.set_removed(return_menu_state);
                        }
                        MenuAction::NotifyDeviceStayOnScreen => {
                            notify_options.set_stay_on_screen(return_menu_state);
                        }
                        _ => {
                            return Err(anyhow!("No match set notify menu: {return_menu_action:?}"));
                        }
                    }

                    config.save_toml();

                    Ok(())
                }
                MenuGroup::CheckBoxTrayTooltip => {
                    let tooltip_options = &mut config.tray_options.tooltip_options;

                    match return_menu_action {
                        MenuAction::TrayTooltipShowDisconnected => {
                            tooltip_options.set_show_disconnected(return_menu_state);
                        }
                        MenuAction::TrayTooltipTruncateName => {
                            tooltip_options.set_truncate_name(return_menu_state);
                        }
                        MenuAction::TrayTooltipPrefixBattery => {
                            tooltip_options.set_prefix_battery(return_menu_state);
                        }
                        _ => {
                            return Err(anyhow!(
                                "No match set tray tooltip menu: {return_menu_action:?}"
                            ));
                        }
                    }

                    config.save_toml();
                    drop(config);

                    proxy
                        .send_event(UserEvent::UpdateTrayTooltip)
                        .context("Failed to send 'Update Tray' event")
                }
                // Radio
                MenuGroup::RadioDevice => {
                    if return_menu.is_checked() {
                        let device_menu_id = return_menu.id();
                        let device_address =
                            device_menu_id.as_ref().parse::<u64>().unwrap_or_else(|_| {
                                panic!("The menu isn't device menu: {}", device_menu_id.0)
                            });

                        let tray_icon_style = &config.tray_options.tray_icon_style;
                        if matches!(tray_icon_style, TrayIconStyle::App) {
                            config.tray_options.tray_icon_style =
                                TrayIconStyle::number_icon(device_address, None);
                        } else {
                            config
                                .tray_options
                                .tray_icon_style
                                .update_address(device_address);
                        }
                    } else {
                        // 全部设备未勾选，设置图标样式变回 AppIcon
                        config.tray_options.tray_icon_style = TrayIconStyle::App;
                        config.tray_options.set_show_lowest_battery_device(false);
                        let _ = proxy.send_event(UserEvent::UnCheckAboutIconMenu);
                    }

                    config.save_toml();
                    drop(config);

                    proxy
                        .send_event(UserEvent::UpdateTray)
                        .context("Failed to send 'Update Icon' event")
                }
                MenuGroup::RadioLowBattery => {
                    let low_battery = return_menu.id().as_ref().parse::<u8>()?;
                    let should_notify = low_battery.ne(&0);

                    config.notify_options.low_battery.set_notify(should_notify);

                    if should_notify {
                        config.notify_options.low_battery.set_value(low_battery);
                    };

                    config.save_toml();
                    drop(config);

                    proxy
                        .send_event(UserEvent::UpdateTrayIcon)
                        .context("Failed to send 'Update Tray' event")
                }
                MenuGroup::RadioTrayIconStyle => {
                    let tray_icon_style = &config.tray_options.tray_icon_style;

                    let Some(address) = tray_icon_style.get_address() else {
                        // 若为App图标，即为无勾选设备，则返回
                        return Ok(());
                    };

                    let color_scheme = tray_icon_style.get_theme();

                    match return_menu_action {
                        MenuAction::TrayIconStyleApp => {
                            // 取消勾选所有设备菜单，取消显示最低电量设备选项
                            config.tray_options.set_show_lowest_battery_device(false);
                            let _ = proxy.send_event(UserEvent::UnCheckDeviceMenu);
                            let _ = proxy.send_event(UserEvent::UnCheckAboutIconMenu);
                            config.tray_options.tray_icon_style = TrayIconStyle::App;
                        }
                        MenuAction::TrayIconStyleHorizontalBattery => {
                            config.tray_options.tray_icon_style =
                                TrayIconStyle::hor_battery_icon(address, color_scheme)
                        }
                        MenuAction::TrayIconStyleVerticalBattery => {
                            config.tray_options.tray_icon_style =
                                TrayIconStyle::vrt_battery_icon(address, color_scheme)
                        }
                        MenuAction::TrayIconStyleNumber => {
                            config.tray_options.tray_icon_style =
                                TrayIconStyle::number_icon(address, color_scheme)
                        }
                        MenuAction::TrayIconStyleRing => {
                            config.tray_options.tray_icon_style =
                                TrayIconStyle::ring_icon(address, color_scheme)
                        }
                        _ => {
                            return Err(anyhow!(
                                "No match set tray icon style menu: {return_menu_id:?}"
                            ));
                        }
                    }

                    config.save_toml();
                    drop(config);

                    proxy
                        .send_event(UserEvent::UpdateTrayIcon)
                        .context("Failed to send 'Update Tray' event")
                }
            }
        }
    }
}
