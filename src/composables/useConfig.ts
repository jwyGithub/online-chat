import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
  base_url: string;
  model: string;
  temperature: number;
  system_prompt: string;
  hotkey: string;
  hide_on_blur: boolean;
}

const defaults: AppConfig = {
  base_url: "https://api.openai.com/v1",
  model: "gpt-4o-mini",
  temperature: 0.7,
  system_prompt: "",
  hotkey: "CmdOrCtrl+Shift+Space",
  hide_on_blur: true,
};

// 模块级单例，跨组件共享
const config = ref<AppConfig>({ ...defaults });
const hasKey = ref(false);

export function useConfig() {
  async function load() {
    config.value = await invoke<AppConfig>("get_config");
    hasKey.value = await invoke<boolean>("has_api_key");
  }

  async function save(next: AppConfig, apiKey?: string) {
    await invoke("save_config", { config: next });
    config.value = { ...next };
    if (apiKey !== undefined) {
      await invoke("set_api_key", { key: apiKey });
      hasKey.value = apiKey.trim().length > 0;
    }
    // 快捷键改动即时生效（失败不阻塞保存）
    try {
      await invoke("update_hotkey", { hotkey: next.hotkey });
    } catch (e) {
      console.warn("更新快捷键失败", e);
    }
  }

  return { config, hasKey, load, save };
}
