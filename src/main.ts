import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Action,
  CHOICES,
  Field,
  FileInfo,
  Options,
  actionsFor,
  defaultOptions,
  notSupportedYet,
  quickFor,
  suggest,
} from "./actions";
import { formatSize, setLanguage, t } from "./i18n";
import { DEFAULT_SETTINGS, Settings } from "./prefs";

const inTauri = "__TAURI_INTERNALS__" in window;

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const header = $<HTMLElement>("#header");
const quick = $<HTMLElement>("#quick");
const list = $<HTMLElement>("#list");
const search = $<HTMLInputElement>("#search");
const status = $<HTMLElement>("#status");
const gear = $<HTMLButtonElement>("#gear");

let files: FileInfo[] = [];
let settings: Settings = DEFAULT_SETTINGS;
let visible: Action[] = [];
let selected = 0;

/** The options submenu, when open: its action, choices and focused field. */
let config: { action: Action; opts: Options; field: number } | null = null;

function matches(a: Action, query: string): boolean {
  const hay = `${t(a.id)} ${a.keywords ?? ""}`.toLowerCase();
  return query.toLowerCase().split(/\s+/).every((w) => hay.includes(w));
}

function el(tag: string, cls: string, text = ""): HTMLElement {
  const e = document.createElement(tag);
  e.className = cls;
  e.textContent = text;
  return e;
}

function renderHeader() {
  header.replaceChildren();
  if (files.length === 0) {
    header.append(el("div", "empty", t("nothing_selected")));
    return;
  }
  const total = files.reduce((s, f) => s + f.size, 0);
  const title = files.length === 1 ? files[0].name : t("files_n", { n: files.length });
  header.append(el("div", "title", title), el("div", "meta", formatSize(total)));
}

function renderList() {
  const query = search.value.trim();
  const all = actionsFor(files);
  const hint = query ? undefined : suggest(files);

  quick.replaceChildren();
  if (!query) {
    for (const a of quickFor(files)) {
      const b = el("button", "quick", a.glyph ?? t(a.id));
      b.title = t(a.id);
      b.onclick = () => run(a);
      quick.append(b);
    }
  }

  const filtered = all.filter((a) => !query || matches(a, query));
  const top = hint ? filtered.filter((a) => a.id === hint) : [];
  const rest = filtered.filter((a) => a.id !== hint);
  visible = [...top, ...rest];
  selected = Math.min(selected, Math.max(visible.length - 1, 0));

  list.replaceChildren();
  if (files.length > 0 && notSupportedYet(files)) list.append(el("div", "empty", t("not_supported_yet")));
  else if (files.length > 0 && all.length === 0) list.append(el("div", "empty", t("no_common")));
  else if (files.length > 0 && visible.length === 0) list.append(el("div", "empty", t("no_match")));

  let i = 0;
  const section = (name: string, items: Action[]) => {
    if (items.length === 0) return;
    list.append(el("div", "section", t(name)));
    for (const a of items) {
      const idx = i++;
      const row = el("div", "row" + (idx === selected ? " active" : ""));
      row.append(el("span", "label", t(a.id)));
      const side = el("span", "side");
      side.append(el("span", "key", idx < 9 ? String(idx + 1) : ""));
      if (a.options?.length) {
        const more = el("button", "more", "›");
        more.title = t("options_of", { name: t(a.id) });
        more.onclick = (e) => {
          e.stopPropagation();
          openConfig(a);
        };
        side.append(more);
      } else side.append(el("span", "more"));
      row.append(side);
      row.onmouseenter = () => select(idx);
      row.onclick = () => run(a);
      list.append(row);
    }
  };
  section("suggested", top);
  section("all_actions", rest);
}

function select(idx: number) {
  if (visible.length === 0) return;
  selected = (idx + visible.length) % visible.length;
  const rows = list.querySelectorAll(".row");
  rows.forEach((r, i) => r.classList.toggle("active", i === selected));
  // Keep section headings visible when wrapping back to the first row.
  if (selected === 0) list.scrollTop = 0;
  else rows[selected]?.scrollIntoView({ block: "nearest" });
}

function choiceLabel(field: Field, v: string | number): string {
  if (field === "quality") return t(`q_${v}`);
  if (v === 0) return t("original");
  if (field === "height") return v === 2160 ? "4K" : `${v}p`;
  if (field === "width") return `${v} px`;
  return String(v);
}

function openConfig(a: Action) {
  const remembered = settings.remember ? settings.last[a.id] : undefined;
  config = { action: a, opts: { ...defaultOptions(a, files, settings.quality), ...remembered }, field: 0 };
  search.hidden = true;
  quick.hidden = true;
  status.textContent = t("hint_options");
  renderConfig();
}

function closeConfig() {
  config = null;
  search.hidden = false;
  quick.hidden = false;
  status.textContent = t("hint");
  renderList();
  search.focus();
}

