pub mod server;
pub mod share;

#[cfg(any(target_os = "android", target_os = "ios"))]
pub mod mobile;