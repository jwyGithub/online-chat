<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ChatView from "./views/ChatView.vue";
import SettingsView from "./views/SettingsView.vue";
import Icon from "./components/Icon.vue";
import { useConfig } from "./composables/useConfig";
import { enterFade, leaveFade } from "./composables/useMotion";

const view = ref<"chat" | "settings">("chat");
const { load, hasKey } = useConfig();

let unlisten: UnlistenFn | null = null;

async function hideWindow() {
  await getCurrentWindow().hide();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") hideWindow();
}

onMounted(async () => {
  await load();
  // 没配置 Key 时首次直接进设置
  if (!hasKey.value) view.value = "settings";

  unlisten = await listen<string>("navigate", (e) => {
    view.value = e.payload === "settings" ? "settings" : "chat";
  });

  window.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
  unlisten?.();
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div class="app">
    <header class="titlebar" data-tauri-drag-region>
      <span class="brand" data-tauri-drag-region>
        <span class="brand-badge"><Icon name="sparkles" :size="15" /></span>
        <span class="brand-name">QuickChat</span>
      </span>
      <div class="flex gap-1">
        <button
          v-if="view === 'chat'"
          class="icon-btn"
          title="设置"
          @click="view = 'settings'"
        >
          <Icon name="settings" :size="17" />
        </button>
        <button v-else class="icon-btn" title="返回对话" @click="view = 'chat'">
          <Icon name="back" :size="17" />
        </button>
        <button class="icon-btn" title="隐藏 (Esc)" @click="hideWindow">
          <Icon name="hide" :size="17" />
        </button>
      </div>
    </header>

    <main class="relative z-1 flex flex-1 min-h-0">
      <Transition mode="out-in" :css="false" @enter="enterFade" @leave="leaveFade">
        <ChatView v-if="view === 'chat'" key="chat" />
        <SettingsView v-else key="settings" @saved="view = 'chat'" />
      </Transition>
    </main>
  </div>
</template>

<style scoped>
/* App 外壳：一整块圆角实心深色面板 */
.app {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  border-radius: var(--radius-xl);
  overflow: hidden;
  background: linear-gradient(180deg, var(--color-bg-2) 0%, var(--color-bg) 100%);
  border: 1px solid var(--color-line);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}
/* 顶部一抹极淡的电子青光晕（科技感） */
.app::before {
  content: "";
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: radial-gradient(
    120% 55% at 50% -12%,
    rgba(34, 211, 238, 0.1),
    transparent 62%
  );
}

.titlebar {
  position: relative;
  z-index: 2;
  flex: 0 0 46px;
  height: 46px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px 0 14px;
  border-bottom: 1px solid var(--color-hairline);
  background: rgba(255, 255, 255, 0.02);
  user-select: none;
}
.brand {
  display: flex;
  align-items: center;
  gap: 9px;
  font-weight: 600;
  letter-spacing: 0.2px;
}
.brand-badge {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 8px;
  color: #04121a;
  background: var(--grad-accent);
  box-shadow: 0 0 12px rgba(34, 211, 238, 0.5);
}
.brand-name {
  font-size: 14px;
  color: var(--color-fg);
}
</style>
