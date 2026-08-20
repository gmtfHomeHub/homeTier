//! Error handling for iOS staticlib

use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::OnceLock;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("EasyTier error: {0}")]
    EasyTier(#[from] easytier::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Instance error: {0}")]
    Instance(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Nul error: {0}")]
    Nul(#[from] std::ffi::NulError),

    #[error("Runtime error: {0}")]
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, Error>;

static LAST_ERROR: OnceLock<std::sync::Mutex<Option<String>>> = OnceLock::new();

pub fn set_last_error(msg: String) {
    LAST_ERROR.get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .replace(msg);
}

pub fn take_last_error() -> Option<String> {
    LAST_ERROR.get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .take()
}

pub fn get_last_error() -> Option<String> {
    LAST_ERROR.get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap()
        .clone()
}

#[no_mangle]
pub extern "C" fn free_string(s: *const c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s as *mut c_char)) };
    }
}