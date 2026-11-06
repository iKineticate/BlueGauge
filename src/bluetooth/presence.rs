use super::{
    ble::process_ble_device,
    btc::fetch_pnp_info,
    info::{BT_INFO_MAP, BluetoothInfo, BluetoothType},
};
use crate::{PROXY, UserEvent, notify::NotifyEvent};

use std::ops::Not;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use dashmap::Entry;
use log::{info, warn};
use tokio::sync::mpsc::Sender;
use windows::{
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice},
        Enumeration::{
            DeviceInformation, DeviceInformationUpdate, DeviceWatcher, DeviceWatcherStatus,
        },
    },
    Foundation::TypedEventHandler,
    core::{HSTRING, Ref},
};

#[derive(PartialEq, Eq)]
enum BluetoothPresence {
    Added,
    Removed,
}

async fn check_presence_async(
    bt_type: BluetoothType,
    presence: BluetoothPresence,
    id: HSTRING,
    tx: Sender<(BluetoothInfo, BluetoothPresence)>,
) -> Result<()> {
    match presence {
        BluetoothPresence::Added => {
            if bt_type.is_low_energy() {
                let ble_device = BluetoothLEDevice::FromIdAsync(&id)?.await?;
                match process_ble_device(&ble_device).await {
                    Ok(ble_info) => {
                        let _ = tx.send((ble_info, presence)).await;
                    }
                    Err(e) => {
                        let name = ble_device
                            .Name()
                            .ok()
                            .filter(|name| name.is_empty().not())
                            .unwrap_or_else(|| "Unknown name".into());

                        return Err(anyhow!("BLE [{name}]: Failed to get info: {e}"));
                    }
                }
            } else {
                let btc_device = BluetoothDevice::FromIdAsync(&id)?.await?;
                let process_btc_device = async |btc_device: &BluetoothDevice| {
                    let name = btc_device.Name()?.to_string();
                    let address = btc_device.BluetoothAddress()?;
                    let status =
                        btc_device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;

                    let mut pnp_devices_info = fetch_pnp_info().await?;
                    let (battery, instance_id) = pnp_devices_info
                        .remove(&address)
                        .map(|i| (i.battery, i.instance_id))
                        .ok_or_else(|| {
                            anyhow!("No matching PnP BT device for address {address:012X}")
                        })?;

                    Ok::<BluetoothInfo, anyhow::Error>(BluetoothInfo {
                        name,
                        battery,
                        status,
                        address,
                        r#type: BluetoothType::Classic(instance_id),
                    })
                };
                match process_btc_device(&btc_device).await {
                    Ok(btc_info) => {
                        let _ = tx.send((btc_info, presence)).await;
                    }
                    Err(e) => {
                        let name = btc_device
                            .Name()
                            .ok()
                            .filter(|name| name.is_empty().not())
                            .unwrap_or_else(|| "Unknown name".into());

                        return Err(anyhow!("BTC [{name}]: Failed to get info: {e}"));
                    }
                }
            };
        }
        BluetoothPresence::Removed => {
            let remove_device_address = match bt_type {
                BluetoothType::LowEnergy => {
                    let device = BluetoothLEDevice::FromIdAsync(&id)?.await?;
                    device.BluetoothAddress()?
                }
                BluetoothType::Classic(_) => {
                    let device = BluetoothDevice::FromIdAsync(&id)?.await?;
                    device.BluetoothAddress()?
                }
            };
            let remove_device_info = BluetoothInfo {
                address: remove_device_address,
                ..Default::default()
            };

            let _ = tx.send((remove_device_info, presence)).await;
        }
    }

    Ok(())
}

macro_rules! create_presence_handler {
    ($tx:ident, $current_runtime:expr, $arg_type:ty, $bt_type:expr, $presence:expr) => {{
        let handler_tx = $tx.clone();
        TypedEventHandler::new(
            move |_watcher: Ref<DeviceWatcher>, event_info: Ref<$arg_type>| {
                if let Some(info) = event_info.as_ref() {
                    let id = info.Id()?;

                    let result = match $current_runtime.as_ref() {
                        Some(handle) => handle.block_on(async {
                            check_presence_async($bt_type, $presence, id, handler_tx.clone()).await
                        }),
                        None => {
                            // 没有当前 Runtime，创建临时单线程 Runtime
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .map_err(|e| -> windows::core::Error { e.into() })?;

                            rt.block_on(async {
                                check_presence_async($bt_type, $presence, id, handler_tx.clone())
                                    .await
                            })
                        }
                    };

                    result.map_err(|e| {
                        warn!("Failed to create Presence Hnalder: {e}");
                        windows::core::Error::new(
                            windows::core::HRESULT(0x80004005u32 as i32), // E_FAIL
                            e.to_string(),
                        )
                    })?;
                }
                Ok(())
            },
        )
    }};
}

fn start_bt_presence_watch(device_watcher: &DeviceWatcher) -> Result<()> {
    let status = device_watcher.Status()?;

    if matches!(
        status,
        DeviceWatcherStatus::Aborted | DeviceWatcherStatus::Created | DeviceWatcherStatus::Stopped
    ) {
        device_watcher
            .Start()
            .with_context(|| "Failed to start watch for the DeviceWatcher")
    } else {
        Err(anyhow!(
            "DeviceWatcher is already started or starting, current status: {status:?}"
        ))
    }
}

