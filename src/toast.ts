import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { formatSize, setLanguage, t } from "./i18n";
import type { Settings, Update } from "./prefs";

type JobEvent =
  | { state: "batch"; batch: number; action: string; total: number }
  | { state: "started"; id: number; batch: number; name: string; steady: boolean }
  | { state: "running"; id: number }
  | { state: "progress"; id: number; percent: number }
  | { state: "done"; id: number; output: string; before: number; after: number }
  | { state: "installed"; id: number }
  | { state: "failed"; id: number; message: string }
  | { state: "cancelled"; id: number }
  | { state: "finished"; batch: number; copied: boolean }
  | { state: "notice"; message: string };

interface Job {
  name: string;
  steady: boolean;
  phase: "queued" | "running" | "done" | "failed" | "cancelled" | "installed";
  percent: number;
  output?: string;
  before?: number;
  after?: number;
  message?: string;
}

/** One card per batch (one menu choice), however many files it has. */
interface Card {
  action: string;
  total: number;
  jobs: Map<number, Job>;
  el: HTMLElement;
  finished: boolean;
  copied: boolean;
  undone: boolean;
}

const inTauri = "__TAURI_INTERNALS__" in window;
const list = document.querySelector("#jobs") as HTMLElement;
const cards = new Map<number, Card>();
const jobCard = new Map<number, number>();
const HIDE_AFTER_MS = 8000;
let hideTimer: number | undefined;
let hovered = false;
let settings: Settings | undefined;

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

function actionName(action: string): string {
  if (action.startsWith("custom:")) {
    return settings?.custom.find((c) => `custom:${c.id}` === action)?.name ?? t("my_actions");
  }
  return t(action);
}

/** Readable text for an error code; a program's own output stays in the tooltip. */
function errorText(message: string): string {
  if (message.startsWith("engine_missing:")) return t("engine_missing", { engine: message.split(":")[1] });
  const known = ["not_ready", "no_duration", "no_audio", "bad_range", "nothing_selected", "folder_not_last", "hotkey_taken"];
  return known.includes(message) ? t(message) : t("failed");
}

function anyRunning(): boolean {
  return [...cards.values()].some((c) => !c.finished);
}

/** Hides the toast a few seconds after the last batch finished. */
function scheduleHide() {
  clearTimeout(hideTimer);
  if (hovered || anyRunning()) return;
  hideTimer = window.setTimeout(async () => {
    list.replaceChildren();
    cards.clear();
    jobCard.clear();
    if (inTauri) await getCurrentWindow().hide();
  }, HIDE_AFTER_MS);
}

function ratio(before: number, after: number): string {
  if (before === 0) return formatSize(after);
  const change = Math.round(((after - before) / before) * 100);
  const sign = change > 0 ? "+" : change < 0 ? "−" : "±";
  return `${formatSize(before)} → ${formatSize(after)} (${sign}${Math.abs(change)}%)`;
}

function render(batch: number, card: Card) {
  const jobs = [...card.jobs.values()];
  const count = (p: Job["phase"]) => jobs.filter((j) => j.phase === p).length;
  const ok = count("done") + count("installed");
  const failed = count("failed");
  const single = card.total === 1 ? jobs[0] : undefined;

  const title = single
    ? (single.output?.split(/[\\/]/).pop() ?? single.name)
    : `${actionName(card.action)} · ${t("files_n", { n: card.total })}`;
  const percent =
    jobs.reduce((s, j) => s + (j.phase === "running" ? j.percent : j.phase === "queued" ? 0 : 100), 0) / card.total;
  const steady = jobs.some((j) => j.phase === "running" && j.steady);

  let info: string;
  if (single && card.finished) {
    info =
      single.phase === "done"
        ? ratio(single.before ?? 0, single.after ?? 0)
        : single.phase === "installed"
          ? t("engine_installed")
          : single.phase === "failed"
            ? errorText(single.message ?? "")
            : t("cancelled");
  } else if (single) {
    info = single.phase === "queued" ? t("queued") : single.steady ? t("converting") : `${t("converting")} ${single.percent}%`;
  } else {
    const parts = [t("done_of", { n: ok, total: card.total })];
    if (failed) parts.push(t("errors_n", { n: failed }));
    if (count("queued")) parts.push(t("queued_n", { n: count("queued") }));
    if (card.finished && ok > 0) {
      const before = jobs.reduce((s, j) => s + (j.output ? (j.before ?? 0) : 0), 0);
      const after = jobs.reduce((s, j) => s + (j.after ?? 0), 0);
      parts.push(ratio(before, after));
    }
    info = parts.join(" · ");
  }
  if (card.undone) info = t("undone");

  const state = card.finished ? (ok > 0 && !card.undone ? " ok" : " bad") : "";
  card.el.className = "job" + state;
  card.el.replaceChildren();
  const track = el("div", "track" + (steady && !card.finished ? " busy" : ""));
  const fill = el("div", "fill");
  fill.style.width = `${card.finished ? 100 : Math.round(percent)}%`;
  track.append(fill);
  const infoEl = el("div", "info", info);
  const errors = jobs.filter((j) => j.phase === "failed").map((j) => `${j.name}: ${j.message ?? ""}`);
  if (errors.length) infoEl.title = errors.join("\n");
  const actions = el("div", "actions");

  if (!card.finished) {
    actions.append(button(t("cancel"), () => invoke("cancel_batch", { batch })));
  } else if (ok > 0 && !card.undone && card.action !== "engine") {
    // "Copy text" leaves no file worth showing or undoing, only the clipboard.
    const textOnly = card.action === "ocr_copy";
    const first = jobs.find((j) => j.output)?.output;
    if (first && !textOnly) actions.append(button(t("show_in_folder"), () => revealItemInDir(first)));
    actions.append(
      card.copied
        ? el("span", "muted", t("copied"))
        : button(t("copy"), async () => {
            try {
              await invoke("copy_batch", { batch });
              card.copied = true;
            } catch {
              infoEl.textContent = t("copy_failed");
              return;
            }
            render(batch, card);
          }),
    );
    if (!textOnly)
      actions.append(
        button(t("undo"), async () => {
          await invoke("undo_batch", { batch });
          card.undone = true;
          render(batch, card);
        }),
      );
  }
  card.el.append(el("div", "name", title), track, infoEl, actions);
}

