pub mod models;
pub mod migrations;

use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("DB open error: {}", e))?;
        let db = Self { conn: Mutex::new(conn) };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(migrations::SCHEMA_SQL)
            .map_err(|e| format!("Migration error: {}", e))?;

        // 兼容性迁移：逐个执行 ALTER TABLE，忽略重复列错误
        for sql in migrations::SCHEMA_MIGRATIONS {
            let _ = conn.execute(sql, []);
        }
        Ok(())
    }

    // --- Spaces ---

    pub fn insert_space(&self, space: &models::SpaceRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO spaces (id, name, owner_id, network_name, network_secret, description, created_at, is_auto_connect, config_json, local_config_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                space.id, space.name, space.owner_id, space.network_name, space.network_secret,
                space.description, space.created_at, space.is_auto_connect,
                space.config_json, space.local_config_json,
            ],
        ).map_err(|e| format!("Insert space error: {}", e))?;
        Ok(())
    }

    pub fn update_space(&self, space: &models::SpaceRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE spaces SET name=?1, description=?2, is_auto_connect=?3, config_json=?4, local_config_json=?5, last_connected_at=?6 WHERE id=?7",
            params![space.name, space.description, space.is_auto_connect, space.config_json, space.local_config_json, space.last_connected_at, space.id],
        ).map_err(|e| format!("Update space error: {}", e))?;
        Ok(())
    }

    pub fn delete_space(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM spaces WHERE id=?1", params![id])
            .map_err(|e| format!("Delete space error: {}", e))?;
        conn.execute("DELETE FROM members WHERE space_id=?1", params![id])
            .map_err(|e| format!("Delete members error: {}", e))?;
        Ok(())
    }

    pub fn list_spaces(&self) -> Result<Vec<models::SpaceRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, name, owner_id, network_name, network_secret, description, created_at, last_connected_at, is_auto_connect, config_json, local_config_json FROM spaces ORDER BY created_at DESC"
        ).map_err(|e| format!("Query error: {}", e))?;

        let rows = stmt.query_map([], |row| {
            Ok(models::SpaceRow {
                id: row.get(0)?,
                name: row.get(1)?,
                owner_id: row.get(2)?,
                network_name: row.get(3)?,
                network_secret: row.get(4)?,
                description: row.get(5)?,
                created_at: row.get(6)?,
                last_connected_at: row.get(7)?,
                is_auto_connect: row.get(8)?,
                config_json: row.get(9)?,
                local_config_json: row.get(10)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut spaces = Vec::new();
        for row in rows {
            spaces.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(spaces)
    }

    // --- Messages ---

    pub fn insert_message(&self, msg: &models::MessageRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO messages (id, space_id, sender_id, sender_name, type, content, timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![msg.id, msg.space_id, msg.sender_id, msg.sender_name, msg.msg_type, msg.content, msg.timestamp, msg.status],
        ).map_err(|e| format!("Insert message error: {}", e))?;
        Ok(())
    }

    pub fn get_messages(&self, space_id: &str, limit: u32) -> Result<Vec<models::MessageRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, sender_id, sender_name, type, content, timestamp, status FROM messages WHERE space_id=?1 ORDER BY timestamp DESC LIMIT ?2"
        ).map_err(|e| format!("Query error: {}", e))?;

        let rows = stmt.query_map(params![space_id, limit], |row| {
            Ok(models::MessageRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                sender_id: row.get(2)?,
                sender_name: row.get(3)?,
                msg_type: row.get(4)?,
                content: row.get(5)?,
                timestamp: row.get(6)?,
                status: row.get(7)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(messages)
    }

    // --- Settings ---

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key=?1")
            .map_err(|e| format!("Query error: {}", e))?;
        let result = stmt.query_row(params![key], |row| row.get::<_, String>(0));
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        ).map_err(|e| format!("Upsert error: {}", e))?;
        Ok(())
    }

    // --- EasyTier Config ---

    /// 获取空间的 EasyTier 配置（config_json）
    pub fn get_space_config(&self, space_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT config_json FROM spaces WHERE id=?1")
            .map_err(|e| format!("Query error: {}", e))?;
        let result = stmt.query_row(params![space_id], |row| row.get::<_, Option<String>>(0));
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }

    /// 获取空间的本地配置（local_config_json）
    pub fn get_local_config_json(&self, space_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT local_config_json FROM spaces WHERE id=?1")
            .map_err(|e| format!("Query error: {}", e))?;
        let result = stmt.query_row(params![space_id], |row| row.get::<_, Option<String>>(0));
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }

    /// 更新空间的本地配置（local_config_json）
    pub fn update_local_config_json(&self, space_id: &str, config_json: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE spaces SET local_config_json=?1 WHERE id=?2",
            params![config_json, space_id],
        ).map_err(|e| format!("Update local config error: {}", e))?;
        Ok(())
    }

    /// 更新空间的 EasyTier 配置（config_json）
    pub fn update_space_config(&self, space_id: &str, config_json: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE spaces SET config_json=?1 WHERE id=?2",
            params![config_json, space_id],
        ).map_err(|e| format!("Update config error: {}", e))?;
        Ok(())
    }

    // --- Members ---

    pub fn add_member(&self, space_id: &str, member_id: &str, nickname: &str, is_owner: bool) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO members (id, space_id, nickname, is_owner, joined_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![member_id, space_id, nickname, is_owner as i32, chrono::Local::now().to_rfc3339()],
        ).map_err(|e| format!("Add member error: {}", e))?;
        Ok(())
    }

    pub fn remove_member(&self, space_id: &str, member_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM members WHERE id=?1 AND space_id=?2",
            params![member_id, space_id],
        ).map_err(|e| format!("Remove member error: {}", e))?;
        Ok(())
    }

    pub fn list_members(&self, space_id: &str) -> Result<Vec<models::MemberRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, nickname, virtual_ip, is_online, is_owner, joined_at, last_seen_at FROM members WHERE space_id=?1 ORDER BY is_owner DESC, joined_at ASC"
        ).map_err(|e| format!("Query error: {}", e))?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(models::MemberRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                nickname: row.get(2)?,
                virtual_ip: row.get(3)?,
                is_online: row.get(4)?,
                is_owner: row.get(5)?,
                joined_at: row.get(6)?,
                last_seen_at: row.get(7)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut members = Vec::new();
        for row in rows {
            members.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(members)
    }

    // --- Space Apps ---

    pub fn insert_app(&self, app: &models::AppRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO space_apps (id, space_id, name, category, icon, protocol, hostname, port, pathname, sort_order, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                app.id, app.space_id, app.name, app.category, app.icon,
                app.protocol, app.hostname, app.port, app.pathname,
                app.sort_order, app.created_by, app.created_at,
            ],
        ).map_err(|e| format!("Insert app error: {}", e))?;
        Ok(())
    }

    pub fn update_app(&self, app: &models::AppRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE space_apps SET name=?1, category=?2, icon=?3, protocol=?4, hostname=?5, port=?6, pathname=?7, sort_order=?8 WHERE id=?9",
            params![app.name, app.category, app.icon, app.protocol, app.hostname, app.port, app.pathname, app.sort_order, app.id],
        ).map_err(|e| format!("Update app error: {}", e))?;
        Ok(())
    }

    pub fn delete_app(&self, id: &str, caller_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // 仅允许创建者删除
        let affected = conn.execute(
            "DELETE FROM space_apps WHERE id=?1 AND created_by=?2",
            params![id, caller_id],
        ).map_err(|e| format!("Delete app error: {}", e))?;
        if affected == 0 {
            return Err("无权限删除或应用不存在".to_string());
        }
        Ok(())
    }

    /// 按创建者查询应用（用于权限校验）
    pub fn list_apps_by_created(&self, app_id: &str, created_by: &str) -> Result<Vec<models::AppRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, name, category, icon, protocol, hostname, port, pathname, sort_order, created_by, created_at FROM space_apps WHERE id=?1 AND created_by=?2"
        ).map_err(|e| format!("Query error: {}", e))?;

        let rows = stmt.query_map(params![app_id, created_by], |row| {
            Ok(models::AppRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                name: row.get(2)?,
                category: row.get(3)?,
                icon: row.get(4)?,
                protocol: row.get(5)?,
                hostname: row.get(6)?,
                port: row.get(7)?,
                pathname: row.get(8)?,
                sort_order: row.get(9)?,
                created_by: row.get(10)?,
                created_at: row.get(11)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut apps = Vec::new();
        for row in rows {
            apps.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(apps)
    }

    pub fn list_apps(&self, space_id: &str) -> Result<Vec<models::AppRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, name, category, icon, protocol, hostname, port, pathname, sort_order, created_by, created_at FROM space_apps WHERE space_id=?1 ORDER BY sort_order ASC, created_at ASC"
        ).map_err(|e| format!("Query error: {}", e))?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(models::AppRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                name: row.get(2)?,
                category: row.get(3)?,
                icon: row.get(4)?,
                protocol: row.get(5)?,
                hostname: row.get(6)?,
                port: row.get(7)?,
                pathname: row.get(8)?,
                sort_order: row.get(9)?,
                created_by: row.get(10)?,
                created_at: row.get(11)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut apps = Vec::new();
        for row in rows {
            apps.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(apps)
    }

    // --- Files ---

    pub fn insert_file(&self, file: &models::FileRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO files (id, space_id, sender_id, file_name, file_size, file_hash, mime_type, is_compressed, is_password_protected, storage_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                file.id, file.space_id, file.sender_id, file.file_name, file.file_size,
                file.file_hash, file.mime_type, file.is_compressed, file.is_password_protected,
                file.storage_path, file.created_at,
            ],
        ).map_err(|e| format!("Insert file error: {}", e))?;
        Ok(())
    }

    pub fn get_file(&self, file_id: &str) -> Result<Option<models::FileRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, sender_id, file_name, file_size, file_hash, mime_type, is_compressed, is_password_protected, storage_path, created_at FROM files WHERE id=?1"
        ).map_err(|e| format!("Query error: {}", e))?;

        let result = stmt.query_row(params![file_id], |row| {
            Ok(models::FileRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                sender_id: row.get(2)?,
                file_name: row.get(3)?,
                file_size: row.get(4)?,
                file_hash: row.get(5)?,
                mime_type: row.get(6)?,
                is_compressed: row.get(7)?,
                is_password_protected: row.get(8)?,
                storage_path: row.get(9)?,
                created_at: row.get(10)?,
            })
        });

        match result {
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }

    pub fn list_files(&self, space_id: &str, limit: Option<u32>) -> Result<Vec<models::FileRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let limit_sql = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
        let sql = format!(
            "SELECT id, space_id, sender_id, file_name, file_size, file_hash, mime_type, is_compressed, is_password_protected, storage_path, created_at FROM files WHERE space_id=?1 ORDER BY created_at DESC{}",
            limit_sql
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| format!("Query error: {}", e))?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(models::FileRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                sender_id: row.get(2)?,
                file_name: row.get(3)?,
                file_size: row.get(4)?,
                file_hash: row.get(5)?,
                mime_type: row.get(6)?,
                is_compressed: row.get(7)?,
                is_password_protected: row.get(8)?,
                storage_path: row.get(9)?,
                created_at: row.get(10)?,
            })
        }).map_err(|e| format!("Query error: {}", e))?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(files)
    }

    pub fn delete_file(&self, file_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM files WHERE id=?1", params![file_id])
            .map_err(|e| format!("Delete file error: {}", e))?;
        Ok(())
    }

    // --- ACL Rules ---

    pub fn insert_acl_rule(&self, rule: &models::AclRuleRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO acl_rules (id, space_id, action, source, dest, ports, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rule.id, rule.space_id, rule.action, rule.source, rule.dest, 
                rule.ports, rule.description, rule.created_at, rule.updated_at
            ],
        ).map_err(|e| format!("Insert ACL rule error: {}", e))?;
        Ok(())
    }

    pub fn get_acl_rules(&self, space_id: &str) -> Result<Vec<models::AclRuleRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, action, source, dest, ports, description, created_at, updated_at
             FROM acl_rules WHERE space_id=?1 ORDER BY created_at DESC"
        ).map_err(|e| format!("Prepare ACL rules error: {}", e))?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(models::AclRuleRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                action: row.get(2)?,
                source: row.get(3)?,
                dest: row.get(4)?,
                ports: row.get(5)?,
                description: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        }).map_err(|e| format!("Query ACL rules error: {}", e))?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(rules)
    }

    pub fn update_acl_rule(&self, rule: &models::AclRuleRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE acl_rules SET action=?2, source=?3, dest=?4, ports=?5, description=?6, updated_at=?7 
             WHERE id=?1",
            params![
                rule.id, rule.action, rule.source, rule.dest, rule.ports, 
                rule.description, rule.updated_at
            ],
        ).map_err(|e| format!("Update ACL rule error: {}", e))?;
        Ok(())
    }

    pub fn delete_acl_rule(&self, rule_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM acl_rules WHERE id=?1", params![rule_id])
            .map_err(|e| format!("Delete ACL rule error: {}", e))?;
        Ok(())
    }

    // --- Port Forward Rules ---

    pub fn insert_port_forward_rule(&self, rule: &models::PortForwardRuleRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO port_forward_rules (id, space_id, name, protocol, source_ip, source_port, target_ip, target_port, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                rule.id, rule.space_id, rule.name, rule.protocol, rule.source_ip, 
                rule.source_port, rule.target_ip, rule.target_port, rule.description,
                rule.created_at, rule.updated_at
            ],
        ).map_err(|e| format!("Insert port forward rule error: {}", e))?;
        Ok(())
    }

    pub fn get_port_forward_rules(&self, space_id: &str) -> Result<Vec<models::PortForwardRuleRow>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, space_id, name, protocol, source_ip, source_port, target_ip, target_port, description, created_at, updated_at
             FROM port_forward_rules WHERE space_id=?1 ORDER BY created_at DESC"
        ).map_err(|e| format!("Prepare port forward rules error: {}", e))?;

        let rows = stmt.query_map(params![space_id], |row| {
            Ok(models::PortForwardRuleRow {
                id: row.get(0)?,
                space_id: row.get(1)?,
                name: row.get(2)?,
                protocol: row.get(3)?,
                source_ip: row.get(4)?,
                source_port: row.get(5)?,
                target_ip: row.get(6)?,
                target_port: row.get(7)?,
                description: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        }).map_err(|e| format!("Query port forward rules error: {}", e))?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(rules)
    }

    pub fn update_port_forward_rule(&self, rule: &models::PortForwardRuleRow) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE port_forward_rules SET name=?2, protocol=?3, source_ip=?4, source_port=?5, target_ip=?6, target_port=?7, description=?8, updated_at=?9 
             WHERE id=?1",
            params![
                rule.id, rule.name, rule.protocol, rule.source_ip, rule.source_port,
                rule.target_ip, rule.target_port, rule.description, rule.updated_at
            ],
        ).map_err(|e| format!("Update port forward rule error: {}", e))?;
        Ok(())
    }

    pub fn delete_port_forward_rule(&self, rule_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM port_forward_rules WHERE id=?1", params![rule_id])
            .map_err(|e| format!("Delete port forward rule error: {}", e))?;
        Ok(())
    }
}