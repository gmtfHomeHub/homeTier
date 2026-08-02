pub const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    config_json TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS spaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    owner_id TEXT,
    network_name TEXT NOT NULL,
    network_secret TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    last_connected_at TEXT,
    is_auto_connect INTEGER DEFAULT 0,
    config_json TEXT
);

CREATE TABLE IF NOT EXISTS members (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    nickname TEXT NOT NULL,
    virtual_ip TEXT,
    is_online INTEGER DEFAULT 0,
    is_owner INTEGER DEFAULT 0,
    joined_at TEXT NOT NULL,
    last_seen_at TEXT,
    is_favorite INTEGER DEFAULT 0,
    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    sender_name TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'text',
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    status TEXT DEFAULT 'sent',
    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_messages_space ON messages(space_id, timestamp);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_hash TEXT,
    mime_type TEXT,
    is_compressed INTEGER DEFAULT 0,
    is_password_protected INTEGER DEFAULT 0,
    storage_path TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS space_apps (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    name TEXT NOT NULL,
    category TEXT DEFAULT '',
    icon TEXT DEFAULT '',
    protocol TEXT DEFAULT 'http:',
    hostname TEXT DEFAULT '',
    port TEXT DEFAULT '',
    pathname TEXT DEFAULT '',
    sort_order INTEGER DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS acl_rules (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('allow', 'deny')),
    source TEXT NOT NULL,
    dest TEXT NOT NULL,
    ports TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS port_forward_rules (
    id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    name TEXT NOT NULL,
    protocol TEXT NOT NULL CHECK (protocol IN ('tcp', 'udp')),
    source_ip TEXT NOT NULL,
    source_port INTEGER NOT NULL,
    target_ip TEXT NOT NULL,
    target_port INTEGER NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
);
";

// 兼容性迁移：对已存在的旧数据库添加新列（幂等执行）
pub const SCHEMA_MIGRATIONS: &[&str] = &[
    "ALTER TABLE spaces ADD COLUMN owner_id TEXT",
    "ALTER TABLE members ADD COLUMN is_owner INTEGER DEFAULT 0",
    "CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        config_json TEXT,
        updated_at TEXT NOT NULL
    )",
    "ALTER TABLE spaces DROP COLUMN local_config_json",
];