function handle(e: JobEvent) {
  if (e.state === "notice") {
    const card = el("div", "job bad");
    card.append(el("div", "name", "Mutabor"), el("div", "info", errorText(e.message)));
    list.prepend(card);
    scheduleHide();
    return;
  }
  if (e.state === "batch") {
    const card: Card = {
      action: e.action,
      total: e.total,
      jobs: new Map(),
      el: el("div", "job"),
      finished: false,
      copied: false,
      undone: false,
    };
    cards.set(e.batch, card);
    // Newest on top.
    list.prepend(card.el);
    clearTimeout(hideTimer);
    render(e.batch, card);
    return;
  }
  if (e.state === "finished") {
    const card = cards.get(e.batch);
    if (!card) return;
    card.finished = true;
    card.copied = e.copied;
    render(e.batch, card);
    scheduleHide();
    return;
  }
  if (e.state === "started") {
    jobCard.set(e.id, e.batch);
    const card = cards.get(e.batch);
    card?.jobs.set(e.id, { name: e.name, steady: e.steady, phase: "queued", percent: 0 });
    if (card) render(e.batch, card);
    return;
  }

  const batch = jobCard.get(e.id);
  const card = batch === undefined ? undefined : cards.get(batch);
  const job = card?.jobs.get(e.id);
  if (!card || !job || batch === undefined) return;
  switch (e.state) {
    case "running":
      job.phase = "running";
      break;
    case "progress":
      job.phase = "running";
      job.percent = e.percent;
      break;
    case "done":
      Object.assign(job, { phase: "done", output: e.output, before: e.before, after: e.after });
      break;
    case "installed":
      job.phase = "installed";
      break;
    case "failed":
      Object.assign(job, { phase: "failed", message: e.message });
      break;
    case "cancelled":
      job.phase = "cancelled";
      break;
  }
  render(batch, card);
}

/** A newer version on GitHub; shown once per version. */
function showUpdate(u: Update) {
  const card = el("div", "job ok");
  const actions = el("div", "actions");
  actions.append(button(t("update_download"), () => openUrl(u.url)));
  card.append(el("div", "name", t("update_title", { v: u.version })), el("div", "info", t("update_note")), actions);
  list.prepend(card);
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
  invoke<Settings>("get_settings").then((s) => {
    settings = s;
    setLanguage(s.language);
  });
  listen<Settings>("settings", (e) => {
    settings = e.payload;
    setLanguage(e.payload.language);
  });
  listen<JobEvent>("job", (e) => handle(e.payload));
  listen<Update>("update:new", (e) => showUpdate(e.payload));
} else {
  showUpdate({ version: "0.2.0", url: "https://github.com/Dec3n7/mutabor/releases" });
  // Plain browser (npm run dev): sample jobs for working on the layout.
  handle({ state: "batch", batch: 1, action: "compress", total: 4 });
  [11, 12, 13, 14].forEach((id, i) => handle({ state: "started", id, batch: 1, name: `clip${i}.mp4`, steady: false }));
  handle({ state: "done", id: 11, output: "/tmp/clip0_compressed.mp4", before: 90_000_000, after: 21_000_000 });
  handle({ state: "progress", id: 12, percent: 40 });
  handle({ state: "failed", id: 13, message: "no_audio" });
  handle({ state: "batch", batch: 2, action: "to_mp3", total: 1 });
  handle({ state: "started", id: 21, batch: 2, name: "song.flac", steady: false });
  handle({ state: "done", id: 21, output: "/tmp/song.mp3", before: 38_000_000, after: 6_400_000 });
  handle({ state: "finished", batch: 2, copied: true });
}
