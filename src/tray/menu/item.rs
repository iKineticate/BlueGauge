use super::{MenuGroup, registry::MenuRegistry};
use crate::bluetooth::info::BT_INFO_MAP;
use crate::config::{CONFIG, Direction, TrayIconStyle};
use crate::language::LOC;
use crate::startup::get_startup_status;

use anyhow::{Context, Result};
use strum::{AsRefStr, EnumString};
use tray_icon::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};

#[derive(Debug, Clone, Eq, Hash, PartialEq, AsRefStr, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum MenuAction {
    // Normal
    Quit,
    About,
    Restart,
    Refresh,
    OpenConfig,
    DeviceMenu,
    // Normal - CheckSingle
    Startup,
    // Normal - CheckSingle
    ShowLowestBatteryDevice,
    // Normal - CheckSingle
    SetIconConnectColor,
    // Radio
    TrayIconStyleApp,
    TrayIconStyleHorizontalBattery,
    TrayIconStyleVerticalBattery,
    TrayIconStyleNumber,
    TrayIconStyleRing,
    // CheckBox
    TrayTooltipShowDisconnected,
    TrayTooltipTruncateName,
    TrayTooltipPrefixBattery,
    // Radio
    #[strum(serialize = "0")]
    LowBattery0,
    #[strum(serialize = "5")]
    LowBattery5,
    #[strum(serialize = "10")]
    LowBattery10,
    #[strum(serialize = "15")]
    LowBattery15,
    #[strum(serialize = "20")]
    LowBattery20,
    #[strum(serialize = "25")]
    LowBattery25,
    #[strum(serialize = "30")]
    LowBattery30,
    // CheckBox
    NotifyDeviceChangeDisconnection,
    NotifyDeviceChangeReconnection,
    NotifyDeviceChangeAdded,
    NotifyDeviceChangeRemoved,
    NotifyDeviceStayOnScreen,
}

impl MenuAction {
    pub fn id(&self) -> MenuId {
        MenuId::new(self.as_ref())
    }
}

struct CreateMenuItem(MenuRegistry<MenuGroup>);

impl CreateMenuItem {
    fn new() -> Self {
        Self(MenuRegistry::<MenuGroup>::new())
    }

    fn separator() -> PredefinedMenuItem {
        PredefinedMenuItem::separator()
    }

    fn quit(&mut self, text: &str) -> MenuItem {
        let menu_item = MenuItem::with_id(MenuAction::Quit.id(), text, true, None);
        self.0.register(menu_item.kind(), None);
        menu_item
    }

    fn about(&mut self, text: &str) -> MenuItem {
        let menu_item = MenuItem::with_id(MenuAction::About.id(), text, true, None);
        self.0.register(menu_item.kind(), None);
        menu_item
    }

    fn restart(&mut self, text: &str) -> MenuItem {
        let menu_item = MenuItem::with_id(MenuAction::Restart.id(), text, true, None);
        self.0.register(menu_item.kind(), None);
        menu_item
    }

    fn open_config(&mut self, text: &str) -> MenuItem {
        let menu_item = MenuItem::with_id(MenuAction::OpenConfig.id(), text, true, None);
        self.0.register(menu_item.kind(), None);
        menu_item
    }

    fn startup(&mut self, text: &str) -> Result<CheckMenuItem> {
        let should_startup = get_startup_status()?;
        let check_menu_item =
            CheckMenuItem::with_id(MenuAction::Startup.id(), text, true, should_startup, None);
        self.0.register(check_menu_item.kind(), None);
        Ok(check_menu_item)
    }

    fn refresh(&mut self, text: &str) -> MenuItem {
        let menu_item = MenuItem::with_id(MenuAction::Refresh.id(), text, true, None);
        self.0.register(menu_item.kind(), None);
        menu_item
    }

