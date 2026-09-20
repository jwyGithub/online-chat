import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Msg {
  role: "user" | "assistant" | "system";
  content: string;
  /** 思考 / 推理内容（reasoning_content / thinking），流式累积 */
  reasoning?: string;
}

const messages = ref<Msg[]>([]);
const streaming = ref(false);
const error = ref("");

let unlisteners: UnlistenFn[] = [];

async function ensureListeners() {
  if (unlisteners.length) return;
  unlisteners.push(
    await listen<string>("chat-token", (e) => {
      const last = messages.value[messages.value.length - 1];
      if (last && last.role === "assistant") last.content += e.payload;
    })
  );
  unlisteners.push(
    await listen<string>("chat-reasoning", (e) => {
      const last = messages.value[messages.value.length - 1];
      if (last && last.role === "assistant")
        last.reasoning = (last.reasoning ?? "") + e.payload;
    })
  );
  unlisteners.push(
    await listen("chat-done", () => {
      streaming.value = false;
    })
  );
  unlisteners.push(
    await listen("chat-cancelled", () => {
      streaming.value = false;
    })
  );
}

export function useChat() {
  async function send(text: string) {
    const content = text.trim();
    if (!content || streaming.value) return;
    error.value = "";
    await ensureListeners();

    messages.value.push({ role: "user", content });
    // 发送前快照（不含即将插入的空 assistant 占位）
    const payload = messages.value.map((m) => ({ role: m.role, content: m.content }));
    messages.value.push({ role: "assistant", content: "" });
    streaming.value = true;

    try {
      await invoke("send_chat", { messages: payload });
    } catch (e) {
      const last = messages.value[messages.value.length - 1];
      if (last && last.role === "assistant" && !last.content) messages.value.pop();
      error.value = String(e);
    } finally {
      streaming.value = false;
    }
  }

  async function stop() {
    try {
      await invoke("cancel_chat");
    } finally {
      streaming.value = false;
    }
  }

  function clear() {
    messages.value = [];
    error.value = "";
  }

  return { messages, streaming, error, send, stop, clear };
}
