// Mirror of `settings::Settings` on the Rust side.

import type { Options, Quality } from "./actions";

export interface Settings {
  hotkey: string;
  language: string;
  quality: Quality;
  remember: boolean;
  last: Record<string, Options>;
}

export const DEFAULT_SETTINGS: Settings = {
  hotkey: "Control+Alt+KeyC",
  language: "auto",
  quality: "normal",
  remember: true,
  last: {},
};
