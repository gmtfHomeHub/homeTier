//! iOS NetworkExtension FFI layer for homeTier
//!
//! This crate exposes C-compatible functions that the Swift
//! NEPacketTunnelProvider can call to start/stop the easytier
//! network instance and inject the TUN file descriptor.

#![cfg(target_os = "ios")]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use easytier::{config::Config as EasyTierConfig, launcher::NetworkInstance, Error as EasyTierError};
use parking_lot::RwLock;
use serde_json::Value;
use thiserror::Error;
use tokio::runtime::Runtime;

mod error;
mod instance;
mod logger;

pub use error::{set_last_error, take_last_error, Error, Result};
pub use logger::init_logger;
use instance::NetworkInstanceWrapper;

/// Global singleton for the running network instance
static INSTANCE: OnceLock<Arc<RwLock<Option<NetworkInstanceWrapper>>>> = OnceLock::new();

/// Global Tokio runtime
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Get or initialize the global Tokio runtime
fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

/// Get the global instance lock
fn get_instance_lock() -> &'static Arc<RwLock<Option<NetworkInstanceWrapper>>> {
    INSTANCE.get_or_init(|| Arc::new(RwLock::new(None)))
}

/// Initialize logger for iOS (writes to App Group container or os_log)
#[no_mangle]
pub extern "C" fn init_logger(
    path: *const c_char,
    level: *const c_char,
    subsystem: *const c_char,
    err: *mut *const c_char,
) -> c_int {
    let path = unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned();
    let level = unsafe { CStr::from_ptr(level) }.to_string_lossy().into_owned();
    let subsystem = unsafe { CStr::from_ptr(subsystem) }.to_string_lossy().into_owned();

    match logger::setup_ios_logger(&path, &level, &subsystem) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(e);
            if !err.is_null() {
                unsafe { *err = CString::new(e.to_string()).unwrap().into_raw() };
            }
            -1
        }
    }
}

/// Clear the logger
#[no_mangle]
pub extern "C" fn clear_logger(err: *mut *const c_char) -> c_int {
    logger::clear_logger();
    0
}

/// Start the network instance with the given TOML configuration string
#[no_mangle]
pub extern "C" fn run_network_instance(cfg_str: *const c_char, err: *mut *const c_char) -> c_int {
    let cfg_str = unsafe { CStr::from_ptr(cfg_str) }.to_string_lossy().into_owned();

    // Parse the configuration (expects JSON format from Swift side)
    let config: Value = match serde_json::from_str(&cfg_str) {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("Failed to parse config JSON: {}", e);
            set_last_error(msg.clone());
            if !err.is_null() {
                unsafe { *err = CString::new(msg).unwrap().into_raw() };
            }
            return -1;
        }
    };

    // Convert to EasyTier Config
    let easytier_config = match instance::json_to_easytier_config(config) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e.to_string());
            if !err.is_null() {
                unsafe { *err = CString::new(e.to_string()).unwrap().into_raw() };
            }
            return -1;
        }
    };

    let rt = get_runtime();
    let instance_lock = get_instance_lock();

    rt.block_on(async {
        let mut guard = instance_lock.write();
        if guard.is_some() {
            let msg = "Network instance already running".to_string();
            set_last_error(msg.clone());
            if !err.is_null() {
                unsafe { *err = CString::new(msg).unwrap().into_raw() };
            }
            return -1;
        }

        // Create and start the network instance
        match NetworkInstanceWrapper::new(easytier_config).await {
            Ok(wrapper) => {
                *guard = Some(wrapper);
                0
            }
            Err(e) => {
                set_last_error(e.to_string());
                if !err.is_null() {
                    unsafe { *err = CString::new(e.to_string()).unwrap().into_raw() };
                }
                -1
            }
        }
    })
}

