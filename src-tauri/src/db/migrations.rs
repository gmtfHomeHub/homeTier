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

CREATE TABLE IF NOT EXISTS proxy_cookies (
    host_key TEXT NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    path TEXT NOT NULL DEFAULT '/',
    domain TEXT,
    expires_at INTEGER,
    http_only INTEGER NOT NULL DEFAULT 0,
    secure INTEGER NOT NULL DEFAULT 0,
    same_site TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (host_key, name, path)
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

