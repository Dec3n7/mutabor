import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Action,
  EngineInfo,
  Field,
  FileInfo,
  Options,
  TIME_FIELDS,
  actionsFor,
  chipValue,
  choicesFor,
  defaultOptions,
  formatTime,
  missingEngine,
  notSupportedYet,
  parseTime,
  quickFor,
  setChip,
  suggest,
} from "./actions";
import { formatSize, setLanguage, t } from "./i18n";
import { DEFAULT_SETTINGS, Settings, Update, showHotkey } from "./prefs";

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
let engines: EngineInfo[] = [];
/** Where the files came from: "selection", "clipboard_files", "clipboard_image". */
let source = "selection";
let visible: Action[] = [];
let selected = 0;

/** The options submenu, when open: its action, choices and focused field. */
let config: { action: Action; opts: Options; field: number } | null = null;

/** Menu text of an action: the user's own name, or the translation. */
const label = (a: Action) => a.label ?? t(a.id);
const fromClipboard = () => source === "clipboard_image";

function matches(a: Action, query: string): boolean {
  const hay = `${label(a)} ${a.keywords ?? ""}`.toLowerCase();
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
  const title = fromClipboard()
    ? t("clipboard_image")
    : files.length === 1
      ? files[0].name
      : t("files_n", { n: files.length });
  const meta = formatSize(total) + (source.startsWith("clipboard") ? ` · ${t("from_clipboard")}` : "");
  header.append(el("div", "title", title), el("div", "meta", meta));
}

