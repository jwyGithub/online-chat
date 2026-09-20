//! OpenAI 兼容协议的流式对话。请求在 Rust 端发起（无 CORS、Key 不进前端），
//! 使用 async-openai 构建请求并解析 SSE，通过事件把增量 token 推给前端渲染。

use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
    CreateChatCompletionRequestArgs,
};
use async_openai::Client;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::config::read_config;
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

    // 组装消息，附加 system prompt。前端只区分 user/assistant，其余按 user 处理。
    let mut msgs: Vec<ChatCompletionRequestMessage> = Vec::new();
    if !cfg.system_prompt.trim().is_empty() {
        msgs.push(ChatCompletionRequestSystemMessage::from(cfg.system_prompt.as_str()).into());
    }
    for m in &messages {
        let msg: ChatCompletionRequestMessage = match m.role.as_str() {
            "assistant" => ChatCompletionRequestAssistantMessage::from(m.content.as_str()).into(),
            _ => ChatCompletionRequestUserMessage::from(m.content.as_str()).into(),
        };
        msgs.push(msg);
    }

    // Key 与自定义 base_url 仍留在 Rust 端。async-openai 的 URL 为裸拼接，
    // 需去掉 base_url 末尾的 '/' 以免出现双斜杠。
    let openai_cfg = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(cfg.base_url.trim_end_matches('/'));
    let client = Client::with_config(openai_cfg);

    let request = CreateChatCompletionRequestArgs::default()
        .model(cfg.model)
        .temperature(cfg.temperature)
        .stream(true)
        .messages(msgs)
        .build()
        .map_err(|e| format!("构建请求失败: {e}"))?;

    let mut stream = client
        .chat()
        .create_stream(request)
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;

    while let Some(item) = stream.next().await {
        // 用户点击“停止”：立即结束并发出取消事件。
        if cancel.load(Ordering::SeqCst) {
            let _ = app.emit("chat-cancelled", ());
            return Ok(());
        }

        let resp = item.map_err(|e| format!("读取流失败: {e}"))?;
        for choice in resp.choices {
            if let Some(delta) = choice.delta.content {
                if !delta.is_empty() {
                    let _ = app.emit("chat-token", delta);
                }
            }
        }
    }

    let _ = app.emit("chat-done", ());
    Ok(())
}
