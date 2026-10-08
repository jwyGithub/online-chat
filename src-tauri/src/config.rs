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
    /// 上游端点协议：chat（/chat/completions）| responses（/responses）| messages（/messages）
    pub api_protocol: String,
    /// 是否开启思考（reasoning / thinking）
    pub thinking_enabled: bool,
    /// 思考强度：low | medium | high
    pub thinking_effort: String,
    /// 是否启用网络代理（启用后所有上游请求都经代理转发）
    pub proxy_enabled: bool,
    /// 代理协议：http | https | socks5
    pub proxy_type: String,
    /// 代理地址：host:port，可带鉴权（user:pass@host:port）；也可直接写含协议的完整 URL
    pub proxy_url: String,
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
            api_protocol: "chat".into(),
            thinking_enabled: false,
            thinking_effort: "medium".into(),
            proxy_enabled: false,
            proxy_type: "http".into(),
            proxy_url: String::new(),
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
