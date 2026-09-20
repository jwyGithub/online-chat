<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import Icon from "../components/Icon.vue";
import { useConfig, type AppConfig } from "../composables/useConfig";

const emit = defineEmits<{ saved: [] }>();
const { config, hasKey, load, save } = useConfig();

const form = reactive<AppConfig>({ ...config.value });
const apiKey = ref("");
const saving = ref(false);
const msg = ref("");
const ok = ref(false);

onMounted(async () => {
  await load();
  Object.assign(form, config.value);
});

async function onSave() {
  saving.value = true;
  msg.value = "";
  ok.value = false;
  try {
    await save({ ...form }, apiKey.value.length ? apiKey.value : undefined);
    apiKey.value = "";
    ok.value = true;
    msg.value = "已保存";
    emit("saved");
  } catch (e) {
    ok.value = false;
    msg.value = "保存失败: " + String(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="flex flex-1 flex-col min-h-0 w-full">
    <div class="flex flex-1 min-h-0 flex-col gap-5 overflow-y-auto p-4">
      <section class="flex flex-col gap-3.5">
        <h3 class="m-0 text-[11px] font-semibold uppercase tracking-[0.8px] text-fg-3">模型连接</h3>

        <label class="flex flex-col gap-1.75">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3"><Icon name="link" :size="15" /> API 地址 (Base URL)</span>
          <input v-model="form.base_url" placeholder="https://api.openai.com/v1" />
          <small class="text-[11px] text-fg-3">OpenAI 兼容接口，末尾无需 /chat/completions</small>
        </label>

        <label class="flex flex-col gap-1.75">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3"><Icon name="key" :size="15" /> API Key</span>
          <input
            v-model="apiKey"
            type="password"
            :placeholder="hasKey ? '已配置（留空则不改动）' : '请输入 API Key'"
          />
          <small class="text-[11px] text-fg-3">存储于程序本地配置目录，与其它配置分开保存</small>
        </label>

        <label class="flex flex-col gap-1.75">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3"><Icon name="cpu" :size="15" /> 模型 (Model)</span>
          <input v-model="form.model" placeholder="gpt-4o-mini" />
        </label>
      </section>

      <section class="flex flex-col gap-3.5">
        <h3 class="m-0 text-[11px] font-semibold uppercase tracking-[0.8px] text-fg-3">对话行为</h3>

        <label class="flex flex-col gap-1.75">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3">
            <Icon name="thermometer" :size="15" /> 温度 (Temperature)
            <span class="ml-auto rounded-full border border-line bg-accent-soft px-2.25 py-px text-xs font-semibold text-accent-2">{{ form.temperature.toFixed(1) }}</span>
          </span>
          <input
            v-model.number="form.temperature"
            class="slider"
            type="range"
            min="0"
            max="2"
            step="0.1"
            :style="{ '--val': form.temperature / 2 }"
          />
          <small class="text-[11px] text-fg-3">值越高越发散，越低越确定</small>
        </label>

        <label class="flex flex-col gap-1.75">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3"><Icon name="message" :size="15" /> System Prompt（可选）</span>
          <textarea
            v-model="form.system_prompt"
            rows="3"
            placeholder="你是一个乐于助人的助手…"
            class="min-h-15.5 resize-y"
          ></textarea>
        </label>

        <label class="flex flex-col gap-1.75">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3"><Icon name="command" :size="15" /> 全局快捷键</span>
          <input v-model="form.hotkey" placeholder="CmdOrCtrl+Shift+Space" />
          <small class="text-[11px] text-fg-3">示例：CmdOrCtrl+Shift+Space、Alt+Space</small>
        </label>

        <label class="flex cursor-pointer items-center justify-between gap-3 rounded-md border border-line bg-surface-2 px-3.5 py-3">
          <span class="inline-flex items-center gap-1.75 text-[13px] font-medium text-fg-2 [&_svg]:text-fg-3"><Icon name="eye" :size="15" /> 失焦时自动隐藏窗口</span>
          <span class="relative inline-flex h-6 w-10.5 shrink-0">
            <input v-model="form.hide_on_blur" type="checkbox" class="peer sr-only" />
            <span
              class="absolute inset-0 rounded-full border border-line bg-surface-3 transition-colors duration-200 peer-checked:border-transparent peer-checked:bg-grad-accent peer-focus-visible:shadow-[0_0_0_3px_rgba(34,211,238,0.18)]"
            ></span>
            <span
              class="pointer-events-none absolute left-0.5 top-1/2 h-4.5 w-4.5 -translate-y-1/2 rounded-full bg-white shadow-[0_1px_4px_rgba(0,0,0,0.35)] transition-transform duration-200 peer-checked:translate-x-4.5"
            ></span>
          </span>
        </label>
      </section>
    </div>

    <div class="flex flex-none items-center justify-end gap-3 border-t border-hairline bg-bg-2 px-4 py-3">
      <Transition name="fade">
        <span
          v-if="msg"
          class="inline-flex items-center gap-1.25 text-[12.5px]"
          :class="ok ? 'text-success' : 'text-fg-2'"
        >
          <Icon v-if="ok" name="check" :size="14" />
          {{ msg }}
        </span>
      </Transition>
      <button
        class="btn btn-primary"
        :disabled="saving"
        @click="onSave"
      >
        <Icon name="check" :size="16" />
        {{ saving ? "保存中…" : "保存" }}
      </button>
    </div>
  </div>
</template>

<style scoped>
/* ---------- 滑块（range 需伪元素，无法用工具类表达） ---------- */
.slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 6px;
  padding: 0;
  border: none;
  border-radius: 999px;
  background: linear-gradient(
    to right,
    var(--color-accent) 0%,
    var(--color-accent-2) calc(var(--val, 0.35) * 100%),
    var(--color-surface-3) calc(var(--val, 0.35) * 100%)
  );
  cursor: pointer;
}
.slider:focus {
  outline: none;
  box-shadow: none;
}
.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  border: 3px solid var(--color-accent);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  transition: transform 0.12s var(--ease);
}
.slider::-webkit-slider-thumb:hover {
  transform: scale(1.12);
}
.slider::-moz-range-thumb {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: #fff;
  border: 3px solid var(--color-accent);
}

/* ---------- 状态提示淡入淡出（Vue 过渡类） ---------- */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s var(--ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
