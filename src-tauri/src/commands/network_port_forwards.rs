use tauri::State;
use crate::db::Database;
use crate::types::PortForwardRule;
use std::sync::Arc;

fn row_to_rule(row: crate::db::models::PortForwardRuleRow) -> PortForwardRule {
    PortForwardRule {
        id: row.id,
        space_id: row.space_id,
        name: row.name,
        protocol: row.protocol,
        source_ip: row.source_ip,
        source_port: row.source_port,
        target_ip: row.target_ip,
        target_port: row.target_port,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[tauri::command]
pub async fn get_port_forward_rules(
    space_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<PortForwardRule>, String> {
    crate::log_debug!(format!("获取端口转发规则: space_id={}", space_id));
    let rows = db.get_port_forward_rules(&space_id)?;
    Ok(rows.into_iter().map(row_to_rule).collect())
}

#[tauri::command]
pub async fn create_port_forward_rule(
    space_id: String,
    name: String,
    protocol: String,
    source_ip: String,
    source_port: i32,
    target_ip: String,
    target_port: i32,
    description: String,
    db: State<'_, Arc<Database>>,
) -> Result<PortForwardRule, String> {
    let now = chrono::Local::now().to_rfc3339();
    let row = crate::db::models::PortForwardRuleRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id: space_id.clone(),
        name,
        protocol,
        source_ip,
        source_port,
        target_ip,
        target_port,
        description,
        created_at: now.clone(),
        updated_at: now,
    };
    db.insert_port_forward_rule(&row)?;
    crate::log_info!(format!("创建端口转发规则: space_id={}", space_id), &space_id);
    Ok(row_to_rule(row))
}

#[tauri::command]
pub async fn update_port_forward_rule(
    space_id: String,
    rule_id: String,
    name: Option<String>,
    protocol: Option<String>,
    source_ip: Option<String>,
    source_port: Option<i32>,
    target_ip: Option<String>,
    target_port: Option<i32>,
    description: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<PortForwardRule, String> {
    let existing = db.get_port_forward_rules(&space_id)?;
    let row = existing.into_iter().find(|r| r.id == rule_id)
        .ok_or_else(|| format!("端口转发规则不存在: {}", rule_id))?;

    let updated = crate::db::models::PortForwardRuleRow {
        name: name.unwrap_or(row.name),
        protocol: protocol.unwrap_or(row.protocol),
        source_ip: source_ip.unwrap_or(row.source_ip),
        source_port: source_port.unwrap_or(row.source_port),
        target_ip: target_ip.unwrap_or(row.target_ip),
        target_port: target_port.unwrap_or(row.target_port),
        description: description.unwrap_or(row.description),
        updated_at: chrono::Local::now().to_rfc3339(),
        ..row
    };
    db.update_port_forward_rule(&updated)?;
    crate::log_info!(format!("更新端口转发规则: space_id={}, rule_id={}", space_id, rule_id), &space_id);
    Ok(row_to_rule(updated))
}

#[tauri::command]
pub async fn delete_port_forward_rule(
    space_id: String,
    rule_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let _ = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    db.delete_port_forward_rule(&rule_id)?;
    crate::log_info!(format!("删除端口转发规则: space_id={}, rule_id={}", space_id, rule_id), &space_id);
    Ok(())
}
