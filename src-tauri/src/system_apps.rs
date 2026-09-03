use serde::{Deserialize, Serialize};
use std::path::Path;

const SYSTEM_APPS_FILE: &str = "system_apps.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemApp {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn builtin_system_apps() -> Vec<SystemApp> {
    vec![
        SystemApp {
            name: "appNav.systemChat".to_string(),
            path: "/chat".to_string(),
            icon: Some("lucide:message-square".to_string()),
            desc: None,
            enabled: true,
        },
        SystemApp {
            name: "appNav.systemVoice".to_string(),
            path: "/voice".to_string(),
            icon: Some("lucide:mic".to_string()),
            desc: None,
            enabled: true,
        },
        SystemApp {
            name: "appNav.systemScreen".to_string(),
            path: "/screen".to_string(),
            icon: Some("lucide:monitor-up".to_string()),
            desc: None,
            enabled: true,
        },
        SystemApp {
            name: "appNav.systemFiles".to_string(),
            path: "/files".to_string(),
            icon: Some("lucide:folder-open".to_string()),
            desc: None,
            enabled: true,
        },
    ]
}

pub fn load_system_apps(data_dir: &Path) -> Vec<SystemApp> {
    let path = data_dir.join(SYSTEM_APPS_FILE);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return builtin_system_apps();
    };
    #[derive(Deserialize)]
    struct File {
        #[serde(default)]
        apps: Vec<SystemApp>,
    }
    serde_json::from_str::<File>(&content)
        .map(|f| f.apps)
        .unwrap_or_else(|_| builtin_system_apps())
}
