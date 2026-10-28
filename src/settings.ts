import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Action,
  CustomAction,
  EngineInfo,
  Field,
  Kind,
  Options,
  Quality,
  TIME_FIELDS,
  catalog,
  chipValue,
  choicesFor,
  setChip,
} from "./actions";
import { LANGUAGES, formatSize, setLanguage, t } from "./i18n";
import { DEFAULT_SETTINGS, Settings, Update, isModifier, showHotkey, specOf } from "./prefs";

const inTauri = "__TAURI_INTERNALS__" in window;
const REPO = "https://github.com/Dec3n7/mutabor";
const KINDS: Exclude<Kind, "unknown">[] = ["video", "audio", "image", "pdf", "markup"];

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const hotkey = $<HTMLButtonElement>("#hotkey");
const hotkeyReset = $<HTMLButtonElement>("#hotkey-reset");
const language = $<HTMLSelectElement>("#language");
const quality = $<HTMLElement>("#quality");
const remember = $<HTMLInputElement>("#remember");
const forget = $<HTMLButtonElement>("#forget");
const copyResult = $<HTMLInputElement>("#copy-result");
const parallel = $<HTMLElement>("#parallel");
const actionHotkeys = $<HTMLElement>("#action-hotkeys");
const hotkeyAction = $<HTMLSelectElement>("#hotkey-action");
const hotkeyAdd = $<HTMLButtonElement>("#hotkey-add");
const customList = $<HTMLElement>("#custom");
const editor = $<HTMLElement>("#editor");
const importBox = $<HTMLElement>("#import-box");
const importText = $<HTMLTextAreaElement>("#import-text");
const about = $<HTMLElement>("#about");
const saved = $<HTMLElement>("#saved");
const enginesBox = $<HTMLElement>("#engines");
const checkUpdates = $<HTMLInputElement>("#check-updates");
const updateState = $<HTMLElement>("#update-state");

let settings: Settings = DEFAULT_SETTINGS;
let version = "";
let engines: EngineInfo[] = [];
let update: Update | null = null;
/** Result of the last "Check now", shown until the window reloads. */
let checked: "" | "busy" | "current" | "failed" = "";
const busy = new Set<string>();
/** What a key press is being recorded for: "menu" or an action id. */
let recording: string | null = null;
/** The user action being edited, a working copy. */
let draft: CustomAction | null = null;

function el(tag: string, cls = "", text = ""): HTMLElement {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  e.textContent = text;
  return e;
}

function button(text: string, onClick: () => void, cls = "link"): HTMLButtonElement {
  const b = el("button", cls, text) as HTMLButtonElement;
  b.onclick = onClick;
  return b;
}

/** Built-in actions and the user's own ones, for pickers. */
function allActions(): Action[] {
  return [...catalog(), ...settings.custom.map((c) => ({ id: `custom:${c.id}`, label: c.name }))];
}

const actionLabel = (id: string) => allActions().find((a) => a.id === id)?.label ?? t(id);

function render() {
  setLanguage(settings.language);
  document.querySelectorAll<HTMLElement>("[data-t]").forEach((n) => (n.textContent = t(n.dataset.t!)));
  document.title = t("s_title");
  hotkey.textContent = recording === "menu" ? t("s_hotkey_press") : showHotkey(settings.hotkey);
  hotkey.classList.toggle("recording", recording === "menu");
  if (language.options.length === 1) {
    for (const [code, name] of Object.entries(LANGUAGES)) language.add(new Option(name, code));
  }
  language.value = settings.language;

  quality.replaceChildren();
  for (const q of ["high", "normal", "small"] as Quality[]) {
    const chip = button(t(`q_${q}`), () => save({ ...settings, quality: q }), "chip" + (settings.quality === q ? " on" : ""));
    quality.append(chip);
  }

  remember.checked = settings.remember;
  const n = Object.keys(settings.last).length;
  forget.textContent = t("s_forget", { n });
  forget.hidden = n === 0;
  copyResult.checked = settings.copy_result;
  checkUpdates.checked = settings.check_updates;
  renderUpdate();

  renderParallel();
  renderActionHotkeys();
  renderCustom();
  renderEngines();

  about.replaceChildren(t("s_about", { v: version }), " · ");
  about.append(button(t("s_source"), () => (inTauri ? openUrl(REPO) : window.open(REPO))));
}

function renderUpdate() {
  updateState.replaceChildren();
  if (update) {
    updateState.append(
      `${t("update_available", { v: update.version })} · `,
      button(t("update_download"), () => (inTauri ? openUrl(update!.url) : window.open(update!.url))),
    );
  } else if (checked === "busy") updateState.textContent = t("s_checking");
  else if (checked === "current") updateState.textContent = t("s_up_to_date", { v: version });
  else if (checked === "failed") updateState.textContent = t("s_check_failed");
}

