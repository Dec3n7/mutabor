import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { formatSize, t } from "./i18n";

type JobEvent =
  | { state: "started"; id: number; name: string }
  | { state: "progress"; id: number; percent: number }
  | { state: "done"; id: number; output: string; before: number; after: number }
  | { state: "failed"; id: number; message: string }
  | { state: "cancelled"; id: number };

interface Row {
  el: HTMLElement;
  bar: HTMLElement;
  info: HTMLElement;
  actions: HTMLElement;
  finished: boolean;
}

const inTauri = "__TAURI_INTERNALS__" in window;
const list = document.querySelector("#jobs") as HTMLElement;
const rows = new Map<number, Row>();
const HIDE_AFTER_MS = 6000;
let hideTimer: number | undefined;
let hovered = false;

function el(tag: string, cls: string, text = ""): HTMLElement {
  const e = document.createElement(tag);
  e.className = cls;
  e.textContent = text;
  return e;
}

function button(text: string, onClick: () => void): HTMLElement {
  const b = el("button", "link", text);
  b.onclick = onClick;
  return b;
}

/** Hides the toast a few seconds after the last job finished. */
function scheduleHide() {
  clearTimeout(hideTimer);
  if (hovered || [...rows.values()].some((r) => !r.finished)) return;
  hideTimer = window.setTimeout(async () => {
    rows.forEach((r) => r.el.remove());
    rows.clear();
    if (inTauri) await getCurrentWindow().hide();
  }, HIDE_AFTER_MS);
}

function ratio(before: number, after: number): string {
  if (before === 0) return formatSize(after);
  const change = Math.round(((after - before) / before) * 100);
  const sign = change > 0 ? "+" : change < 0 ? "−" : "±";
  return `${formatSize(before)} → ${formatSize(after)} (${sign}${Math.abs(change)}%)`;
}

function handle(e: JobEvent) {
  if (e.state === "started") {
    const row: Row = {
      el: el("div", "job"),
      bar: el("div", "fill"),
      info: el("div", "info", t("converting")),
      actions: el("div", "actions"),
      finished: false,
    };
    const track = el("div", "track");
    track.append(row.bar);
    row.actions.append(button(t("cancel"), () => invoke("cancel_job", { id: e.id })));
    row.el.append(el("div", "name", e.name), track, row.info, row.actions);
    // Newest on top; the window shows about two jobs.
    list.prepend(row.el);
    rows.set(e.id, row);
    clearTimeout(hideTimer);
    return;
  }

  const row = rows.get(e.id);
  if (!row) return;
  if (e.state === "progress") {
    row.bar.style.width = `${e.percent}%`;
    row.info.textContent = `${t("converting")} ${e.percent}%`;
    return;
  }

  row.finished = true;
  row.actions.replaceChildren();
  if (e.state === "done") {
    row.bar.style.width = "100%";
    row.el.classList.add("ok");
    row.info.textContent = ratio(e.before, e.after);
    row.actions.append(button(t("show_in_folder"), () => revealItemInDir(e.output)));
  } else if (e.state === "failed") {
    row.el.classList.add("bad");
    row.info.textContent = t("failed");
    row.info.title = e.message;
  } else {
    row.el.classList.add("bad");
    row.info.textContent = t("cancelled");
  }
  scheduleHide();
}

document.body.addEventListener("mouseenter", () => {
  hovered = true;
  clearTimeout(hideTimer);
});
document.body.addEventListener("mouseleave", () => {
  hovered = false;
  scheduleHide();
});

if (inTauri) {
  listen<JobEvent>("job", (e) => handle(e.payload));
} else {
  // Plain browser (npm run dev): sample jobs for working on the layout.
  handle({ state: "started", id: 1, name: "video.gif" });
  handle({ state: "progress", id: 1, percent: 64 });
  handle({ state: "started", id: 2, name: "song.mp3" });
  handle({ state: "done", id: 2, output: "/tmp/song.mp3", before: 38_000_000, after: 6_400_000 });
}