fn stop_bt_presence_watch(device_watcher: &DeviceWatcher) -> Result<()> {
    let status = device_watcher.Status()?;

    if matches!(
        status,
        DeviceWatcherStatus::Aborted
            | DeviceWatcherStatus::EnumerationCompleted
            | DeviceWatcherStatus::Started
    ) {
        device_watcher
            .Stop()
            .with_context(|| "Failed to stop watch for the DeviceWatcher")
    } else {
        Err(anyhow!(
            "DeviceWatcher is already stoped or stoping, current status: {status:?}"
        ))
    }
}

#[rustfmt::skip]
pub async fn watch_bt_presence_async(
    exit_flag: &Arc<AtomicBool>,
    restart_flag: &Arc<AtomicUsize>,
) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let current_runtime = tokio::runtime::Handle::try_current().ok();

    let btc_filter = BluetoothDevice::GetDeviceSelector()?;
    let btc_watcher = DeviceInformation::CreateWatcherAqsFilter(&btc_filter)?;
    let btc_tokens = {
        let rt_added = current_runtime.clone();
        let rt_removed = current_runtime.clone();
        let added_handler = create_presence_handler!(tx, rt_added, DeviceInformation, BluetoothType::Classic(String::new()), BluetoothPresence::Added);
        let removed_handler = create_presence_handler!(tx, rt_removed, DeviceInformationUpdate, BluetoothType::Classic(String::new()), BluetoothPresence::Removed);
        let btc_watch_added_token = btc_watcher.Added(&added_handler)?;
        let btc_watch_removed_token = btc_watcher.Removed(&removed_handler)?;
        [btc_watch_added_token, btc_watch_removed_token]
    };

    let ble_filter = BluetoothLEDevice::GetDeviceSelector()?;
    let ble_watcher = DeviceInformation::CreateWatcherAqsFilter(&ble_filter)?;
    let ble_tokens = {
        let rt_added = current_runtime.clone();
        let rt_removed = current_runtime.clone();
        let added_handler = create_presence_handler!(tx, rt_added, DeviceInformation, BluetoothType::LowEnergy, BluetoothPresence::Added);
        let removed_handler = create_presence_handler!(tx, rt_removed, DeviceInformationUpdate, BluetoothType::LowEnergy, BluetoothPresence::Removed);
        let ble_watch_added_token = ble_watcher.Added(&added_handler)?;
        let ble_watch_removed_token = ble_watcher.Removed(&removed_handler)?;
        [ble_watch_added_token, ble_watch_removed_token]
    };

    start_bt_presence_watch(&btc_watcher)?;
    start_bt_presence_watch(&ble_watcher)?;

    scopeguard::defer! {
        btc_tokens.into_iter().enumerate().for_each(|(index, token)| match index {
            0 => { let _ = btc_watcher.RemoveAdded(token); },
            1 => { let _ = btc_watcher.RemoveRemoved(token); },
            _ => ()
        });
        ble_tokens.into_iter().enumerate().for_each(|(index, token)| match index {
            0 => { let _ = ble_watcher.RemoveAdded(token); },
            1 => { let _ = ble_watcher.RemoveRemoved(token); },
            _ => ()
        });

        stop_bt_presence_watch(&btc_watcher).unwrap();
        stop_bt_presence_watch(&ble_watcher).unwrap();
    }

    let proxy = PROXY.get().unwrap();

    loop {
        tokio::select! {
            maybe_update = rx.recv() => {
                let Some((info, presence)) = maybe_update else {
                    return Err(anyhow!("Channel closed while watching Bluetooth presence"));
                };

                let update_event = |presence: BluetoothPresence, name: String| {
                    // 设备添加/移除后，所有监听增加或移除设备
                    restart_flag.fetch_add(1, Ordering::Relaxed);
                    // 更新托盘信息
                    let _ = proxy.send_event(UserEvent::UpdateTray);
                    // 因 Watcher 无 Config，需传递给有通知配置的 APP 结构体
                    match presence {
                        BluetoothPresence::Added => {
                            info!("[{name}]: New Bluetooth Device Added");
                            let _ = proxy.send_event(UserEvent::Notify(NotifyEvent::Added(name)));
                        }
                        BluetoothPresence::Removed => {
                            info!("[{name}]: Old Bluetooth Device Removed");
                            let _ = proxy.send_event(UserEvent::Notify(NotifyEvent::Removed(name)));
                        }
                    }
                };

                if let Entry::Vacant(e) = BT_INFO_MAP.entry(info.address) {
                    match presence {
                        BluetoothPresence::Removed => (), // 原设备本就无该设备，无需移除
                        BluetoothPresence::Added => {
                            let name = info.name.clone();
                            e.insert(info);
                            update_event(presence, name);
                        }
                    }
                } else {
                    match presence {
                        BluetoothPresence::Added => (), // 原设备本就有该设备，无需添加
                        BluetoothPresence::Removed => {
                            let removed_info = BT_INFO_MAP.remove(&info.address);
                            let name = removed_info
                                .filter(|(_, i)| i.name.is_empty().not())
                                .map_or("Unknown name".to_owned(), |(_, i)| i.name);
                            update_event(presence, name);
                        }
                    }
                }
            }
            _ = async {
                while !exit_flag.load(Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            } => {
                info!("Watch Bluetooth Presence was cancelled by exit flag.");
                return Ok(());
            },
        }
    }
}