function renderParallel() {
  parallel.replaceChildren();
  for (const k of KINDS) {
    const n = settings.parallel[k];
    const set = (v: number) => save({ ...settings, parallel: { ...settings.parallel, [k]: Math.min(16, Math.max(1, v)) } });
    const stepper = el("span", "stepper");
    stepper.append(button("−", () => set(n - 1), "stepbtn"), el("span", "count", String(n)), button("+", () => set(n + 1), "stepbtn"));
    parallel.append(el("span", "", t(`kind_${k}`)), stepper);
  }
}

function renderActionHotkeys() {
  actionHotkeys.replaceChildren();
  for (const [id, spec] of Object.entries(settings.hotkeys)) {
    const row = el("div", "engine");
    const key = button(recording === id ? t("s_hotkey_press") : showHotkey(spec), () => record(id), "field small");
    key.classList.toggle("recording", recording === id);
    const remove = button("×", () => {
      const hotkeys = { ...settings.hotkeys };
      delete hotkeys[id];
      save({ ...settings, hotkeys });
    });
    remove.title = t("s_remove");
    const right = el("span", "line");
    right.append(key, remove);
    row.append(el("span", "", actionLabel(id)), right);
    actionHotkeys.append(row);
  }
  // A pending row while the first combination for a new action is recorded.
  if (recording && recording !== "menu" && !settings.hotkeys[recording]) {
    const row = el("div", "engine");
    const key = el("span", "field small recording", t("s_hotkey_press"));
    row.append(el("span", "", actionLabel(recording)), key);
    actionHotkeys.append(row);
  }
  hotkeyAction.replaceChildren();
  for (const a of allActions().filter((a) => !settings.hotkeys[a.id])) {
    const o = document.createElement("option");
    o.value = a.id;
    o.textContent = a.label ?? t(a.id);
    hotkeyAction.append(o);
  }
}

function record(target: string) {
  recording = target;
  render();
}

function stepText(step: CustomAction["steps"][number]): string {
  return "ffmpeg" in step ? `ffmpeg ${step.ffmpeg} → .${step.ext}` : t(step.action);
}

function renderCustom() {
  customList.replaceChildren();
  for (const c of settings.custom) {
    const row = el("div", "engine");
    const text = el("div");
    text.append(el("div", "name", c.name), el("div", "info", `${t(`kind_${c.kind}`)} · ${c.steps.map(stepText).join(" → ")}`));
    const right = el("span", "line");
    right.append(
      button(t("s_edit"), () => {
        draft = structuredClone(c);
        render();
      }),
      button("×", () => {
        const hotkeys = { ...settings.hotkeys };
        delete hotkeys[`custom:${c.id}`];
        save({ ...settings, hotkeys, custom: settings.custom.filter((x) => x.id !== c.id) });
      }),
    );
    row.append(text, right);
    customList.append(row);
  }
  renderEditor();
}

/** Chips for one step's options; time fields are left to the menu. */
function stepOptions(action: Action, options: Options, onChange: () => void): HTMLElement {
  const box = el("div", "step-options");
  for (const f of (action.options ?? []).filter((f: Field) => !TIME_FIELDS.includes(f))) {
    const chips = el("div", "chips");
    for (const v of choicesFor(action, f)) {
      const on = chipValue(options, f) === v;
      chips.append(
        button(
          chipLabel(f, v),
          () => {
            if (on) delete (options as Record<string, unknown>)[f];
            else setChip(options, f, v);
            onChange();
          },
          "chip" + (on ? " on" : ""),
        ),
      );
    }
    box.append(chips);
  }
  return box;
}

function chipLabel(f: Field, v: string | number): string {
  if (f === "quality") return t(`q_${v}`);
  if (f === "page") return t(`page_${v}`);
  if (f === "mb") return t("mb_n", { n: v });
  if (f === "speed") return `×${v}`;
  if (v === 0) return t("original");
  if (f === "height") return `${v}p`;
  if (f === "width" || f === "side") return `${v} px`;
  return String(v);
}

