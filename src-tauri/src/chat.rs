//! 流式对话。请求在 Rust 端发起（无 CORS、Key 不进前端），用 reqwest 直接发 HTTP
//! 并手工解析 SSE。支持三种上游协议：
//!   - chat      → POST {base}/chat/completions（OpenAI Chat Completions）
//!   - responses → POST {base}/responses（OpenAI Responses）
//!   - messages  → POST {base}/messages（Anthropic Messages）
//! 正文增量以 `chat-token` 事件推送；思考（reasoning / thinking）增量以
//! `chat-reasoning` 事件推送，前端分区渲染。

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::config::{read_config, Config};
use crate::secret::read_api_key;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// 全局取消标记：新对话开始时复位，点击“停止”时置位。
#[derive(Default)]
pub struct CancelState(pub Arc<AtomicBool>);

#[tauri::command]
pub fn cancel_chat(state: tauri::State<'_, CancelState>) {
    state.0.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn send_chat(
    app: AppHandle,
    state: tauri::State<'_, CancelState>,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let cancel = state.0.clone();
    cancel.store(false, Ordering::SeqCst);

    let cfg = read_config(&app);
    let api_key = read_api_key(&app);
    if api_key.is_empty() {
        return Err("未配置 API Key，请在设置里填写".into());
    }

    let protocol = normalize_protocol(&cfg.api_protocol);
    let url = build_url(&cfg.base_url, protocol);
    let body = build_body(&cfg, protocol, &messages);

    // Key 与自定义 base_url 仍留在 Rust 端。
    let client = reqwest::Client::new();
    let mut builder = client
        .post(&url)
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body);

    // Anthropic Messages 端点通常用 x-api-key + anthropic-version 鉴权，一并带上以兼容。
    if protocol == "messages" {
        builder = builder
            .header("x-api-key", api_key.clone())
            .header("anthropic-version", "2023-06-01");
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("请求失败（{status}）: {}", truncate(&text, 500)));
    }

    stream_sse(&app, &cancel, protocol, resp).await?;

    // 被用户取消时 stream_sse 已发出 chat-cancelled；正常结束才发 chat-done。
    if !cancel.load(Ordering::SeqCst) {
        let _ = app.emit("chat-done", ());
    }
    Ok(())
}

/// 归一化协议名，未知值回退到 chat。
fn normalize_protocol(p: &str) -> &'static str {
    match p {
        "responses" => "responses",
        "messages" => "messages",
        _ => "chat",
    }
}

/// 拼接端点 URL。base_url 末尾的 '/' 去掉，避免双斜杠。
fn build_url(base: &str, protocol: &str) -> String {
    let b = base.trim_end_matches('/');
    match protocol {
        "responses" => format!("{b}/responses"),
        "messages" => format!("{b}/messages"),
        _ => format!("{b}/chat/completions"),
    }
}

/// 思考强度映射到 Anthropic thinking.budget_tokens。
fn effort_budget(effort: &str) -> u32 {
    match effort {
        "low" => 2048,
        "high" => 24576,
        _ => 8192,
    }
}

/// 按协议构造请求体。前端只区分 user/assistant，其余按 user 处理。
fn build_body(cfg: &Config, protocol: &str, messages: &[ChatMessage]) -> Value {
    let effort = cfg.thinking_effort.as_str();
    let system = cfg.system_prompt.trim();

    match protocol {
        // OpenAI Responses：system 走 instructions，历史走 input 数组。
        "responses" => {
            let input: Vec<Value> = messages
                .iter()
                .map(|m| {
                    let role = if m.role == "assistant" { "assistant" } else { "user" };
                    json!({
                        "type": "message",
                        "role": role,
                        "content": [{ "type": "input_text", "text": m.content }],
                    })
                })
                .collect();
            let mut body = json!({
                "model": cfg.model,
                "input": input,
                "stream": true,
            });
            if !system.is_empty() {
                body["instructions"] = json!(system);
            }
            if cfg.thinking_enabled {
                body["reasoning"] = json!({ "effort": effort });
            }
            body
        }
        // Anthropic Messages：system 顶层字段，必须带 max_tokens。
        "messages" => {
            let msgs: Vec<Value> = messages
                .iter()
                .map(|m| {
                    let role = if m.role == "assistant" { "assistant" } else { "user" };
                    json!({ "role": role, "content": m.content })
                })
                .collect();
            let budget = effort_budget(effort);
            let max_tokens = if cfg.thinking_enabled { budget + 4096 } else { 4096 };
            let mut body = json!({
                "model": cfg.model,
                "messages": msgs,
                "max_tokens": max_tokens,
                "stream": true,
            });
            if !system.is_empty() {
                body["system"] = json!(system);
            }
            if cfg.thinking_enabled {
                // 开思考时不带 temperature（部分模型要求 temperature=1，交由上游默认）。
                body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
            } else {
                body["temperature"] = json!(cfg.temperature);
            }
            body
        }
        // OpenAI Chat Completions（默认）。
        _ => {
            let mut msgs: Vec<Value> = Vec::new();
            if !system.is_empty() {
                msgs.push(json!({ "role": "system", "content": system }));
            }
            for m in messages {
                let role = if m.role == "assistant" { "assistant" } else { "user" };
                msgs.push(json!({ "role": role, "content": m.content }));
            }
            let mut body = json!({
                "model": cfg.model,
                "messages": msgs,
                "temperature": cfg.temperature,
                "stream": true,
            });
            if cfg.thinking_enabled {
                // 该网关按 output_config.effort 读取思考强度。
                body["output_config"] = json!({ "effort": effort });
            }
            body
        }
    }
}