    fn bluetooth_devices(&mut self) -> Vec<CheckMenuItem> {
        let mut sorted_devices_info = BT_INFO_MAP
            .iter()
            .map(|entry| entry.value().clone())
            .collect::<Vec<_>>();

        sorted_devices_info.sort_by(|a, b| {
            // 1. 先按状态排序（🟢在前，🔴在后）
            match (a.status, b.status) {
                (true, false) => std::cmp::Ordering::Less, // true 在 false 前
                (false, true) => std::cmp::Ordering::Greater, // false 在 true 后
                _ => {
                    // 2. 同组内按名称字母顺序排序（A-Z）
                    a.name.cmp(&b.name)
                }
            }
        });

        let config = CONFIG.read().unwrap();

        let show_tray_battery_icon_bt_address = config.get_tray_icon_bt_address();

        sorted_devices_info
            .iter()
            .map(|info| {
                let menu_id = MenuId::from(info.address);
                let name = config.device_aliases.get(&info.name).unwrap_or(&info.name);
                let text = format!(
                    "{} - {name} - {}%",
                    if info.status { '♾' } else { '🚫' },
                    info.battery
                );
                let menu = CheckMenuItem::with_id(
                    menu_id,
                    text,
                    true,
                    show_tray_battery_icon_bt_address.is_some_and(|addr| addr.eq(&info.address)),
                    None,
                );
                self.0
                    .register_radio(menu.kind(), MenuGroup::RadioDevice, None);
                menu
            })
            .collect::<Vec<CheckMenuItem>>()
    }

    fn tray_icon_style(&mut self) -> Submenu {
        let tray_icon_style = &CONFIG.read().unwrap().tray_options.tray_icon_style;

        let select_horizontal_battery_icon = matches!(
            tray_icon_style,
            TrayIconStyle::BatteryIcon {
                direction: Direction::Horizontal,
                ..
            }
        );
        let select_vertical_battery_icon = matches!(
            tray_icon_style,
            TrayIconStyle::BatteryIcon {
                direction: Direction::Vertical,
                ..
            }
        );
        let select_number_icon = matches!(tray_icon_style, TrayIconStyle::BatteryNumber { .. });
        let select_ring_icon = matches!(tray_icon_style, TrayIconStyle::BatteryRing { .. });
        let select_app_icon = matches!(tray_icon_style, TrayIconStyle::App);

        let mut menus = Vec::new();

        [
            (
                MenuAction::TrayIconStyleHorizontalBattery.id(),
                LOC.horizontal_battery_icon,
                select_horizontal_battery_icon,
            ),
            (
                MenuAction::TrayIconStyleVerticalBattery.id(),
                LOC.vertical_battery_icon,
                select_vertical_battery_icon,
            ),
            (
                MenuAction::TrayIconStyleNumber.id(),
                LOC.number_icon,
                select_number_icon,
            ),
            (
                MenuAction::TrayIconStyleRing.id(),
                LOC.ring_icon,
                select_ring_icon,
            ),
            (
                MenuAction::TrayIconStyleApp.id(),
                LOC.app_icon,
                select_app_icon,
            ),
        ]
        .into_iter()
        .for_each(|(menu_id, text, checked)| {
            let menu = CheckMenuItem::with_id(menu_id, text, true, checked, None);
            self.0.register_radio(
                menu.kind(),
                MenuGroup::RadioTrayIconStyle,
                Some(MenuAction::TrayIconStyleNumber.id()),
            );
            menus.push(menu);
        });

        let menu_tray_icon_style: Vec<&dyn IsMenuItem> =
            menus.iter().map(|item| item as &dyn IsMenuItem).collect();

        Submenu::with_items(LOC.icon_style_options, true, &menu_tray_icon_style)
            .expect("Failed to create submenu for tray icon style")
    }

    fn tray_tooltip_options(&mut self) -> Submenu {
        let mut menus = Vec::new();
        let config = CONFIG.read().unwrap();

        [
            (
                MenuAction::TrayTooltipShowDisconnected.id(),
                LOC.show_disconnected,
                config.tray_options.tooltip_options.show_disconnected(),
            ),
            (
                MenuAction::TrayTooltipTruncateName.id(),
                LOC.truncate_name,
                config.tray_options.tooltip_options.truncate_name(),
            ),
            (
                MenuAction::TrayTooltipPrefixBattery.id(),
                LOC.prefix_battery,
                config.tray_options.tooltip_options.prefix_battery(),
            ),
        ]
        .into_iter()
        .for_each(|(menu_id, text, checked)| {
            let menu = CheckMenuItem::with_id(menu_id, text, true, checked, None);
            self.0
                .register(menu.kind(), Some(MenuGroup::CheckBoxTrayTooltip));
            menus.push(menu);
        });

        let menu_tray_tooltip_options: Vec<&dyn IsMenuItem> =
            menus.iter().map(|item| item as &dyn IsMenuItem).collect();

        Submenu::with_items(LOC.tray_tooltip_options, true, &menu_tray_tooltip_options)
            .expect("Failed to create submenu for tray tooltip options")
    }

