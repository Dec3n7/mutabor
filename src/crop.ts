// The crop window: shows a picture with a frame the user drags and resizes,
// then crops the real file to that frame. Several pictures go one by one.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setLanguage, t } from "./i18n";
import type { Settings } from "./prefs";

interface Preview {
  src: string;
  /** Size in pixels of the picture the crop applies to. */
  width: number;
  height: number;
}
interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const inTauri = "__TAURI_INTERNALS__" in window;
const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const picture = $<HTMLImageElement>("#picture");
const canvas = $<HTMLElement>("#canvas");
const frame = $<HTMLElement>("#frame");
const frameSize = $<HTMLElement>("#frame-size");
const ratios = $<HTMLElement>("#crop-ratios");
const statusEl = $<HTMLElement>("#crop-status");

/** Width:height pairs; null is a free frame. */
const RATIOS: ([number, number] | null)[] = [null, [1, 1], [4, 3], [3, 2], [16, 9], [9, 16], [4, 5]];
const MIN = 8;

let queue: string[] = [];
let index = 0;
let preview: Preview | null = null;
let rect: Rect = { x: 0, y: 0, w: 0, h: 0 };
let ratio: [number, number] | null = null;

/** Display pixels per picture pixel. */
const scale = () => (preview ? picture.clientWidth / preview.width : 1);
const name = () => queue[index]?.split(/[\\/]/).pop() ?? "";

function labels() {
  $<HTMLElement>("#crop-hint").textContent = t("crop_hint");
  $<HTMLElement>("#crop-reset").textContent = t("crop_reset");
  $<HTMLElement>("#crop-skip").textContent = t("crop_skip");
  $<HTMLElement>("#crop-cancel").textContent = t("cancel");
  $<HTMLElement>("#crop-save").textContent = `${t("s_save")}  ↵`;
  document.title = `${t("crop")} · Mutabor`;
  ratios.replaceChildren();
  for (const r of RATIOS) {
    const b = document.createElement("button");
    const on = r === ratio || (r && ratio && r[0] === ratio[0] && r[1] === ratio[1]);
    b.className = "chip" + (on ? " on" : "");
    b.textContent = r ? `${r[0]}:${r[1]}` : t("crop_free");
    b.onclick = () => setRatio(r);
    ratios.append(b);
  }
}

function draw() {
  const s = scale();
  Object.assign(frame.style, {
    left: `${rect.x * s}px`,
    top: `${rect.y * s}px`,
    width: `${rect.w * s}px`,
    height: `${rect.h * s}px`,
  });
  frameSize.textContent = `${Math.round(rect.w)} × ${Math.round(rect.h)}`;
}

/** Keeps the frame inside the picture and at least MIN pixels big. */
function clamp(r: Rect): Rect {
  if (!preview) return r;
  const w = Math.min(Math.max(r.w, MIN), preview.width);
  const h = Math.min(Math.max(r.h, MIN), preview.height);
  return {
    w,
    h,
    x: Math.min(Math.max(r.x, 0), preview.width - w),
    y: Math.min(Math.max(r.y, 0), preview.height - h),
  };
}

/** The largest centred frame with the ratio, or the whole picture. */
function fullFrame(): Rect {
  if (!preview) return rect;
  const { width: W, height: H } = preview;
  if (!ratio) return { x: 0, y: 0, w: W, h: H };
  const k = ratio[0] / ratio[1];
  const w = Math.min(W, H * k);
  const h = w / k;
  return { x: (W - w) / 2, y: (H - h) / 2, w, h };
}

function setRatio(r: [number, number] | null) {
  ratio = r;
  rect = fullFrame();
  labels();
  draw();
}

async function load() {
  preview = null;
  frame.hidden = true;
  picture.removeAttribute("src");
  $<HTMLElement>("#crop-name").textContent = name();
  $<HTMLElement>("#crop-count").textContent = queue.length > 1 ? `${index + 1} / ${queue.length}` : "";
  $<HTMLButtonElement>("#crop-skip").hidden = queue.length < 2;
  statusEl.textContent = t("crop_loading");
  try {
    preview = inTauri ? await invoke<Preview>("crop_preview", { path: queue[index] }) : demoPreview();
  } catch (err) {
    statusEl.textContent = t("crop_unreadable", { msg: String(err) });
    return;
  }
  picture.onload = () => {
    statusEl.textContent = "";
    frame.hidden = false;
    rect = fullFrame();
    draw();
  };
  picture.src = preview.src;
}

async function next() {
  index++;
  if (index < queue.length) load();
  else close();
}

async function close() {
  queue = [];
  if (inTauri) await getCurrentWindow().hide();
}