function renderList() {
  const query = search.value.trim();
  const all = actionsFor(files, engines, settings.custom, fromClipboard());
  const hint = query ? undefined : suggest(files, fromClipboard());

  quick.replaceChildren();
  if (!query) {
    for (const a of quickFor(files)) {
      const b = el("button", "quick", a.glyph ?? label(a));
      b.title = label(a);
      b.onclick = () => run(a);
      quick.append(b);
    }
  }

  const filtered = all.filter((a) => !query || matches(a, query));
  const top = hint ? filtered.filter((a) => a.id === hint) : [];
  const own = filtered.filter((a) => a.id.startsWith("custom:") && a.id !== hint);
  const rest = filtered.filter((a) => a.id !== hint && !a.id.startsWith("custom:"));
  visible = [...top, ...own, ...rest];
  selected = Math.min(selected, Math.max(visible.length - 1, 0));

  list.replaceChildren();
  if (files.length > 0 && notSupportedYet(files, engines)) list.append(el("div", "empty", t("not_supported_yet")));
  else if (files.length > 0 && all.length === 0) list.append(el("div", "empty", t("no_common")));
  else if (files.length > 0 && visible.length === 0) list.append(el("div", "empty", t("no_match")));

  let i = 0;
  const section = (name: string, items: Action[]) => {
    if (items.length === 0) return;
    list.append(el("div", "section", t(name)));
    for (const a of items) {
      const idx = i++;
      const missing = missingEngine(a, engines);
      const row = el("div", "row" + (idx === selected ? " active" : "") + (missing ? " locked" : ""));
      row.append(el("span", "label", label(a)));
      const side = el("span", "side");
      const hotkey = settings.hotkeys?.[a.id];
      if (hotkey) side.append(el("span", "badge", showHotkey(hotkey)));
      if (missing) {
        const badge = el("span", "badge", `⤓ ${formatSize(missing.size)}`);
        badge.title = t("needs_engine", { engine: missing.title });
        side.append(badge);
      }
      side.append(el("span", "key", idx < 9 ? String(idx + 1) : ""));
      if (a.options?.length && !a.window) {
        const more = el("button", "more", "›");
        more.title = t("options_of", { name: label(a) });
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
  section("my_actions", own);
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
  if (field === "page") return t(`page_${v}`);
  if (field === "ratio") return String(v);
  if (v === 0) return t("original");
  if (field === "height") return v === 2160 ? "4K" : v === 1440 ? "2K" : `${v}p`;
  if (field === "width" || field === "side") return `${v} px`;
  if (field === "mb") return t("mb_n", { n: v });
  if (field === "speed") return `×${v}`;
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

/** Moves the keyboard focus to the submenu field `i` (text fields get the caret). */
function focusField(i: number) {
  if (!config) return;
  config.field = i;
  list.querySelectorAll(".group").forEach((g, n) => g.classList.toggle("focused", n === i));
  const input = list.querySelectorAll(".group")[i]?.querySelector("input");
  if (input) input.focus();
  else (document.activeElement as HTMLElement | null)?.blur();
}

function renderConfig() {
  if (!config) return;
  const { action, opts, field } = config;
  list.replaceChildren();
  const back = el("button", "back", `‹ ${t("options_of", { name: label(action) })}`);
  back.onclick = closeConfig;
  list.append(back);
  action.options!.forEach((f, i) => {
    const group = el("div", "group" + (i === field ? " focused" : ""));
    group.append(el("div", "section", t(`opt_${f}`)));
    if (TIME_FIELDS.includes(f)) {
      const input = document.createElement("input");
      input.className = "time";
      input.placeholder = f === "end" ? t("to_end") : "0:00";
      const v = opts[f as "start" | "end" | "at"];
      input.value = v === undefined ? "" : formatTime(v);
      input.oninput = () => {
        const secs = parseTime(input.value);
        input.classList.toggle("bad", Number.isNaN(secs));
        opts[f as "start" | "end" | "at"] = secs;
      };
      input.onfocus = () => focusField(i);
      group.append(input);
    } else {
      const chips = el("div", "chips");
      for (const v of choicesFor(action, f)) {
        const chip = el("button", "chip" + (chipValue(opts, f) === v ? " on" : ""), choiceLabel(f, v));
        chip.onclick = () => {
          setChip(opts, f, v);
          config!.field = i;
          renderConfig();
        };
        chips.append(chip);
      }
      group.append(chips);
    }
    list.append(group);
  });
  const go = el("button", "primary", `${t("run")}  ↵`);
  go.onclick = () => runConfig();
  list.append(go);
  focusField(field);
}

/** ←/→ on the focused chip field of the submenu. */
function step(dir: number) {
  if (!config) return;
  const f = config.action.options![config.field];
  const values = choicesFor(config.action, f);
  if (values.length === 0) return;
  const at = values.indexOf(chipValue(config.opts, f)!);
  setChip(config.opts, f, values[(at + dir + values.length) % values.length]);
  renderConfig();
}

function runConfig() {
  if (!config) return;
  const { start, end, at } = config.opts;
  if ([start, end, at].some((v) => Number.isNaN(v))) {
    status.textContent = t("bad_time");
    return;
  }
  run(config.action, config.opts);
}

async function run(a: Action, options?: Options) {
  const missing = missingEngine(a, engines);
  if (missing) return offerEngine(a, missing, options);
  if (a.window && !options) {
    if (inTauri) await invoke("open_crop", { paths: files.map((f) => f.path) });
    return hide();
  }
  if (a.ask && !options) return openConfig(a);
  try {
    await invoke("run_action", { action: a.id, paths: files.map((f) => f.path), options: options ?? null });
    await hide();
  } catch (err) {
    status.textContent = errorText(String(err));
  }
}

/** Readable text for an error code from the Rust side. */
function errorText(code: string): string {
  if (code.startsWith("engine_missing:")) return t("engine_missing", { engine: code.split(":")[1] });
  const known = ["not_ready", "no_duration", "no_audio", "bad_range", "nothing_selected", "folder_not_last"];
  return known.includes(code) ? t(code) : t("start_failed", { msg: code });
}

/** The download offer shown in place of the list; Enter accepts, Esc goes back. */
let offer: (() => void) | null = null;

function offerEngine(a: Action, engine: EngineInfo, options?: Options) {
  search.hidden = true;
  quick.hidden = true;
  status.textContent = t("hint_offer");
  list.replaceChildren();
  const box = el("div", "offer");
  box.append(
    el("div", "offer-title", t("engine_title", { engine: engine.title })),
    el("p", "", t("engine_text", { action: label(a), engine: `${engine.title} ${engine.version}`, size: formatSize(engine.size) })),
    el("p", "note", t("engine_note", { license: engine.license })),
  );
  const go = el("button", "primary", `${t("engine_download")}  ↵`);
  const back = el("button", "back", `‹ ${t("cancel")}`);
  const paths = files.map((f) => f.path);
  offer = async () => {
    offer = null;
    await hide();
    try {
      await invoke("install_engine", { engine: engine.name });
      engine.installed = true;
      // Then do what the user asked for in the first place.
      await invoke("run_action", { action: a.id, paths, options: options ?? null });
    } catch {
      // The toast shows what went wrong.
    }
  };
  go.onclick = () => offer?.();
  back.onclick = () => {
    offer = null;
    closeConfig();
  };
  box.append(go, back);
  list.append(box);
}

async function hide() {
  if (inTauri) await getCurrentWindow().hide();
}

function open(next: FileInfo[], prefs: Settings, engineList: EngineInfo[], from = "selection") {
  files = next;
  source = from;
  settings = prefs;
  engines = engineList;
  offer = null;
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
  // Inside a time field, editing keys belong to the field.
  const typing = document.activeElement instanceof HTMLInputElement;
  if (e.key === "Escape" || (e.key === "Backspace" && !typing)) closeConfig();
  else if (e.key === "ArrowDown" || e.key === "ArrowUp") focusField((config!.field + (e.key === "ArrowDown" ? 1 : -1) + n) % n);
  else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && !typing) step(e.key === "ArrowRight" ? 1 : -1);
  else if (e.key === "Enter") runConfig();
  else return;
  e.preventDefault();
}

document.addEventListener("keydown", (e) => {
  if (offer) {
    if (e.key === "Enter") offer();
    else if (e.key === "Escape") {
      offer = null;
      closeConfig();
    } else return;
    e.preventDefault();
    return;
  }
  if (config) return configKeys(e);
  const current = visible[selected];
  const caretAtEnd = search.selectionStart === search.value.length;
  if (e.key === "Escape") {
    e.preventDefault();
    hide();
  } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    select(selected + (e.key === "ArrowDown" ? 1 : -1));
  } else if (current?.options?.length && !current.window && ((e.key === "Enter" && e.shiftKey) || (e.key === "ArrowRight" && caretAtEnd))) {
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

/** The tray menu is drawn by Rust, which does not know the system language:
 * the menu window, loaded at startup, sends it the translated labels. */
let update: Update | null = null;

function syncTray(prefs: Settings) {
  setLanguage(prefs.language);
  invoke("set_tray_language", {
    settings: t("tray_settings"),
    quit: t("tray_quit"),
  });
  invoke("set_tray_update", { text: update ? t("update_available", { v: update.version }) : null });
}

if (inTauri) {
  invoke<Settings>("get_settings").then(syncTray);
  listen<Settings>("settings", (e) => syncTray(e.payload));
  listen<Update | null>("update", (e) => {
    update = e.payload;
    invoke<Settings>("get_settings").then(syncTray);
  });
  listen<{ files: FileInfo[]; source: string; settings: Settings; engines: EngineInfo[] }>("menu:open", (e) =>
    open(e.payload.files, e.payload.settings, e.payload.engines, e.payload.source),
  );
} else {
  // Plain browser (npm run dev): sample data for working on the layout.
  const demo: Record<string, FileInfo[]> = {
    video: [{ path: "/tmp/video.mp4", name: "video.mp4", ext: "mp4", kind: "video", size: 38_000_000 }],
    image: [{ path: "/tmp/IMG_2041.heic", name: "IMG_2041.heic", ext: "heic", kind: "image", size: 2_900_000 }],
    mixed: [
      { path: "/tmp/a.png", name: "a.png", ext: "png", kind: "image", size: 120_000 },
      { path: "/tmp/b.mp3", name: "b.mp3", ext: "mp3", kind: "audio", size: 4_000_000 },
    ],
    pdf: [{ path: "/tmp/report.pdf", name: "report.pdf", ext: "pdf", kind: "pdf", size: 5_300_000 }],
    book: [{ path: "/tmp/book.fb2", name: "book.fb2", ext: "fb2", kind: "markup", size: 900_000 }],
    none: [],
  };
  const demoEngines: EngineInfo[] = [
    { name: "mupdf", title: "MuPDF", version: "1.28.5", license: "AGPL-3.0", purpose: "", size: 94_292_330, available: true, installed: false },
    { name: "pandoc", title: "Pandoc", version: "3.13", license: "GPL-2.0-or-later", purpose: "", size: 42_983_200, available: true, installed: true },
  ];
  const params = new URLSearchParams(location.search);
  const demoSettings: Settings = {
    ...DEFAULT_SETTINGS,
    language: params.get("lang") ?? "auto",
    hotkeys: { preset_discord: "Control+Alt+KeyD" },
    custom: [
      { id: "d1", name: "Discord без звука", kind: "video", steps: [{ action: "mute" }, { action: "preset_discord" }] },
    ],
  };
  const name = params.get("demo") ?? "video";
  const clip = name === "clipboard";
  const shot = { path: "/tmp/clipboard-1.png", name: "clipboard-1.png", ext: "png", kind: "image" as const, size: 420_000 };
  open(clip ? [shot] : (demo[name] ?? []), demoSettings, demoEngines, clip ? "clipboard_image" : "selection");
  const opts = visible.find((a) => a.id === params.get("options"));
  if (opts) openConfig(opts);
}