function renderEditor() {
  editor.hidden = !draft;
  editor.replaceChildren();
  if (!draft) return;
  const d = draft;

  const name = document.createElement("input");
  name.className = "field";
  name.placeholder = t("s_custom_name");
  name.value = d.name;
  name.oninput = () => (d.name = name.value);

  const kind = document.createElement("select");
  kind.className = "field";
  for (const k of KINDS) {
    const o = document.createElement("option");
    o.value = k;
    o.textContent = t(`kind_${k}`);
    kind.append(o);
  }
  kind.value = d.kind;
  kind.onchange = () => {
    d.kind = kind.value as Kind;
    renderEditor();
  };
  const head = el("div", "line");
  head.append(name, kind);
  editor.append(head);

  const actions = catalog(d.kind as Kind);
  d.steps.forEach((step, i) => {
    const row = el("div", "step");
    row.append(el("span", "count", `${i + 1}.`));
    const body = el("div", "step-body");
    if ("ffmpeg" in step) {
      const args = document.createElement("input");
      args.className = "field code";
      args.placeholder = "-vf hflip";
      args.value = step.ffmpeg;
      args.oninput = () => (step.ffmpeg = args.value);
      const ext = document.createElement("input");
      ext.className = "field ext";
      ext.placeholder = "mp4";
      ext.value = step.ext;
      ext.oninput = () => (step.ext = ext.value.replace(/^\./, ""));
      const line = el("div", "line");
      line.append(el("span", "", "ffmpeg"), args, el("span", "", "→ ."), ext);
      body.append(line);
    } else {
      const pick = document.createElement("select");
      pick.className = "field";
      for (const a of actions) {
        const o = document.createElement("option");
        o.value = a.id;
        o.textContent = t(a.id);
        pick.append(o);
      }
      pick.value = step.action;
      pick.onchange = () => {
        d.steps[i] = { action: pick.value, options: {} };
        renderEditor();
      };
      body.append(pick);
      const a = actions.find((x) => x.id === step.action);
      step.options ??= {};
      if (a) body.append(stepOptions(a, step.options, renderEditor));
    }
    row.append(body);
    const tools = el("span", "line");
    if (i > 0)
      tools.append(
        button("↑", () => {
          [d.steps[i - 1], d.steps[i]] = [d.steps[i], d.steps[i - 1]];
          renderEditor();
        }),
      );
    tools.append(
      button("×", () => {
        d.steps.splice(i, 1);
        renderEditor();
      }),
    );
    row.append(tools);
    editor.append(row);
  });

  const add = el("div", "line");
  add.append(
    button(t("s_step_add"), () => {
      d.steps.push({ action: actions[0]?.id ?? "compress", options: {} });
      renderEditor();
    }),
    button(t("s_step_ffmpeg"), () => {
      d.steps.push({ ffmpeg: "", ext: d.kind === "audio" ? "mp3" : d.kind === "image" ? "png" : "mp4" });
      renderEditor();
    }),
  );
  editor.append(add);

  const done = el("div", "line");
  done.append(
    button(t("s_save"), () => {
      if (!d.name.trim() || d.steps.length === 0) return flash(t("s_custom_incomplete"), true);
      if (d.steps.some((s) => "ffmpeg" in s && (!s.ffmpeg.trim() || !s.ext.trim())))
        return flash(t("s_custom_incomplete"), true);
      const custom = settings.custom.some((c) => c.id === d.id)
        ? settings.custom.map((c) => (c.id === d.id ? d : c))
        : [...settings.custom, d];
      draft = null;
      save({ ...settings, custom });
    }, "primary small"),
    button(t("cancel"), () => {
      draft = null;
      render();
    }),
  );
  editor.append(done);
}

function newId(): string {
  return Math.random().toString(36).slice(2, 10);
}

/** Keeps only well-formed actions from pasted JSON; ids are renewed. */
function parseImport(text: string): CustomAction[] {
  const data = JSON.parse(text);
  const list: unknown[] = Array.isArray(data) ? data : Array.isArray(data?.custom) ? data.custom : [data];
  const out: CustomAction[] = [];
  for (const item of list) {
    const c = item as CustomAction;
    if (typeof c?.name !== "string" || !KINDS.includes(c.kind as never) || !Array.isArray(c.steps)) continue;
    const steps = c.steps.filter(
      (s) =>
        (typeof (s as { action?: unknown }).action === "string") ||
        (typeof (s as { ffmpeg?: unknown }).ffmpeg === "string" && typeof (s as { ext?: unknown }).ext === "string"),
    );
    if (steps.length) out.push({ id: newId(), name: c.name, kind: c.kind, steps });
  }
  return out;
}

