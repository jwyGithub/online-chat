<script lang="ts">
import { defineComponent, h, type PropType } from "vue";

type Shape =
  | { t: "path"; d: string }
  | { t: "circle"; cx: number; cy: number; r: number; fill?: boolean }
  | { t: "rect"; x: number; y: number; w: number; h: number; rx?: number; fill?: boolean };

// 线性 SVG 图标（24x24，stroke=currentColor），源自 Lucide 风格。
const ICONS: Record<string, Shape[]> = {
  sparkles: [
    {
      t: "path",
      d: "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .962 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.962 0z",
    },
    { t: "path", d: "M20 3v4" },
    { t: "path", d: "M22 5h-4" },
    { t: "path", d: "M4 17v2" },
    { t: "path", d: "M5 18H3" },
  ],
  settings: [
    { t: "path", d: "M20 7h-9" },
    { t: "path", d: "M14 17H5" },
    { t: "circle", cx: 17, cy: 17, r: 3 },
    { t: "circle", cx: 7, cy: 7, r: 3 },
  ],
  chat: [{ t: "path", d: "M7.9 20A9 9 0 1 0 4 16.1L2 22Z" }],
  back: [
    { t: "path", d: "m12 19-7-7 7-7" },
    { t: "path", d: "M19 12H5" },
  ],
  hide: [{ t: "path", d: "m6 9 6 6 6-6" }],
  send: [
    { t: "path", d: "m5 12 7-7 7 7" },
    { t: "path", d: "M12 19V5" },
  ],
  stop: [{ t: "rect", x: 6, y: 6, w: 12, h: 12, rx: 3, fill: true }],
  trash: [
    { t: "path", d: "M3 6h18" },
    { t: "path", d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" },
    { t: "path", d: "M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" },
    { t: "path", d: "M10 11v6" },
    { t: "path", d: "M14 11v6" },
  ],
  key: [
    {
      t: "path",
      d: "M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 0 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 0 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z",
    },
    { t: "circle", cx: 16.5, cy: 7.5, r: 0.6, fill: true },
  ],
  link: [
    { t: "path", d: "M9 17H7A5 5 0 0 1 7 7h2" },
    { t: "path", d: "M15 7h2a5 5 0 1 1 0 10h-2" },
    { t: "path", d: "M8 12h8" },
  ],
  cpu: [
    { t: "rect", x: 4, y: 4, w: 16, h: 16, rx: 2 },
    { t: "rect", x: 9, y: 9, w: 6, h: 6 },
    { t: "path", d: "M15 2v2" },
    { t: "path", d: "M15 20v2" },
    { t: "path", d: "M2 15h2" },
    { t: "path", d: "M2 9h2" },
    { t: "path", d: "M20 15h2" },
    { t: "path", d: "M20 9h2" },
    { t: "path", d: "M9 2v2" },
    { t: "path", d: "M9 20v2" },
  ],
  thermometer: [{ t: "path", d: "M14 4v10.54a4 4 0 1 1-4 0V4a2 2 0 0 1 4 0Z" }],
  command: [
    { t: "path", d: "M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" },
  ],
  eye: [
    {
      t: "path",
      d: "M2.06 12.35a1 1 0 0 1 0-.7 10.75 10.75 0 0 1 19.88 0 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-19.88 0",
    },
    { t: "circle", cx: 12, cy: 12, r: 3 },
  ],
  check: [{ t: "path", d: "M20 6 9 17l-5-5" }],
  alert: [
    {
      t: "path",
      d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z",
    },
    { t: "path", d: "M12 9v4" },
    { t: "path", d: "M12 17h.01" },
  ],
  message: [
    { t: "path", d: "M14 9a2 2 0 0 1-2 2H6l-4 4V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2z" },
    { t: "path", d: "M18 9h2a2 2 0 0 1 2 2v11l-4-4h-6a2 2 0 0 1-2-2v-1" },
  ],
};

function shapeToVNode(s: Shape) {
  if (s.t === "path") return h("path", { d: s.d });
  if (s.t === "circle")
    return h("circle", { cx: s.cx, cy: s.cy, r: s.r, fill: s.fill ? "currentColor" : "none" });
  return h("rect", {
    x: s.x,
    y: s.y,
    width: s.w,
    height: s.h,
    rx: s.rx ?? 0,
    fill: s.fill ? "currentColor" : "none",
  });
}

export default defineComponent({
  name: "Icon",
  props: {
    name: { type: String as PropType<string>, required: true },
    size: { type: [Number, String], default: 18 },
    strokeWidth: { type: Number, default: 1.75 },
  },
  setup(props) {
    return () =>
      h(
        "svg",
        {
          width: props.size,
          height: props.size,
          viewBox: "0 0 24 24",
          fill: "none",
          stroke: "currentColor",
          "stroke-width": props.strokeWidth,
          "stroke-linecap": "round",
          "stroke-linejoin": "round",
          "aria-hidden": "true",
          class: "icon",
        },
        (ICONS[props.name] || []).map(shapeToVNode)
      );
  },
});
</script>
