// Mirror of `settings::Settings` on the Rust side, and hotkey helpers
// shared by the menu and the settings window.

import type { CustomAction, Options, Quality } from "./actions";

export interface Settings {
  hotkey: string;
  language: string;
  quality: Quality;
  remember: boolean;
  last: Record<string, Options>;
  copy_result: boolean;
  /** Files of each kind converted at the same time. */
  parallel: Record<"video" | "audio" | "image" | "pdf" | "markup", number>;
  /** Action id (or "custom:<id>") -> shortcut. */
  hotkeys: Record<string, string>;
  custom: CustomAction[];
  check_updates: boolean;
  update_seen: string;
}

/** Mirror of `updates::Update`: a newer release on GitHub. */
export interface Update {
  version: string;
  url: string;
}

export const isMac = navigator.userAgent.includes("Mac");

export const DEFAULT_SETTINGS: Settings = {
  hotkey: isMac ? "Super+Alt+KeyC" : "Control+Alt+KeyC",
  language: "auto",
  quality: "normal",
  remember: true,
  last: {},
  copy_result: false,
  parallel: { video: 2, audio: 4, image: 4, pdf: 2, markup: 2 },
  hotkeys: {},
  custom: [],
  check_updates: true,
  update_seen: "",
};

const MOD_NAMES: Record<string, string> = isMac
  ? { Control: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" }
  : { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Super: "Win" };

/** "Control+Alt+KeyC" -> "Ctrl+Alt+C" (⌃⌥C on a Mac). */
export function showHotkey(spec: string): string {
  return spec
    .split("+")
    .map((p) => MOD_NAMES[p] ?? p.replace(/^Key|^Digit/, "").replace(/^Arrow/, ""))
    .join(isMac ? "" : "+");
}

const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta", "OS"];

export const isModifier = (e: KeyboardEvent) => MODIFIER_KEYS.includes(e.key);

/** The global-hotkey spec for a key press, or null without Ctrl, Alt or Win/⌘. */
export function specOf(e: KeyboardEvent): string | null {
  if (isModifier(e) || (!e.ctrlKey && !e.altKey && !e.metaKey)) return null;
  const mods = [e.ctrlKey && "Control", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"];
  return [...mods.filter(Boolean), e.code].join("+");
}
