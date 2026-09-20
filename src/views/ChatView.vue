<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import MarkdownRender from "markstream-vue";
import Icon from "../components/Icon.vue";
import { useChat } from "../composables/useChat";
import { gsap, reduceMotion } from "../composables/useMotion";

const { messages, streaming, error, send, stop, clear } = useChat();
const input = ref("");
const composing = ref(false);
const listEl = ref<HTMLElement | null>(null);
const nearBottom = ref(true);

const canSend = computed(() => input.value.trim().length > 0 && !streaming.value);

function isStreamingLast(i: number): boolean {
  return streaming.value && i === messages.value.length - 1;
}

// 思考面板展开状态：用户未手动切换时，流式中默认展开、结束后默认折叠。
const reasoningExpanded = reactive<Record<number, boolean>>({});
function isReasoningOpen(i: number): boolean {
  return reasoningExpanded[i] ?? isStreamingLast(i);
}
function toggleReasoning(i: number) {
  reasoningExpanded[i] = !isReasoningOpen(i);
}

function onScroll() {
  const el = listEl.value;
  if (!el) return;
  nearBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight < 90;
}

async function scrollBottom() {
  await nextTick();
  const el = listEl.value;
  if (el && nearBottom.value) el.scrollTop = el.scrollHeight;
}

watch(messages, scrollBottom, { deep: true });

// 每条新消息的入场动画（GSAP，逐条上浮淡入）
function onMsgEnter(el: Element, done: () => void) {
  if (reduceMotion()) return done();
  gsap.fromTo(
    el,
    { autoAlpha: 0, y: 16, scale: 0.98 },
    { autoAlpha: 1, y: 0, scale: 1, duration: 0.4, ease: "power3.out", onComplete: done }
  );
}

async function onSend() {
  if (!canSend.value) return;
  const text = input.value;
  input.value = "";
  nearBottom.value = true;
  await send(text);
}

function onKeydown(e: KeyboardEvent) {
  // 输入法（中文/日文等）合成期间的回车用于确认候选词，不应触发发送。
  // 浏览器(Chromium)中确认候选词时 keydown 的 isComposing 仍为 true，可拦下；
  // 但 Tauri 的 WKWebView 会先触发 compositionend（把 composing 置回 false）再触发 keydown，
  // 此时 isComposing/composing 都已是 false，故需额外用 keyCode===229 及刚结束合成的标记兜底。
  if (e.isComposing || composing.value || e.keyCode === 229 || justComposed) return;
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    onSend();
  }
}

// 标记「刚结束输入法合成」：compositionend 到紧随其后的 keydown 之间保持为 true，
// 用于兼容 compositionend 先于 keydown 触发的 WebView。
let justComposed = false;
let justComposedTimer: ReturnType<typeof setTimeout> | undefined;
function onCompositionEnd() {
  composing.value = false;
  justComposed = true;
  clearTimeout(justComposedTimer);
  // 在下一个宏任务里复位；同一次回车确认产生的 keydown 仍处于当前任务，能被拦下。
  justComposedTimer = setTimeout(() => {
    justComposed = false;
  }, 0);
}
</script>

