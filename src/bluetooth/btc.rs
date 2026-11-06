use super::info::{BT_INFO_MAP, BluetoothInfo, BluetoothType};
use super::pnp_battery::{PnpDeviceInfo, enumerate_bt_batteries, read_bt_battery_by_instance_id};
use crate::{PROXY, UserEvent, notify::NotifyEvent};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;
use futures::StreamExt;
use tokio::sync::{Mutex, mpsc::Sender};
use windows::{
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice},
        Enumeration::DeviceInformation,
    },
    Foundation::TypedEventHandler,
};

const MAX_RETRIES: u32 = 2;
const RETRY_DELAY: tokio::time::Duration = tokio::time::Duration::from_secs(1);

pub async fn find_btc_devices() -> Result<Vec<BluetoothDevice>> {
    let btc_aqs_filter = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;

    let btc_devices_info = DeviceInformation::FindAllAsyncAqsFilter(&btc_aqs_filter)?
        .await
        .with_context(|| "Failed to find BTC from AqsFilter")?;

    let btc_devices = futures::stream::iter(btc_devices_info)
        .filter_map(|device_info| async move {
            let device_id = device_info.Id().ok()?;
            BluetoothDevice::FromIdAsync(&device_id).ok()?.await.ok()
        })
        .collect::<Vec<_>>()
        .await;

    Ok(btc_devices)
}

