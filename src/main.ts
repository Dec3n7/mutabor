import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Action, FileInfo, actionsFor, notSupportedYet, quickFor, suggest } from "./actions";
import { formatSize, t } from "./i18n";

const inTauri = "__TAURI_INTERNALS__" in window;

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const header = $<HTMLElement>("#header");
const quick = $<HTMLElement>("#quick");
const list = $<HTMLElement>("#list");
const search = $<HTMLInputElement>("#search");
const status = $<HTMLElement>("#status");

let files: FileInfo[] = [];
let visible: Action[] = [];
let selected = 0;

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
      row.append(el("span", "key", idx < 9 ? String(idx + 1) : ""));
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

async function run(a: Action) {
  try {
    await invoke("run_action", { action: a.id, paths: files.map((f) => f.path) });
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

function open(next: FileInfo[]) {
  files = next;
  selected = 0;
  search.value = "";
  status.textContent = t("hint");
  renderHeader();
  renderList();
  search.focus();
}

search.placeholder = t("search");
search.addEventListener("input", () => {
  selected = 0;
  renderList();
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    hide();
  } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    select(selected + (e.key === "ArrowDown" ? 1 : -1));
  } else if (e.key === "Enter" && visible[selected]) {
    e.preventDefault();
    // TODO: Shift+Enter opens the quality settings submenu.
    run(visible[selected]);
  } else if (/^[1-9]$/.test(e.key) && search.value === "") {
    const a = visible[Number(e.key) - 1];
    if (a) {
      e.preventDefault();
      run(a);
    }
  }
});

if (inTauri) {
  listen<{ files: FileInfo[] }>("menu:open", (e) => open(e.payload.files));
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
  open(demo[new URLSearchParams(location.search).get("demo") ?? "video"] ?? []);
}
