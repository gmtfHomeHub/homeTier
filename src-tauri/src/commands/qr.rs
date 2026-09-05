//! 通用二维码解析命令（与业务无关，仅做事件路由 + 解密）。

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Serialize;

/// `parse_qr` 返回：事件标识 + base64 编码的业务数据。
#[derive(Serialize)]
pub struct ParseQrResult {
    pub event: String,
    pub data: String,
}

/// 解析通用二维码链接，返回 `event` 与 base64 编码的解密数据。
///
/// 业务方在前端按 `event` 分发：如 `j_s`（join_space）再调
/// [`crate::commands::space::parse_share_data`] 还原 `ShareInfo`。
#[tauri::command]
pub async fn parse_qr(link: String) -> Result<ParseQrResult, String> {
    let (event, data) = crate::qr::decrypt_qr(&link)?;
    Ok(ParseQrResult {
        event,
        data: STANDARD.encode(&data),
    })
}
