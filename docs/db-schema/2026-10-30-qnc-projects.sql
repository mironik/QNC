-- schema of data/qnc-projects.db

-- table app_settings: 1 rows
CREATE TABLE app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

-- table local_runtime_settings: 2 rows
CREATE TABLE local_runtime_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT
        );

-- table module_state: 0 rows
CREATE TABLE module_state (
            module_id TEXT PRIMARY KEY,
            enabled INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT
        );

-- table project_origin: 1 rows
CREATE TABLE project_origin (
            project_id TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            identity_json TEXT NOT NULL CHECK(json_valid(identity_json))
        );

-- table project_search: 1 rows
CREATE TABLE project_search (
            project_id TEXT PRIMARY KEY,
            search_text TEXT NOT NULL
        );

-- table project_storage_locations: 1 rows
CREATE TABLE project_storage_locations (
            project_id TEXT PRIMARY KEY,
            local_path TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT
        );

-- table project_template_kv: 0 rows
CREATE TABLE project_template_kv (
            template_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (template_id, setting_key)
        );

-- table project_template_sources: 0 rows
CREATE TABLE project_template_sources (
            template_id TEXT NOT NULL,
            source_template_id TEXT NOT NULL,
            PRIMARY KEY (template_id, source_template_id)
        );

-- table project_templates: 10 rows
CREATE TABLE project_templates (
            template_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            system INTEGER NOT NULL DEFAULT 0,
            settings_json TEXT NOT NULL DEFAULT '{}',
            source_template_ids_json TEXT NOT NULL DEFAULT '[]',
            created_by TEXT,
            updated_by TEXT,
            created_at TEXT,
            updated_at TEXT
        );

-- table projects: 1 rows
CREATE TABLE projects (
            project_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            project_uri TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT,
            created_by TEXT,
            updated_by TEXT,
            last_opened_at TEXT
        );

-- table sessions: 0 rows
CREATE TABLE sessions (
            session_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            station_id TEXT NOT NULL,
            client_label TEXT NOT NULL DEFAULT '',
            created_at TEXT,
            last_seen_at TEXT,
            FOREIGN KEY(user_id) REFERENCES users(user_id)
        );

-- table source_template_kv: 0 rows
CREATE TABLE source_template_kv (
            source_template_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (source_template_id, setting_key)
        );

-- table source_templates: 4 rows
CREATE TABLE source_templates (
            source_template_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            source_kind TEXT NOT NULL DEFAULT 'local',
            system INTEGER NOT NULL DEFAULT 0,
            config_json TEXT NOT NULL DEFAULT '{}',
            created_by TEXT,
            updated_by TEXT,
            created_at TEXT,
            updated_at TEXT
        );

-- table users: 0 rows
CREATE TABLE users (
            user_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'editor',
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT,
            updated_at TEXT
        );
CREATE VIEW public_app_settings AS
            SELECT key, value FROM app_settings;
CREATE VIEW public_module_state AS
            SELECT module_id, enabled, updated_at FROM module_state;
CREATE VIEW public_project_origin AS
            SELECT project_id, project_name, created_at,
                json_extract(identity_json, '$.contract_version') AS identity_contract_version,
                json_extract(identity_json, '$.workstation_name') AS workstation_name,
                json_extract(identity_json, '$.user_name') AS user_name,
                json_extract(identity_json, '$.hardware_serial.value') AS serial_number,
                json_extract(identity_json, '$.hardware_serial.kind') AS serial_kind,
                json_extract(identity_json, '$.hardware_serial.source') AS serial_source,
                identity_json
            FROM project_origin;
CREATE VIEW public_project_templates AS
            SELECT template_id, name, description, system, created_at, updated_at
            FROM project_templates;
CREATE VIEW public_projects AS
            SELECT project_id, name, project_uri, created_at, updated_at, last_opened_at
            FROM projects;
CREATE VIEW public_source_templates AS
            SELECT source_template_id, name, description, source_kind, system, created_at, updated_at
            FROM source_templates;