    fn notify_low_battery(&mut self, low_battery: u8) -> [CheckMenuItem; 7] {
        [
            MenuAction::LowBattery0.id(),
            MenuAction::LowBattery5.id(),
            MenuAction::LowBattery10.id(),
            MenuAction::LowBattery15.id(),
            MenuAction::LowBattery20.id(),
            MenuAction::LowBattery25.id(),
            MenuAction::LowBattery30.id(),
        ]
        .map(|menu_id| {
            let dafault_menu_id = MenuId::from(low_battery);
            let battery = menu_id.as_ref().parse::<u8>().unwrap();
            let menu = CheckMenuItem::with_id(
                menu_id,
                if battery.eq(&0) {
                    LOC.never.to_string()
                } else {
                    format!("{battery}%")
                },
                true,
                low_battery == battery,
                None,
            );

            self.0.register_radio(
                menu.kind(),
                MenuGroup::RadioLowBattery,
                Some(dafault_menu_id),
            );

            menu
        })
    }

    fn notify_device_change(&mut self) -> Vec<CheckMenuItem> {
        let mut menus = Vec::new();
        let config = CONFIG.read().unwrap();

        [
            (
                MenuAction::NotifyDeviceChangeDisconnection.id(),
                LOC.disconnection,
                config.notify_options.disconnection(),
            ),
            (
                MenuAction::NotifyDeviceChangeReconnection.id(),
                LOC.reconnection,
                config.notify_options.reconnection(),
            ),
            (
                MenuAction::NotifyDeviceChangeAdded.id(),
                LOC.added,
                config.notify_options.added(),
            ),
            (
                MenuAction::NotifyDeviceChangeRemoved.id(),
                LOC.removed,
                config.notify_options.removed(),
            ),
            (
                MenuAction::NotifyDeviceStayOnScreen.id(),
                LOC.stay_on_screen,
                config.notify_options.stay_on_screen(),
            ),
        ]
        .into_iter()
        .for_each(|(menu_id, text, checked)| {
            let menu = CheckMenuItem::with_id(menu_id, text, true, checked, None);
            self.0
                .register(menu.kind(), Some(MenuGroup::CheckBoxNotify));
            menus.push(menu);
        });

        menus
    }

    fn set_icon_connect_color(&mut self) -> CheckMenuItem {
        let menu_id = MenuAction::SetIconConnectColor.id();
        // 仅 [数字图标]  [圆环图标] [电池图标] 支持连接配色
        let (enabled, checked) = match &CONFIG.read().unwrap().tray_options.tray_icon_style {
            TrayIconStyle::BatteryNumber { theme, .. }
            | TrayIconStyle::BatteryRing { theme, .. }
            | TrayIconStyle::BatteryIcon { theme, .. } => (true, theme.is_status()),
            _ => (false, false),
        };

        let menu =
            CheckMenuItem::with_id(menu_id, LOC.set_icon_connect_color, enabled, checked, None);

        self.0.register(menu.kind(), None);

        menu
    }

    fn show_lowest_battery_device(&mut self) -> CheckMenuItem {
        let menu = CheckMenuItem::with_id(
            MenuAction::ShowLowestBatteryDevice.id(),
            LOC.show_lowest_battery_device,
            true,
            CONFIG
                .read()
                .unwrap()
                .tray_options
                .show_lowest_battery_device(),
            None,
        );

        self.0.register(menu.kind(), None);

        menu
    }
}

