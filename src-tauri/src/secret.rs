//! API Key 存取：保存在应用本地配置目录下的独立文件中（不再使用系统钥匙串）。
//! 与 config.json 分开存放，避免保存设置时被整体覆盖。
//! 出于安全考虑，不向前端暴露读取明文的命令，仅 Rust 内部 read_api_key 使用。

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Key 文件路径：<app_config_dir>/api_key，并确保目录存在。
fn key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("api_key"))
}

/// 供后端对话时读取，前端无法调用
pub fn read_api_key(app: &AppHandle) -> String {
    key_path(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// 前端保存 Key；传空字符串表示删除
#[tauri::command]
pub fn set_api_key(app: AppHandle, key: String) -> Result<(), String> {
    let path = key_path(&app)?;
    if key.trim().is_empty() {
        match fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        fs::write(&path, key.trim()).map_err(|e| e.to_string())
    }
}

/// 前端用于判断是否已配置 Key（不返回明文）
#[tauri::command]
pub fn has_api_key(app: AppHandle) -> bool {
    !read_api_key(&app).is_empty()
}
