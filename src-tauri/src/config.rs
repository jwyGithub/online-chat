use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 应用配置（不含 API Key，Key 存本地配置目录下的独立文件）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
    pub system_prompt: String,
    pub hotkey: String,
    pub hide_on_blur: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            temperature: 0.7,
            system_prompt: String::new(),
            hotkey: "CmdOrCtrl+Shift+Space".into(),
            hide_on_blur: true,
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

/// 读取配置，出错时回退到默认值
pub fn read_config(app: &AppHandle) -> Config {
    match config_path(app).ok().and_then(|p| fs::read_to_string(p).ok()) {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => Config::default(),
    }
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Config {
    read_config(&app)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: Config) -> Result<(), String> {
    let path = config_path(&app)?;
    let s = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())?;
    Ok(())
}