async function save() {
  if (!preview || !queue[index]) return;
  const r = clamp(rect);
  const options = { rect: [Math.round(r.x), Math.round(r.y), Math.round(r.w), Math.round(r.h)] };
  try {
    if (inTauri) await invoke("run_action", { action: "crop", paths: [queue[index]], options });
  } catch (err) {
    statusEl.textContent = t("start_failed", { msg: String(err) });
    return;
  }
  next();
}

// Dragging: the frame body moves it, the handles resize from that side.
let drag: { handle: string; start: Rect; px: number; py: number } | null = null;

frame.addEventListener("pointerdown", (e) => {
  const handle = (e.target as HTMLElement).dataset.h ?? "move";
  drag = { handle, start: { ...rect }, px: e.clientX, py: e.clientY };
  frame.setPointerCapture(e.pointerId);
  e.preventDefault();
});

frame.addEventListener("pointermove", (e) => {
  if (!drag || !preview) return;
  const s = scale();
  const dx = (e.clientX - drag.px) / s;
  const dy = (e.clientY - drag.py) / s;
  const { x, y, w, h } = drag.start;
  if (drag.handle === "move") {
    rect = clamp({ x: x + dx, y: y + dy, w, h });
    return draw();
  }
  let left = x;
  let top = y;
  let right = x + w;
  let bottom = y + h;
  if (drag.handle.includes("w")) left = Math.min(x + dx, right - MIN);
  if (drag.handle.includes("e")) right = Math.max(x + w + dx, left + MIN);
  if (drag.handle.includes("n")) top = Math.min(y + dy, bottom - MIN);
  if (drag.handle.includes("s")) bottom = Math.max(y + h + dy, top + MIN);
  left = Math.max(left, 0);
  top = Math.max(top, 0);
  right = Math.min(right, preview.width);
  bottom = Math.min(bottom, preview.height);
  let nw = right - left;
  let nh = bottom - top;
  if (ratio) {
    // Keep the proportions: the side being dragged leads, the other follows.
    const k = ratio[0] / ratio[1];
    const sideways = drag.handle === "e" || drag.handle === "w";
    if (sideways || (drag.handle.length === 2 && nw / nh > k)) nh = nw / k;
    else nw = nh * k;
    if (left + nw > preview.width || (drag.handle.includes("w") && right - nw < 0)) {
      nw = drag.handle.includes("w") ? right : preview.width - left;
      nh = nw / k;
    }
    if (top + nh > preview.height || (drag.handle.includes("n") && bottom - nh < 0)) {
      nh = drag.handle.includes("n") ? bottom : preview.height - top;
      nw = nh * k;
    }
    if (drag.handle.includes("w")) left = right - nw;
    if (drag.handle.includes("n")) top = bottom - nh;
  }
  rect = clamp({ x: left, y: top, w: nw, h: nh });
  draw();
});

frame.addEventListener("pointerup", () => (drag = null));
window.addEventListener("resize", draw);

document.addEventListener("keydown", (e) => {
  if (e.key === "Enter") save();
  else if (e.key === "Escape") close();
  else if (e.key.startsWith("Arrow")) {
    const step = e.shiftKey ? 10 : 1;
    const dx = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
    const dy = e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0;
    rect = clamp({ ...rect, x: rect.x + dx, y: rect.y + dy });
    draw();
  } else return;
  e.preventDefault();
});

$<HTMLButtonElement>("#crop-save").onclick = save;
$<HTMLButtonElement>("#crop-cancel").onclick = close;
$<HTMLButtonElement>("#crop-skip").onclick = next;
$<HTMLButtonElement>("#crop-reset").onclick = () => setRatio(ratio);

/** Plain browser (npm run dev): a drawn sample picture. */
function demoPreview(): Preview {
  const c = document.createElement("canvas");
  c.width = 1200;
  c.height = 800;
  const g = c.getContext("2d")!;
  const grad = g.createLinearGradient(0, 0, 1200, 800);
  grad.addColorStop(0, "#2f6fde");
  grad.addColorStop(1, "#5dcaa5");
  g.fillStyle = grad;
  g.fillRect(0, 0, 1200, 800);
  g.fillStyle = "#fff";
  g.font = "bold 120px sans-serif";
  g.fillText("Mutabor", 330, 440);
  return { src: c.toDataURL("image/jpeg"), width: 4800, height: 3200 };
}

async function init() {
  if (inTauri) {
    const s = await invoke<Settings>("get_settings");
    setLanguage(s.language);
    listen<Settings>("settings", (e) => {
      setLanguage(e.payload.language);
      labels();
    });
    listen<string[]>("crop:open", (e) => {
      queue = e.payload;
      index = 0;
      ratio = null;
      labels();
      load();
    });
  }
  labels();
  if (!inTauri) {
    setLanguage(new URLSearchParams(location.search).get("lang") ?? "auto");
    queue = ["/tmp/IMG_2041.heic", "/tmp/photo.jpg"];
    labels();
    load();
  }
}
init();
canvas.addEventListener("dragstart", (e) => e.preventDefault());