/// 读取 SSE 字节流，按帧解析并把增量推给前端。返回 Ok 表示流已结束（正常或被取消）。
async fn stream_sse(
    app: &AppHandle,
    cancel: &Arc<AtomicBool>,
    protocol: &str,
    resp: reqwest::Response,
) -> Result<(), String> {
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();

    while let Some(chunk) = stream.next().await {
        // 用户点击“停止”：立即结束并发出取消事件。
        if cancel.load(Ordering::SeqCst) {
            let _ = app.emit("chat-cancelled", ());
            return Ok(());
        }

        let bytes = chunk.map_err(|e| format!("读取流失败: {e}"))?;
        buf.extend_from_slice(&bytes);

        // 帧分隔符是空行（\n\n 或 \r\n\r\n），均为 ASCII，切帧不会截断多字节字符。
        while let Some((end, sep_len)) = find_frame_end(&buf) {
            let frame: Vec<u8> = buf.drain(..end + sep_len).collect();
            let done = process_frame(app, protocol, &frame[..end]);
            if done {
                return Ok(());
            }
        }
    }

    // 处理流结束时缓冲区里可能残留的最后一帧（无结尾空行）。
    if !buf.is_empty() {
        let _ = process_frame(app, protocol, &buf);
    }
    Ok(())
}

/// 找到第一个 SSE 帧分隔符，返回（分隔符起始下标, 分隔符长度）。
fn find_frame_end(buf: &[u8]) -> Option<(usize, usize)> {
    let n = buf.len();
    let mut i = 0;
    while i + 1 < n {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some((i, 2));
        }
        if buf[i] == b'\r'
            && i + 3 < n
            && buf[i + 1] == b'\n'
            && buf[i + 2] == b'\r'
            && buf[i + 3] == b'\n'
        {
            return Some((i, 4));
        }
        i += 1;
    }
    None
}

/// 解析单帧 SSE，按协议分发增量。返回 true 表示上游已明确结束（[DONE] / 完成事件）。
fn process_frame(app: &AppHandle, protocol: &str, frame: &[u8]) -> bool {
    let text = String::from_utf8_lossy(frame);
    let mut event_name = String::new();
    let mut data_lines: Vec<&str> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }

    if data_lines.is_empty() {
        return false;
    }
    let data = data_lines.join("\n");
    let data = data.trim();
    if data == "[DONE]" {
        return true;
    }

    match protocol {
        "responses" => {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return false,
            };
            match event_name.as_str() {
                "response.output_text.delta" => emit_str(app, "chat-token", v.get("delta")),
                "response.reasoning_summary_text.delta" => {
                    emit_str(app, "chat-reasoning", v.get("delta"))
                }
                "response.completed" | "response.done" | "response.failed" => return true,
                _ => {}
            }
        }
        "messages" => {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return false,
            };
            match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                "content_block_delta" => {
                    if let Some(delta) = v.get("delta") {
                        match delta.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                            "text_delta" => emit_str(app, "chat-token", delta.get("text")),
                            "thinking_delta" => {
                                emit_str(app, "chat-reasoning", delta.get("thinking"))
                            }
                            _ => {}
                        }
                    }
                }
                "message_stop" => return true,
                _ => {}
            }
        }
        // chat/completions
        _ => {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
                for choice in choices {
                    if let Some(delta) = choice.get("delta") {
                        // 思考先于正文推送，保持顺序。
                        emit_str(app, "chat-reasoning", delta.get("reasoning_content"));
                        emit_str(app, "chat-token", delta.get("content"));
                    }
                }
            }
        }
    }
    false
}

/// 若 value 是非空字符串，则以指定事件名推送。
fn emit_str(app: &AppHandle, event: &str, value: Option<&Value>) {
    if let Some(s) = value.and_then(|v| v.as_str()) {
        if !s.is_empty() {
            let _ = app.emit(event, s.to_string());
        }
    }
}

/// 按字符数截断（避免把多字节字符切一半），用于错误信息展示。
fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}
