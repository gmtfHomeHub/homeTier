#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ};

/// 跨平台 machine-id 读取
pub fn get_machine_id() -> Option<String> {
    #[cfg(target_os = "linux")]
    return {
        std::fs::read_to_string("/etc/machine-id")
            .map(|s| s.trim().to_string())
            .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id").map(|s| s.trim().to_string()))
            .ok()
    };

    #[cfg(target_os = "macos")]
    return {
        match std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.lines()
                    .find(|l| l.contains("IOPlatformUUID"))
                    .and_then(|l| {
                        let start = l.find('"')?;
                        let end = l[start + 1..].find('"')?;
                        Some(l[start + 1..start + 1 + end].to_string())
                    })
            }
            Err(_) => None,
        }
    };

    #[cfg(windows)]
    return {
        let mut key = std::ptr::null_mut();
        let wide_path: Vec<u16> = "SOFTWARE\\Microsoft\\Cryptography\0".encode_utf16().collect();
        unsafe {
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(wide_path.as_ptr()), 0, KEY_READ, &mut key).is_ok() {
                let mut buf = [0u16; 256];
                let mut size = (buf.len() * 2) as u32;
                let val_name: Vec<u16> = "MachineGuid\0".encode_utf16().collect();
                if RegQueryValueExW(key, PCWSTR(val_name.as_ptr()), std::ptr::null_mut(), std::ptr::null_mut(), buf.as_mut_ptr() as *mut _, &mut size).is_ok() {
                    let s = String::from_utf16_lossy(&buf);
                    return Some(s);
                }
            }
            None
        }
    };

    #[cfg(any(target_os = "android", target_os = "ios"))]
    None
}