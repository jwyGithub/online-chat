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
    // 启用代理时，所有上游请求都经代理转发（支持 http / https / socks5）。
    let mut client_builder = reqwest::Client::builder();
    if cfg.proxy_enabled {
        if let Some(proxy_url) = build_proxy_url(&cfg.proxy_type, &cfg.proxy_url) {
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| format!("代理配置无效（{proxy_url}）: {e}"))?;
            client_builder = client_builder.proxy(proxy);
        }
    }
    let client = client_builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

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

/// 拉取可用模型列表：GET {base_url}/models（OpenAI 兼容，Anthropic 亦适用）。
/// 与对话请求共用代理与鉴权逻辑，请求在 Rust 端发起（Key 不进前端、无 CORS）。
/// 配置与 api_key 可由设置页临时传入（尚未保存时也能预览列表）；
/// 未传入时回退到已保存的配置与 Key。返回去重排序后的模型 id 列表。
#[tauri::command]
pub async fn list_models(
    app: AppHandle,
    config: Option<Config>,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let saved_cfg = read_config(&app);
    let cfg = config.unwrap_or_else(|| saved_cfg.clone());

    let base = if cfg.base_url.trim().is_empty() {
        saved_cfg.base_url.trim()
    } else {
        cfg.base_url.trim()
    };
    let key = api_key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| read_api_key(&app));
    if key.is_empty() {
        return Err("未配置 API Key，请先填写".into());
    }

    let url = format!("{}/models", base.trim_end_matches('/'));

    // 复用对话请求的代理设置（启用时所有上游请求都经代理转发）。
    let mut client_builder = reqwest::Client::builder();
    if cfg.proxy_enabled {
        if let Some(proxy_url) = build_proxy_url(&cfg.proxy_type, &cfg.proxy_url) {
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| format!("代理配置无效（{proxy_url}）: {e}"))?;
            client_builder = client_builder.proxy(proxy);
        }
    }
    let client = client_builder
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let mut builder = client
        .get(&url)
        .header("Authorization", format!("Bearer {key}"));

    // Anthropic 的 /models 用 x-api-key + anthropic-version 鉴权，一并带上以兼容。
    if normalize_protocol(&cfg.api_protocol) == "messages" {
        builder = builder
            .header("x-api-key", key.clone())
            .header("anthropic-version", "2023-06-01");
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "获取模型失败（{status}）: {}",
            truncate(&text, 500)
        ));
    }

    let v: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析模型列表失败: {e}"))?;

    // 兼容两种返回形态：{ "data": [ { "id": ... } ] }（OpenAI / Anthropic）
    // 或直接是数组（元素为对象或字符串）。
    let arr = v
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| v.as_array());
    let mut ids: Vec<String> = match arr {
        Some(items) => items
            .iter()
            .filter_map(|m| {
                m.get("id")
                    .and_then(|x| x.as_str())
                    .or_else(|| m.as_str())
                    .map(|s| s.to_string())
            })
            .collect(),
        None => Vec::new(),
    };

    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// 组装代理 URL。地址为空时返回 None（视为未配置，直连）。