pub async fn fetch_pnp_info() -> Result<HashMap<u64, PnpDeviceInfo>> {
    // 获取Pnp设备可能出错（初始化可能失败），需重试多次避开错误
    for attempt in 1..=MAX_RETRIES {
        match tokio::task::spawn_blocking(enumerate_bt_batteries).await? {
            Ok(info) => return Ok(info),
            Err(e) if attempt < MAX_RETRIES => {
                log::error!(
                    "Failed to get PnP device information: {e}, retrying in {} seconds... (attempt {attempt}/{MAX_RETRIES})",
                    RETRY_DELAY.as_secs()
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                return Err(anyhow!(
                    "Failed to enumerate PnP devices after {MAX_RETRIES} attempts: {e}"
                ));
            }
        }
    }
    unreachable!("loop should always return or error")
}

fn process_btc_device(
    btc_device: &BluetoothDevice,
    pnp_devices_info: &HashMap<u64, PnpDeviceInfo>,
) -> Result<BluetoothInfo> {
    let btc_name = btc_device.Name()?.to_string().trim().to_owned();

    let btc_address = btc_device.BluetoothAddress()?;

    let (pnp_instance_id, btc_battery) = pnp_devices_info
        .get(&btc_address)
        .map(|i| (i.instance_id.clone(), i.battery))
        .ok_or_else(|| anyhow!("BTC [{btc_name}]: No matching BTC in Pnp devices"))?;

    let btc_status = btc_device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;

    Ok(BluetoothInfo {
        name: btc_name,
        battery: btc_battery,
        status: btc_status,
        address: btc_address,
        r#type: BluetoothType::Classic(pnp_instance_id),
    })
}

pub async fn get_btc_devices_info(
    btc_devices: &[BluetoothDevice],
) -> Result<DashMap<u64, BluetoothInfo>> {
    let pnp_devices_info = fetch_pnp_info().await?;

    let devices_info: DashMap<u64, BluetoothInfo> = DashMap::new();

    btc_devices.iter().for_each(|btc_device| {
        match process_btc_device(btc_device, &pnp_devices_info) {
            Ok(i) => {
                devices_info.insert(i.address, i);
            }
            Err(e) => log::warn!("{e}"),
        };
    });

    Ok(devices_info)
}

pub async fn watch_btc_devices_battery(
    exit_flag: &Arc<AtomicBool>,
    restart_flag: &Arc<AtomicUsize>,
) -> Result<()> {
    let mut local_generation = 0;

    let get_btc_devices_info = || {
        BT_INFO_MAP
            .iter()
            .filter_map(|entry| entry.is_btc().then_some(entry.value().clone()))
            .collect::<Vec<_>>()
    };

    let mut original_btc_devices_info = get_btc_devices_info();

    let proxy = PROXY.get().unwrap();

    while !exit_flag.load(Ordering::Relaxed) {
        let current_generation = restart_flag.load(Ordering::Relaxed);
        if local_generation < current_generation {
            log::info!("Watch BTC Batttery restart by restart flag.");
            local_generation = current_generation;
            original_btc_devices_info = get_btc_devices_info();
            continue;
        }

        let btc_devices: Vec<(u64, u8)> = original_btc_devices_info
            .iter()
            .filter_map(|info| {
                let original_battery = info.battery;
                info.get_btc_instance_id()
                    .and_then(|id| read_bt_battery_by_instance_id(&id))
                    .filter(|current_battery| original_battery.ne(current_battery))
                    .map(|battery| (info.address, battery))
            })
            .collect();

        let mut need_update = false;
        for (address, new_battery) in btc_devices.into_iter() {
            if let Some(mut info) = BT_INFO_MAP.get_mut(&address) {
                let name = info.name.clone();
                let old_battery = info.battery;
                log::info!("BTC [{name}]: Battery {old_battery} -> {new_battery}");
                need_update = true;
                info.battery = new_battery;
                let _ = proxy.send_event(UserEvent::Notify(NotifyEvent::LowBattery(
                    name,
                    new_battery,
                    address,
                )));
            };
        }

        if need_update {
            original_btc_devices_info = get_btc_devices_info();
            let _ = proxy.send_event(UserEvent::UpdateTray);
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    Ok(())
}

type WatchBTCGuard = (BluetoothDevice, i64);

async fn watch_btc_device_status(
    btc_address: u64,
    btc_device: BluetoothDevice,
    tx: Sender<(u64, bool)>,
) -> Result<WatchBTCGuard> {
    let tx_status = tx.clone();
    let connection_status_token = {
        let handler =
            TypedEventHandler::new(move |sender: windows::core::Ref<BluetoothDevice>, _args| {
                if let Some(btc) = sender.as_ref() {
                    let status = btc.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    let _ = tx_status
                        .try_send((btc_address, status))
                        .inspect_err(|e| log::error!("Failed to send BTC status update: {e}"));
                }
                Ok(())
            });
        btc_device.ConnectionStatusChanged(&handler)?
    };

    Ok((btc_device, connection_status_token))
}

fn get_btc_devices_address<C: FromIterator<u64>>() -> C {
    BT_INFO_MAP
        .iter()
        .filter_map(|entry| entry.is_btc().then_some(*entry.key()))
        .collect()
}

async fn get_btc_device_from_address(address: u64) -> Result<BluetoothDevice> {
    BluetoothDevice::FromBluetoothAddressAsync(address)?
        .await
        .with_context(|| format!("Failed to find BTC device from ({address})"))
}

pub async fn watch_btc_devices_status_async(
    exit_flag: &Arc<AtomicBool>,
    restart_flag: &Arc<AtomicUsize>,
) -> Result<()> {
    let mut local_generation = 0;

    let original_btc_devices_address = Arc::new(Mutex::new(HashSet::new()));

    let btc_devices = futures::stream::iter(get_btc_devices_address::<Vec<_>>())
        .filter_map(|address| {
            let original_btc_devices_address = original_btc_devices_address.clone();
            async move {
                if let Ok(btc_device) = get_btc_device_from_address(address).await {
                    original_btc_devices_address.lock().await.insert(address);
                    Some((address, btc_device))
                } else {
                    None
                }
            }
        })
        .collect::<Vec<_>>()
        .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let mut guard = scopeguard::guard(HashMap::<u64, WatchBTCGuard>::new(), |map| {
        for (device, connection_status_token) in map.into_values() {
            let _ = device.RemoveConnectionStatusChanged(connection_status_token);
        }
    });

    for (btc_address, btc_device) in btc_devices {
        let watch_btc_guard = watch_btc_device_status(btc_address, btc_device, tx.clone()).await?;

        guard.insert(btc_address, watch_btc_guard);
    }

    let proxy = PROXY.get().unwrap();

    loop {
        tokio::select! {
            maybe_update = rx.recv() => {
                let (address, status) = maybe_update.ok_or_else(|| anyhow!("Channel closed while watching BTC devices"))?;

                if let Some(mut update_device) = BT_INFO_MAP.get_mut(&address)
                    && update_device.status != status {
                        log::info!("BTC [{}]: Status -> {status}", update_device.name);
                        let notify_event = if status {
                            NotifyEvent::Reconnect(update_device.name.clone())
                        } else {
                            NotifyEvent::Disconnect(update_device.name.clone())
                        };
                        update_device.status = status;
                        drop(update_device);
                        let _ = proxy.send_event(UserEvent::Notify(notify_event));
                        let _ = proxy.send_event(UserEvent::UpdateTray);
                    }
            },
            _ = async {
                while !exit_flag.load(Ordering::Relaxed) {
                    let current_generation = restart_flag.load(Ordering::Relaxed);
                    if local_generation < current_generation {
                        log::info!("Watch BTC Status restart by restart flag.");
                        local_generation = current_generation;

                        let current_btc_devices_address: HashSet<_> = get_btc_devices_address();

                        let original_btc_devices_address_clone = original_btc_devices_address.lock().await.clone();

                        let removed_devices = original_btc_devices_address_clone
                            .difference(&current_btc_devices_address)
                            .cloned()
                            .collect::<Vec<_>>();

                        let added_devices = current_btc_devices_address
                            .difference(&original_btc_devices_address_clone)
                            .cloned()
                            .collect::<Vec<_>>();

                        for removed_device in removed_devices {
                            guard.remove(&removed_device);
                            original_btc_devices_address.lock().await.remove(&removed_device);
                        }

                        for added_device_address in added_devices {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            let Ok(btc_device) = get_btc_device_from_address(added_device_address).await else {
                                // 移除错误设备
                                BT_INFO_MAP.remove(&added_device_address);
                                log::warn!("Failed to get added BTC Device from address");
                                continue;
                            };

                            let name = btc_device.Name().unwrap_or_else(|_| "Unknown name".into());

                            match watch_btc_device_status(added_device_address, btc_device, tx.clone()).await  {
                                Ok(watch_ble_guard) => {
                                    guard.insert(added_device_address, watch_ble_guard);
                                    original_btc_devices_address.lock().await.insert(added_device_address);
                                },
                                Err(e) => {
                                    // 移除错误设备
                                    BT_INFO_MAP.remove(&added_device_address);
                                    log::warn!("BTC [{name}]: Failed to watch added BTC Device - {e}");
                                }
                            }
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }

                log::info!("Watch BTC Status was cancelled by exit flag.");
            } => return Ok(()),
        }
    }
}