pub fn create_menu(menu_manager: &mut MenuRegistry<MenuGroup>) -> Result<Menu> {
    let menu_separator = CreateMenuItem::separator();

    let mut create_menu_item = CreateMenuItem::new();

    let menu_about = create_menu_item.about(LOC.about);

    let menu_quit = create_menu_item.quit(LOC.quit);

    let menu_refresh = create_menu_item.refresh(LOC.refresh);

    let menu_restart = create_menu_item.restart(LOC.restart);

    let menu_startup = create_menu_item.startup(LOC.startup)?;

    let menu_open_config = create_menu_item.open_config(LOC.open_config);

    let menu_devices = create_menu_item.bluetooth_devices();
    let menu_devices: Vec<&dyn IsMenuItem> = menu_devices
        .iter()
        .map(|item| item as &dyn IsMenuItem)
        .collect();

    let menu_tray_options = {
        let menu_show_lowest_battery_device = create_menu_item.show_lowest_battery_device();
        let menu_set_icon_connect_color = create_menu_item.set_icon_connect_color();
        let menu_tray_icon_style = create_menu_item.tray_icon_style();
        let menu_tray_tooltip_options = create_menu_item.tray_tooltip_options();

        let menu_tray_options: Vec<&dyn IsMenuItem> = vec![
            &menu_show_lowest_battery_device as &dyn IsMenuItem,
            &menu_set_icon_connect_color as &dyn IsMenuItem,
            &menu_tray_icon_style as &dyn IsMenuItem,
            &menu_tray_tooltip_options as &dyn IsMenuItem,
        ];

        Submenu::with_items(LOC.tray_options, true, &menu_tray_options)?
    };

    let menu_notify_options = {
        let menu_notify_low_battery = create_menu_item
            .notify_low_battery(CONFIG.read().unwrap().notify_options.low_battery.value());
        let menu_notify_low_battery: Vec<&dyn IsMenuItem> = menu_notify_low_battery
            .iter()
            .map(|item| item as &dyn IsMenuItem)
            .collect();
        let menu_notify_low_battery =
            &Submenu::with_items(LOC.low_battery, true, &menu_notify_low_battery)?;

        let menu_notify_device_change = create_menu_item.notify_device_change();

        let mut menu_notify_options: Vec<&dyn IsMenuItem> = Vec::new();
        menu_notify_options.push(menu_notify_low_battery as &dyn IsMenuItem);
        menu_notify_options.extend(
            menu_notify_device_change
                .iter()
                .map(|item| item as &dyn IsMenuItem),
        );
        Submenu::with_items(LOC.notify_options, true, &menu_notify_options)?
    };

    let settings_items = &[
        &menu_tray_options as &dyn IsMenuItem,
        &menu_notify_options as &dyn IsMenuItem,
        &menu_open_config as &dyn IsMenuItem,
    ];
    let menu_setting = Submenu::with_items(LOC.settings, true, settings_items)?;

    *menu_manager = create_menu_item.0;

    let tray_menu = Menu::new();
    tray_menu
        .prepend_items(&menu_devices)
        .context("Failed to prepend 'Bluetooth Items' to Tray Menu")?;
    tray_menu
        .append(&menu_separator)
        .context("Failed to apped 'Separator' to Tray Menu")?;
    tray_menu
        .append(&menu_setting)
        .context("Failed to apped 'Setting' to Tray Menu")?;
    tray_menu
        .append(&menu_separator)
        .context("Failed to apped 'Separator' to Tray Menu")?;
    tray_menu
        .append(&menu_startup)
        .context("Failed to apped 'Satr up' to Tray Menu")?;
    tray_menu
        .append(&menu_separator)
        .context("Failed to apped 'Separator' to Tray Menu")?;
    tray_menu
        .append(&menu_restart)
        .context("Failed to apped 'Restart' to Tray Menu")?;
    tray_menu
        .append(&menu_separator)
        .context("Failed to apped 'Separator' to Tray Menu")?;
    tray_menu
        .append(&menu_refresh)
        .context("Failed to apped 'Refresh' to Tray Menu")?;
    tray_menu
        .append(&menu_separator)
        .context("Failed to apped 'Separator' to Tray Menu")?;
    tray_menu
        .append(&menu_about)
        .context("Failed to apped 'About' to Tray Menu")?;
    tray_menu
        .append(&menu_separator)
        .context("Failed to apped 'Separator' to Tray Menu")?;
    tray_menu
        .append(&menu_quit)
        .context("Failed to apped 'Quit' to Tray Menu")?;

    Ok(tray_menu)
}