/// 若用户已在地址里写了协议（含 "://"）则原样使用；否则按 proxy_type 补全协议。
/// proxy_type 未知时回退到 http。地址可带鉴权，如 user:pass@host:port。
fn build_proxy_url(proxy_type: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.contains("://") {
        return Some(raw.to_string());
    }
    let scheme = match proxy_type {
        "https" => "https",
        "socks5" => "socks5",
        _ => "http",
    };
    Some(format!("{scheme}://{raw}"))
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
                    let role = if m.role == "assistant" {
                        "assistant"
                    } else {
                        "user"
                    };
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
                    let role = if m.role == "assistant" {
                        "assistant"
                    } else {
                        "user"
                    };
                    json!({ "role": role, "content": m.content })
                })
                .collect();
            let budget = effort_budget(effort);
            let max_tokens = if cfg.thinking_enabled {
                budget + 4096
            } else {
                4096
            };
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
                let role = if m.role == "assistant" {
                    "assistant"
                } else {
                    "user"
                };
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
            let done = process_frame(app, protocol, &frame[..end])?;
            if done {
                return Ok(());
            }
        }
    }

    // 处理流结束时缓冲区里可能残留的最后一帧（无结尾空行）。
    if !buf.is_empty() {
        let _ = process_frame(app, protocol, &buf)?;
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
fn process_frame(app: &AppHandle, protocol: &str, frame: &[u8]) -> Result<bool, String> {
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
        return Ok(false);
    }
    let data = data_lines.join("\n");
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(true);
    }

    match protocol {
        "responses" => match parse_responses_event(&event_name, data)? {
            Some(ResponsesEvent::Token(text)) => {
                let _ = app.emit("chat-token", text);
            }
            Some(ResponsesEvent::Reasoning(text)) => {
                let _ = app.emit("chat-reasoning", text);
            }
            Some(ResponsesEvent::Done) => return Ok(true),
            None => {}
        },
        "messages" => {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return Ok(false),
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
                "message_stop" => return Ok(true),
                _ => {}
            }
        }
        // chat/completions
        _ => {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return Ok(false),
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
    Ok(false)
}

#[derive(Debug, PartialEq, Eq)]
enum ResponsesEvent {
    Token(String),
    Reasoning(String),
    Done,
}

fn parse_responses_event(event_name: &str, data: &str) -> Result<Option<ResponsesEvent>, String> {
    let v: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    match event_name {
        "response.output_text.delta" => Ok(v
            .get("delta")
            .and_then(Value::as_str)
            .map(|s| ResponsesEvent::Token(s.to_string()))),
        "response.reasoning_summary_text.delta" => Ok(v
            .get("delta")
            .and_then(Value::as_str)
            .map(|s| ResponsesEvent::Reasoning(s.to_string()))),
        "response.completed" | "response.done" => Ok(Some(ResponsesEvent::Done)),
        "response.failed" => Err(v
            .pointer("/response/error/message")
            .or_else(|| v.pointer("/error/message"))
            .and_then(Value::as_str)
            .unwrap_or("Responses API 返回失败事件")
            .to_string()),
        _ => Ok(None),
    }
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

#[cfg(test)]
mod tests {
    use super::{find_frame_end, parse_responses_event, ResponsesEvent};

    #[test]
    fn responses_failed_event_returns_upstream_error() {
        let error = parse_responses_event(
            "response.failed",
            r#"{"response":{"error":{"message":"模型暂时不可用"}}}"#,
        )
        .unwrap_err();

        assert_eq!(error, "模型暂时不可用");
    }

    #[test]
    fn responses_completed_event_is_successful_terminal_event() {
        assert!(matches!(
            parse_responses_event("response.completed", r#"{"type":"response.completed"}"#)
                .unwrap(),
            Some(ResponsesEvent::Done)
        ));
    }

    #[test]
    fn sse_frame_end_accepts_lf_and_crlf() {
        assert_eq!(find_frame_end(b"data: x\n\nrest"), Some((7, 2)));
        assert_eq!(find_frame_end(b"data: x\r\n\r\nrest"), Some((7, 4)));
    }

    #[test]
    fn responses_text_delta_is_forwarded_as_token() {
        assert_eq!(
            parse_responses_event("response.output_text.delta", r#"{"delta":"hello"}"#).unwrap(),
            Some(ResponsesEvent::Token("hello".into()))
        );
    }

    #[test]
    fn request_bodies_keep_protocol_specific_fields() {
        use super::{build_body, ChatMessage};
        use crate::config::Config;

        let mut config = Config::default();
        config.system_prompt = "system".into();
        let messages = [ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let chat = build_body(&config, "chat", &messages);
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][1]["content"], "hello");

        let responses = build_body(&config, "responses", &messages);
        assert_eq!(responses["input"][0]["role"], "user");
        assert!(responses.get("messages").is_none());

        let messages_api = build_body(&config, "messages", &messages);
        assert_eq!(messages_api["messages"][0]["content"], "hello");
        assert_eq!(messages_api["max_tokens"], 4096);
    }
}
