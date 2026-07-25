use tauri::State;
use crate::db::Database;
use crate::types::AclRule;
use std::sync::Arc;

fn row_to_rule(row: crate::db::models::AclRuleRow) -> AclRule {
    AclRule {
        id: row.id,
        space_id: row.space_id,
        action: row.action,
        source: row.source,
        dest: row.dest,
        ports: row.ports,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[tauri::command]
pub async fn get_acl_rules(
    space_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AclRule>, String> {
    crate::log_debug!(format!("获取 ACL 规则: space_id={}", space_id));
    let rows = db.get_acl_rules(&space_id)?;
    Ok(rows.into_iter().map(row_to_rule).collect())
}

#[tauri::command]
pub async fn create_acl_rule(
    space_id: String,
    action: String,
    source: String,
    dest: String,
    ports: String,
    description: String,
    db: State<'_, Arc<Database>>,
) -> Result<AclRule, String> {
    let now = chrono::Local::now().to_rfc3339();
    let row = crate::db::models::AclRuleRow {
        id: uuid::Uuid::new_v4().to_string(),
        space_id: space_id.clone(),
        action,
        source,
        dest,
        ports,
        description,
        created_at: now.clone(),
        updated_at: now,
    };
    db.insert_acl_rule(&row)?;
    crate::log_info!(format!("创建 ACL 规则: space_id={}", space_id), &space_id);
    Ok(row_to_rule(row))
}

#[tauri::command]
pub async fn update_acl_rule(
    space_id: String,
    rule_id: String,
    action: Option<String>,
    source: Option<String>,
    dest: Option<String>,
    ports: Option<String>,
    description: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<AclRule, String> {
    let existing = db.get_acl_rules(&space_id)?;
    let row = existing.into_iter().find(|r| r.id == rule_id)
        .ok_or_else(|| format!("ACL 规则不存在: {}", rule_id))?;

    let updated = crate::db::models::AclRuleRow {
        action: action.unwrap_or(row.action),
        source: source.unwrap_or(row.source),
        dest: dest.unwrap_or(row.dest),
        ports: ports.unwrap_or(row.ports),
        description: description.unwrap_or(row.description),
        updated_at: chrono::Local::now().to_rfc3339(),
        ..row
    };
    db.update_acl_rule(&updated)?;
    crate::log_info!(format!("更新 ACL 规则: space_id={}, rule_id={}", space_id, rule_id), &space_id);
    Ok(row_to_rule(updated))
}

#[tauri::command]
pub async fn delete_acl_rule(
    space_id: String,
    rule_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let _ = uuid::Uuid::parse_str(&space_id).map_err(|e| e.to_string())?;
    db.delete_acl_rule(&rule_id)?;
    crate::log_info!(format!("删除 ACL 规则: space_id={}, rule_id={}", space_id, rule_id), &space_id);
    Ok(())
}