<template>
  <div class="flex flex-1 flex-col min-h-0 w-full">
    <div
      ref="listEl"
      class="flex-1 min-h-0 overflow-y-auto px-3.5 pt-4 pb-2"
      @scroll="onScroll"
    >
      <div
        v-if="messages.length === 0"
        class="flex h-full min-h-65 flex-col items-center justify-center gap-2 text-center text-fg-3"
      >
        <div
          class="mb-1 grid h-14.5 w-14.5 place-items-center rounded-lg border border-line bg-accent-soft text-accent"
        >
          <Icon name="sparkles" :size="26" />
        </div>
        <p class="m-0 text-[15px] font-semibold text-fg-2">开始一段对话</p>
        <p class="m-0 text-xs">Enter 发送 · Shift + Enter 换行 · Esc 隐藏窗口</p>
      </div>

      <TransitionGroup
        name="msg"
        tag="div"
        class="flex flex-col gap-3.5"
        :css="false"
        @enter="onMsgEnter"
      >
        <div
          v-for="(m, i) in messages"
          :key="i"
          class="flex will-change-[transform,opacity]"
          :class="{ 'justify-end': m.role === 'user' }"
        >
          <div
            v-if="m.role === 'user'"
            class="max-w-[84%] whitespace-pre-wrap wrap-break-word rounded-lg rounded-br-[5px] bg-grad-accent px-3.5 py-2.5 leading-relaxed text-[#04121a] shadow-[0_4px_16px_rgba(34,211,238,0.28)]"
          >
            {{ m.content }}
          </div>

          <div
            v-else
            class="max-w-[84%] rounded-lg rounded-bl-[5px] border border-line bg-surface px-3.5 py-2.5 leading-relaxed shadow-[0_2px_10px_rgba(0,0,0,0.4)]"
          >
            <div v-if="m.reasoning" class="reasoning" :class="{ open: isReasoningOpen(i) }">
              <button type="button" class="reasoning-head" @click="toggleReasoning(i)">
                <Icon name="sparkles" :size="13" />
                <span class="reasoning-title">思考过程</span>
                <Icon name="hide" :size="14" class="reasoning-chev" />
              </button>
              <div v-if="isReasoningOpen(i)" class="reasoning-body">{{ m.reasoning }}</div>
            </div>

            <div
              v-if="!m.content && isStreamingLast(i)"
              class="typing"
              aria-label="思考中"
            >
              <span></span><span></span><span></span>
            </div>
            <div v-else class="assistant-md">
              <MarkdownRender
                mode="chat"
                :content="m.content"
                :final="!isStreamingLast(i)"
                :is-dark="true"
                :smooth-streaming="'auto'"
                :fade="false"
              />
            </div>
          </div>
        </div>
      </TransitionGroup>

      <Transition name="fade">
        <div
          v-if="error"
          class="mt-3 flex items-center gap-2 rounded-md border border-danger/35 bg-danger-soft px-3 py-2.5 text-[13px] text-danger"
        >
          <Icon name="alert" :size="16" class="shrink-0" />
          <span>{{ error }}</span>
        </div>
      </Transition>
    </div>

    <div class="flex-none border-t border-hairline px-3 pt-2.5 pb-3">
      <form class="composer-field" @submit.prevent="onSend">
        <textarea
          v-model="input"
          :disabled="streaming"
          placeholder="给模型发消息…"
          rows="1"
          @keydown="onKeydown"
          @compositionstart="composing = true"
          @compositionend="onCompositionEnd"
        ></textarea>

        <div class="flex items-center gap-1 pb-px">
          <button
            type="button"
            class="icon-btn"
            title="清空对话"
            :disabled="streaming || messages.length === 0"
            @click="clear"
          >
            <Icon name="trash" :size="16" />
          </button>

          <button
            v-if="!streaming"
            type="submit"
            class="send-btn"
            title="发送 (Enter)"
            :disabled="!canSend"
          >
            <Icon name="send" :size="18" />
          </button>
          <button v-else type="button" class="send-btn stop" title="停止生成" @click="stop">
            <Icon name="stop" :size="16" />
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
/* 思考过程面板 */
.reasoning {
  margin-bottom: 8px;
  border: 1px solid var(--color-hairline, rgba(255, 255, 255, 0.08));
  border-radius: 10px;
  background: rgba(34, 211, 238, 0.05);
  overflow: hidden;
}
.reasoning-head {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 6px 10px;
  background: transparent;
  border: 0;
  cursor: pointer;
  color: var(--color-fg, #cbd5e1);
  font-size: 12px;
  font-weight: 600;
  text-align: left;
}
.reasoning-head :first-child {
  color: #22d3ee;
}
.reasoning-title {
  flex: 1;
}
.reasoning-chev {
  transition: transform 0.18s ease;
  transform: rotate(-90deg);
  opacity: 0.6;
}
.reasoning.open .reasoning-chev {
  transform: rotate(0deg);
}
.reasoning-body {
  padding: 2px 10px 9px;
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--color-fg, #cbd5e1);
  opacity: 0.72;
  white-space: pre-wrap;
  word-break: break-word;
  border-top: 1px solid var(--color-hairline, rgba(255, 255, 255, 0.06));
  max-height: 260px;
  overflow-y: auto;
}

/* 打字指示器 */
.typing {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 2px;
}
.typing span {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--color-accent);
  box-shadow: 0 0 6px rgba(34, 211, 238, 0.6);
  animation: typing 1.2s var(--ease) infinite;
}
.typing span:nth-child(2) {
  animation-delay: 0.18s;
}
.typing span:nth-child(3) {
  animation-delay: 0.36s;
}
@keyframes typing {
  0%,
  60%,
  100% {
    opacity: 0.35;
    transform: translateY(0);
  }
  30% {
    opacity: 1;
    transform: translateY(-4px);
  }
}

/* 输入区外框 */
.composer-field {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 6px 6px 6px 4px;
  border: 1px solid var(--color-line);
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  transition: border-color 0.18s var(--ease), box-shadow 0.18s var(--ease);
}
.composer-field:focus-within {
  border-color: var(--color-accent-ring);
  box-shadow: var(--glow-accent);
}
.composer-field textarea {
  flex: 1;
  border: none;
  background: transparent;
  padding: 7px 8px;
  max-height: 150px;
  min-height: 24px;
  line-height: 1.5;
  resize: none;
  field-sizing: content;
}
.composer-field textarea:focus {
  outline: none;
  box-shadow: none;
  background: transparent;
}

/* 发送 / 停止按钮 */
.send-btn {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  border-radius: 50%;
  border: none;
  color: #04121a;
  background: var(--grad-accent);
  box-shadow: 0 0 14px rgba(34, 211, 238, 0.45);
  cursor: pointer;
  transition: filter 0.16s var(--ease), transform 0.12s var(--ease),
    background 0.16s var(--ease);
}
.send-btn:hover:not(:disabled) {
  filter: brightness(1.1);
}
.send-btn:active:not(:disabled) {
  transform: scale(0.95);
}
.send-btn:disabled {
  background: var(--color-surface-3);
  color: var(--color-fg-3);
  box-shadow: none;
  cursor: not-allowed;
}
.send-btn.stop {
  color: #fff;
  background: var(--color-danger);
  box-shadow: 0 0 14px rgba(255, 107, 107, 0.4);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s var(--ease);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
