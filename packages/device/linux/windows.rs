//! The Windows readings (LLP 1133.031 D1–D3, D9): the battery and Energy
//! Saver from the power management functions, and the internet connection
//! profile from `Windows.Networking.Connectivity`. The package reaches the
//! `windows` crate through `exact_windows::windows`, since the desktop
//! builder writes no Cargo manifest of its own.

use super::readers::{self, Battery};
use exact_linux::package::{json, Json};
use exact_windows::windows;
use std::sync::atomic::{AtomicU32, Ordering};
use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};
use windows::Win32::System::Power::{
    GetSystemPowerStatus, PowerSettingRegisterNotification, PowerSettingUnregisterNotification,
    DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY, POWERBROADCAST_SETTING, SYSTEM_POWER_STATUS,
};
use windows::Win32::System::SystemServices::{
    GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING, GUID_POWER_SAVING_STATUS,
};
use windows::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_CALLBACK;

/// `GUID_ENERGY_SAVER_STATUS` (Windows 11 24H2; [power setting GUIDs](https://learn.microsoft.com/windows/win32/power/power-setting-guids)):
/// 0 off, 1 standard, 2 high savings. The `windows` crate does not define it
/// yet; the value is the one that page gives, though it is also RFC 4122's
/// example UUID.
const GUID_ENERGY_SAVER_STATUS: windows::core::GUID =
    windows::core::GUID::from_u128(0x550e8400_e29b_41d4_a716_446655440000);

/// The battery: `BatteryFlag` 128 is no system battery, which answers the
/// specification's values; `charging` is false only on battery power.
pub fn battery() -> Battery {
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_err()
        || status.BatteryFlag & 128 != 0
        || status.BatteryFlag == 255
        || status.BatteryLifePercent > 100
    {
        return readers::NO_BATTERY;
    }
    Battery {
        level: readers::hundredths(status.BatteryLifePercent as f64 / 100.0),
        charging: status.ACLineStatus != 0,
    }
}

/// Energy Saver's last status (24H2 and later), or battery saver's before
/// it: each registration's callback is told the value at once and at each
/// change. `u32::MAX` until told.
static ENERGY_SAVER: AtomicU32 = AtomicU32::new(u32::MAX);
static BATTERY_SAVER: AtomicU32 = AtomicU32::new(u32::MAX);

/// Low power: Energy Saver not off where Windows has it, since it may stay
/// on while plugged in; else battery saver on (D3's Windows row).
pub fn low_power() -> bool {
    match ENERGY_SAVER.load(Ordering::Relaxed) {
        u32::MAX => BATTERY_SAVER.load(Ordering::Relaxed) == 1,
        status => status != 0,
    }
}

/// The internet profile's `type` and the `windows` words (D2, D9).
pub fn network() -> (&'static str, Json) {
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return ("none", Json::Null);
    };
    let kind = if profile.IsWlanConnectionProfile().unwrap_or(false) {
        "wifi"
    } else if profile.IsWwanConnectionProfile().unwrap_or(false) {
        "cellular"
    } else if profile
        .NetworkAdapter()
        .and_then(|a| a.IanaInterfaceType())
        .ok()
        == Some(6)
    {
        "ethernet"
    } else {
        "other"
    };
    let words = match profile.GetConnectionCost() {
        Ok(cost) => {
            let kind = match cost.NetworkCostType() {
                Ok(NetworkCostType::Unrestricted) => "unrestricted",
                Ok(NetworkCostType::Fixed) => "fixed",
                Ok(NetworkCostType::Variable) => "variable",
                _ => "unknown",
            };
            json!({
                "networkCostType": kind,
                "roaming": cost.Roaming().unwrap_or(false),
                "approachingDataLimit": cost.ApproachingDataLimit().unwrap_or(false),
                "overDataLimit": cost.OverDataLimit().unwrap_or(false),
                "backgroundDataUsageRestricted": cost.BackgroundDataUsageRestricted().unwrap_or(false),
            })
        }
        Err(_) => Json::Null,
    };
    (kind, words)
}

/// A registration for power setting changes, unregistered when dropped.
pub struct Registration(Vec<HPOWERNOTIFY>, Box<Box<dyn Fn() + Send + Sync>>);

impl Drop for Registration {
    fn drop(&mut self) {
        for handle in self.0.drain(..) {
            unsafe {
                let _ = PowerSettingUnregisterNotification(handle);
            }
        }
    }
}

unsafe extern "system" fn heard(context: *const core::ffi::c_void, _kind: u32, setting: *const core::ffi::c_void) -> u32 {
    let setting = &*(setting as *const POWERBROADCAST_SETTING);
    if setting.DataLength >= 4 {
        let value = u32::from_ne_bytes(std::ptr::read_unaligned(setting.Data.as_ptr() as *const [u8; 4]));
        if setting.PowerSetting == GUID_ENERGY_SAVER_STATUS {
            ENERGY_SAVER.store(value, Ordering::Relaxed);
        } else if setting.PowerSetting == GUID_POWER_SAVING_STATUS {
            BATTERY_SAVER.store(value, Ordering::Relaxed);
        }
    }
    let then = &*(context as *const Box<dyn Fn() + Send + Sync>);
    then();
    0
}

/// Register for the battery's settings (`battery`) or the power-saving ones
/// (else), calling `then` at each change, with no window
/// (`DEVICE_NOTIFY_CALLBACK`).
pub fn watch_power(battery: bool, then: Box<dyn Fn() + Send + Sync>) -> Registration {
    let then = Box::new(then);
    let mut handles = Vec::new();
    let settings: &[windows::core::GUID] = if battery {
        &[GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING]
    } else {
        &[GUID_ENERGY_SAVER_STATUS, GUID_POWER_SAVING_STATUS]
    };
    for guid in settings {
        let mut parameters = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(heard),
            Context: &*then as *const Box<dyn Fn() + Send + Sync> as *mut core::ffi::c_void,
        };
        let mut handle: *mut core::ffi::c_void = std::ptr::null_mut();
        let error = unsafe {
            PowerSettingRegisterNotification(
                guid,
                DEVICE_NOTIFY_CALLBACK,
                windows::Win32::Foundation::HANDLE(&mut parameters as *mut _ as *mut core::ffi::c_void),
                &mut handle,
            )
        };
        if error.0 == 0 {
            handles.push(HPOWERNOTIFY(handle as isize));
        }
    }
    Registration(handles, then)
}

/// Watch the internet profile; the returned token removes the handler.
pub fn watch_network(then: Box<dyn Fn() + Send + Sync>) -> Option<i64> {
    let handler = windows::Networking::Connectivity::NetworkStatusChangedEventHandler::new(move |_| {
        then();
        Ok(())
    });
    NetworkInformation::NetworkStatusChanged(&handler).ok()
}

pub fn unwatch_network(token: i64) {
    let _ = NetworkInformation::RemoveNetworkStatusChanged(token);
}