function renderEngines() {
  enginesBox.replaceChildren();
  for (const e of engines) {
    const row = el("div", "engine");
    const text = el("div");
    const state = !e.available
      ? t("s_engine_unavailable")
      : busy.has(e.name)
        ? t("s_engine_busy")
        : e.installed
          ? t("s_engine_installed")
          : formatSize(e.size);
    const purpose = t(`purpose_${e.name}`);
    text.append(
      el("div", "name", `${e.title} ${e.version}`),
      el("div", "info", `${purpose.startsWith("purpose_") ? e.purpose : purpose} · ${e.license} · ${state}`),
    );
    row.append(text);
    if (e.available && !busy.has(e.name)) {
      row.append(
        button(e.installed ? t("s_engine_remove") : t("s_engine_install"), async () => {
          if (!inTauri) return;
          busy.add(e.name);
          render();
          try {
            await invoke(e.installed ? "remove_engine" : "install_engine", { engine: e.name });
          } catch (err) {
            flash(String(err), true);
          }
          busy.delete(e.name);
          engines = await invoke<EngineInfo[]>("list_engines");
          render();
        }),
      );
    }
    enginesBox.append(row);
  }
}

function flash(text: string, bad = false) {
  saved.textContent = text;
  saved.classList.toggle("bad", bad);
}

async function save(next: Settings) {
  try {
    if (inTauri) await invoke("save_settings", { next });
    settings = next;
    flash(t("s_saved"));
  } catch (err) {
    const code = String(err);
    flash(code.startsWith("hotkey_") ? t(code) : t("s_save_failed", { msg: code }), true);
  }
  render();
}

hotkey.onclick = () => record("menu");
hotkeyReset.onclick = () => save({ ...settings, hotkey: DEFAULT_SETTINGS.hotkey });
hotkeyAdd.onclick = () => hotkeyAction.value && record(hotkeyAction.value);

document.addEventListener("keydown", (e) => {
  if (!recording) return;
  e.preventDefault();
  const target = recording;
  if (e.key === "Escape") {
    recording = null;
    render();
    return;
  }
  const spec = specOf(e);
  if (!spec) {
    if (!isModifier(e)) flash(t("s_hotkey_need_mod"), true);
    return;
  }
  recording = null;
  if (target === "menu") save({ ...settings, hotkey: spec });
  else save({ ...settings, hotkeys: { ...settings.hotkeys, [target]: spec } });
});
// Clicking elsewhere stops recording.
document.addEventListener("mousedown", (e) => {
  if (recording && !(e.target as HTMLElement).classList.contains("recording")) {
    recording = null;
    render();
  }
});

language.onchange = () => save({ ...settings, language: language.value });
remember.onchange = () => save({ ...settings, remember: remember.checked });
forget.onclick = () => save({ ...settings, last: {} });
copyResult.onchange = () => save({ ...settings, copy_result: copyResult.checked });
checkUpdates.onchange = () => save({ ...settings, check_updates: checkUpdates.checked });
$<HTMLButtonElement>("#check-now").onclick = async () => {
  checked = "busy";
  renderUpdate();
  try {
    update = inTauri ? await invoke<Update | null>("check_updates_now") : null;
    checked = update ? "" : "current";
  } catch {
    checked = "failed";
  }
  renderUpdate();
};

$<HTMLButtonElement>("#custom-new").onclick = () => {
  draft = { id: newId(), name: "", kind: "video", steps: [{ action: "compress", options: {} }] };
  render();
};
$<HTMLButtonElement>("#custom-export").onclick = async () => {
  const json = JSON.stringify(settings.custom.map(({ name, kind, steps }) => ({ name, kind, steps })), null, 2);
  try {
    await navigator.clipboard.writeText(json);
    flash(t("s_exported", { n: settings.custom.length }));
  } catch {
    importBox.hidden = false;
    importText.value = json;
  }
};
$<HTMLButtonElement>("#custom-import").onclick = () => {
  importBox.hidden = !importBox.hidden;
  importText.focus();
};
$<HTMLButtonElement>("#import-go").onclick = () => {
  try {
    const added = parseImport(importText.value);
    if (added.length === 0) return flash(t("s_import_none"), true);
    importText.value = "";
    importBox.hidden = true;
    save({ ...settings, custom: [...settings.custom, ...added] });
    flash(t("s_imported", { n: added.length }));
  } catch {
    flash(t("s_import_bad"), true);
  }
};

async function init() {
  if (inTauri) {
    version = await getVersion();
    settings = { ...DEFAULT_SETTINGS, ...(await invoke<Settings>("get_settings")) };
    engines = await invoke<EngineInfo[]>("list_engines");
    update = await invoke<Update | null>("get_update");
    listen<Update | null>("update", (e) => {
      update = e.payload;
      renderUpdate();
    });
    listen<EngineInfo[]>("engines", (e) => {
      engines = e.payload;
      render();
    });
    // The menu remembers choices too; keep this window in sync.
    listen<Settings>("settings", (e) => {
      settings = e.payload;
      render();
    });
  }
  render();
}
init();