function renderConfig() {
  if (!config) return;
  const { action, opts, field } = config;
  list.replaceChildren();
  const back = el("button", "back", `‹ ${t("options_of", { name: t(action.id) })}`);
  back.onclick = closeConfig;
  list.append(back);
  action.options!.forEach((f, i) => {
    const group = el("div", "group" + (i === field ? " focused" : ""));
    group.append(el("div", "section", t(`opt_${f}`)));
    const chips = el("div", "chips");
    for (const v of CHOICES[f]) {
      const chip = el("button", "chip" + (opts[f] === v ? " on" : ""), choiceLabel(f, v));
      chip.onclick = () => {
        (opts as Record<string, unknown>)[f] = v;
        config!.field = i;
        renderConfig();
      };
      chips.append(chip);
    }
    group.append(chips);
    list.append(group);
  });
  const go = el("button", "primary", `${t("run")}  ↵`);
  go.onclick = () => run(action, opts);
  list.append(go);
}

/** ←/→ on the focused field of the submenu. */
function step(dir: number) {
  if (!config) return;
  const f = config.action.options![config.field];
  const values = CHOICES[f];
  const at = values.indexOf(config.opts[f] as string | number);
  (config.opts as Record<string, unknown>)[f] = values[(at + dir + values.length) % values.length];
  renderConfig();
}

async function run(a: Action, options?: Options) {
  try {
    await invoke("run_action", { action: a.id, paths: files.map((f) => f.path), options: options ?? null });
    await hide();
  } catch (err) {
    const code = String(err);
    status.textContent =
      ["not_ready", "no_duration", "no_audio"].includes(code) ? t(code) : t("start_failed", { msg: code });
  }
}

async function hide() {
  if (inTauri) await getCurrentWindow().hide();
}

function open(next: FileInfo[], prefs: Settings) {
  files = next;
  settings = prefs;
  setLanguage(prefs.language);
  search.placeholder = t("search");
  gear.title = t("settings");
  selected = 0;
  search.value = "";
  config = null;
  search.hidden = false;
  quick.hidden = false;
  status.textContent = t("hint");
  renderHeader();
  renderList();
  search.focus();
}

search.addEventListener("input", () => {
  selected = 0;
  renderList();
});

gear.onclick = () => {
  if (inTauri) invoke("open_settings");
};

function configKeys(e: KeyboardEvent) {
  const n = config!.action.options!.length;
  if (e.key === "Escape" || e.key === "Backspace" || (e.key === "ArrowLeft" && e.altKey)) closeConfig();
  else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    config!.field = (config!.field + (e.key === "ArrowDown" ? 1 : -1) + n) % n;
    renderConfig();
  } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") step(e.key === "ArrowRight" ? 1 : -1);
  else if (e.key === "Enter") run(config!.action, config!.opts);
  else return;
  e.preventDefault();
}

document.addEventListener("keydown", (e) => {
  if (config) return configKeys(e);
  const current = visible[selected];
  const caretAtEnd = search.selectionStart === search.value.length;
  if (e.key === "Escape") {
    e.preventDefault();
    hide();
  } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    select(selected + (e.key === "ArrowDown" ? 1 : -1));
  } else if (current?.options?.length && ((e.key === "Enter" && e.shiftKey) || (e.key === "ArrowRight" && caretAtEnd))) {
    e.preventDefault();
    openConfig(current);
  } else if (e.key === "Enter" && current) {
    e.preventDefault();
    run(current);
  } else if (/^[1-9]$/.test(e.key) && search.value === "") {
    const a = visible[Number(e.key) - 1];
    if (a) {
      e.preventDefault();
      run(a);
    }
  }
});

if (inTauri) {
  listen<{ files: FileInfo[]; settings: Settings }>("menu:open", (e) => open(e.payload.files, e.payload.settings));
} else {
  // Plain browser (npm run dev): sample data for working on the layout.
  const demo: Record<string, FileInfo[]> = {
    video: [{ path: "/tmp/video.mp4", name: "video.mp4", ext: "mp4", kind: "video", size: 38_000_000 }],
    image: [{ path: "/tmp/IMG_2041.heic", name: "IMG_2041.heic", ext: "heic", kind: "image", size: 2_900_000 }],
    mixed: [
      { path: "/tmp/a.png", name: "a.png", ext: "png", kind: "image", size: 120_000 },
      { path: "/tmp/b.mp3", name: "b.mp3", ext: "mp3", kind: "audio", size: 4_000_000 },
    ],
    none: [],
  };
  const params = new URLSearchParams(location.search);
  open(demo[params.get("demo") ?? "video"] ?? [], DEFAULT_SETTINGS);
  const opts = visible.find((a) => a.id === params.get("options"));
  if (opts) openConfig(opts);
}