/// Stop the network instance
#[no_mangle]
pub extern "C" fn stop_network_instance() -> c_int {
    let instance_lock = get_instance_lock();
    let rt = get_runtime();

    rt.block_on(async {
        let mut guard = instance_lock.write();
        if let Some(wrapper) = guard.take() {
            wrapper.stop().await;
        }
    });
    0
}

/// Inject the TUN file descriptor into the running network instance
#[no_mangle]
pub extern "C" fn set_tun_fd(fd: c_int, err: *mut *const c_char) -> c_int {
    let instance_lock = get_instance_lock();
    let rt = get_runtime();

    rt.block_on(async {
        let guard = instance_lock.read();
        if let Some(wrapper) = guard.as_ref() {
            if let Err(e) = wrapper.set_tun_fd(fd).await {
                set_last_error(e.to_string());
                if !err.is_null() {
                    unsafe { *err = CString::new(e.to_string()).unwrap().into_raw() };
                }
                return -1;
            }
            0
        } else {
            let msg = "No running network instance".to_string();
            set_last_error(msg.clone());
            if !err.is_null() {
                unsafe { *err = CString::new(msg).unwrap().into_raw() };
            }
            -1
        }
    })
}

/// Register a callback to be called when the network instance stops
#[no_mangle]
pub extern "C" fn register_stop_callback(
    cb: Option<extern "C" fn()>,
    err: *mut *const c_char,
) -> c_int {
    let instance_lock = get_instance_lock();
    let guard = instance_lock.read();
    if let Some(wrapper) = guard.as_ref() {
        wrapper.register_stop_callback(cb);
        0
    } else {
        let msg = "No running network instance".to_string();
        set_last_error(msg.clone());
        if !err.is_null() {
            unsafe { *err = CString::new(msg).unwrap().into_raw() };
        }
        -1
    }
}

/// Register a callback for periodic running info updates
#[no_mangle]
pub extern "C" fn register_running_info_callback(
    cb: Option<extern "C" fn()>,
    err: *mut *const c_char,
) -> c_int {
    let instance_lock = get_instance_lock();
    let guard = instance_lock.read();
    if let Some(wrapper) = guard.as_ref() {
        wrapper.register_running_info_callback(cb);
        0
    } else {
        let msg = "No running network instance".to_string();
        set_last_error(msg.clone());
        if !err.is_null() {
            unsafe { *err = CString::new(msg).unwrap().into_raw() };
        }
        -1
    }
}

/// Get the current running info as JSON string
#[no_mangle]
pub extern "C" fn get_running_info(
    json: *mut *const c_char,
    err: *mut *const c_char,
) -> c_int {
    let instance_lock = get_instance_lock();
    let guard = instance_lock.read();
    if let Some(wrapper) = guard.as_ref() {
        match wrapper.get_running_info() {
            Ok(info) => {
                let cstr = CString::new(info).unwrap();
                unsafe { *json = cstr.into_raw() };
                0
            }
            Err(e) => {
                set_last_error(e.to_string());
                if !err.is_null() {
                    unsafe { *err = CString::new(e.to_string()).unwrap().into_raw() };
                }
                -1
            }
        }
    } else {
        let msg = "No running network instance".to_string();
        set_last_error(msg.clone());
        if !err.is_null() {
            unsafe { *err = CString::new(msg).unwrap().into_raw() };
        }
        -1
    }
}

/// Get the latest error message
#[no_mangle]
pub extern "C" fn get_latest_error_msg(
    msg: *mut *const c_char,
    err: *mut *const c_char,
) -> c_int {
    if let Some(e) = take_last_error() {
        let cstr = CString::new(e).unwrap();
        unsafe { *msg = cstr.into_raw() };
        0
    } else {
        let cstr = CString::new("").unwrap();
        unsafe { *msg = cstr.into_raw() };
        0
    }
}

/// Free a string returned by this library
#[no_mangle]
pub extern "C" fn free_string(s: *const c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s as *mut c_char)) };
    }
}