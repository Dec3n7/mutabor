import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Quality } from "./actions";
import { setLanguage, t } from "./i18n";
import { DEFAULT_SETTINGS, Settings } from "./prefs";

const inTauri = "__TAURI_INTERNALS__" in window;
const isMac = navigator.userAgent.includes("Mac");
const REPO = "https://github.com/Dec3n7/mutabor";

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const hotkey = $<HTMLButtonElement>("#hotkey");
const hotkeyReset = $<HTMLButtonElement>("#hotkey-reset");
const language = $<HTMLSelectElement>("#language");
const quality = $<HTMLElement>("#quality");
const remember = $<HTMLInputElement>("#remember");
const forget = $<HTMLButtonElement>("#forget");
const about = $<HTMLElement>("#about");
const saved = $<HTMLElement>("#saved");

let settings: Settings = { ...DEFAULT_SETTINGS, hotkey: isMac ? "Super+Alt+KeyC" : DEFAULT_SETTINGS.hotkey };
let version = "";
let recording = false;

const MOD_NAMES: Record<string, string> = isMac
  ? { Control: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" }
  : { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Super: "Win" };

/** "Control+Alt+KeyC" -> "Ctrl + Alt + C". */
function showHotkey(spec: string): string {
  return spec
    .split("+")
    .map((p) => MOD_NAMES[p] ?? p.replace(/^Key|^Digit/, "").replace(/^Arrow/, ""))
    .join(isMac ? "" : " + ");
}

/** Builds the global-hotkey spec from a key press, or null for a bare key. */
function specOf(e: KeyboardEvent): string | null {
  if (["Control", "Alt", "Shift", "Meta", "OS"].includes(e.key)) return null;
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null;
  const mods = [e.ctrlKey && "Control", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"];
  return [...mods.filter(Boolean), e.code].join("+");
}

function render() {
  setLanguage(settings.language);
  document.querySelectorAll<HTMLElement>("[data-t]").forEach((n) => (n.textContent = t(n.dataset.t!)));
  document.title = t("s_title");
  hotkey.textContent = recording ? t("s_hotkey_press") : showHotkey(settings.hotkey);
  hotkey.classList.toggle("recording", recording);
  language.value = settings.language;

  quality.replaceChildren();
  for (const q of ["high", "normal", "small"] as Quality[]) {
    const chip = document.createElement("button");
    chip.className = "chip" + (settings.quality === q ? " on" : "");
    chip.textContent = t(`q_${q}`);
    chip.onclick = () => save({ ...settings, quality: q });
    quality.append(chip);
  }

  remember.checked = settings.remember;
  const n = Object.keys(settings.last).length;
  forget.textContent = t("s_forget", { n });
  forget.hidden = n === 0;

  about.replaceChildren(t("s_about", { v: version }), " · ");
  const link = document.createElement("button");
  link.className = "link";
  link.textContent = t("s_source");
  link.onclick = () => (inTauri ? openUrl(REPO) : window.open(REPO));
  about.append(link);
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

hotkey.onclick = () => {
  recording = true;
  render();
};
hotkey.onblur = () => {
  recording = false;
  render();
};
hotkeyReset.onclick = () => save({ ...settings, hotkey: isMac ? "Super+Alt+KeyC" : "Control+Alt+KeyC" });

document.addEventListener("keydown", (e) => {
  if (!recording) return;
  e.preventDefault();
  if (e.key === "Escape") {
    recording = false;
    render();
    return;
  }
  const spec = specOf(e);
  if (!spec) {
    if (!["Control", "Alt", "Shift", "Meta", "OS"].includes(e.key)) flash(t("s_hotkey_need_mod"), true);
    return;
  }
  recording = false;
  save({ ...settings, hotkey: spec });
});

language.onchange = () => save({ ...settings, language: language.value });
remember.onchange = () => save({ ...settings, remember: remember.checked });
forget.onclick = () => save({ ...settings, last: {} });

async function init() {
  if (inTauri) {
    version = await getVersion();
    settings = await invoke<Settings>("get_settings");
    // The menu remembers choices too; keep this window in sync.
    listen<Settings>("settings", (e) => {
      settings = e.payload;
      render();
    });
  }
  render();
}
init();
