// UI strings. Each language is one file in ./locales with the same keys as
// en.ts; `npm run check-locales` finds missing keys and placeholders.
// To add a language: copy en.ts, translate the values, list it below.

import de from "./locales/de";
import en from "./locales/en";
import es from "./locales/es";
import fr from "./locales/fr";
import it from "./locales/it";
import ja from "./locales/ja";
import ko from "./locales/ko";
import pl from "./locales/pl";
import pt from "./locales/pt";
import ru from "./locales/ru";
import tr from "./locales/tr";
import uk from "./locales/uk";
import zh from "./locales/zh";

type Dict = Record<string, string>;

/** Language code -> its own name, in the order the settings list them. */
export const LANGUAGES: Record<string, string> = {
  en: "English",
  ru: "Русский",
  uk: "Українська",
  es: "Español",
  pt: "Português (Brasil)",
  de: "Deutsch",
  fr: "Français",
  it: "Italiano",
  pl: "Polski",
  tr: "Türkçe",
  zh: "简体中文",
  ja: "日本語",
  ko: "한국어",
};

const DICTS: Record<string, Dict> = { en, ru, uk, es, pt, de, fr, it, pl, tr, zh, ja, ko };

/** The first system language Mutabor speaks ("pt-BR" -> "pt"), else English. */
function systemLanguage(): string {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const code = tag.toLowerCase().split("-")[0];
    if (code in DICTS) return code;
  }
  return "en";
}

let lang = systemLanguage();

/** Applies the language setting: "auto" follows the system. */
export function setLanguage(pref: string) {
  lang = pref in DICTS ? pref : systemLanguage();
  document.documentElement.lang = lang;
}

export function t(key: string, vars: Record<string, string | number> = {}): string {
  const s = DICTS[lang][key] ?? (en as Dict)[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}

export function formatSize(bytes: number): string {
  const units = [t("unit_b"), t("unit_kb"), t("unit_mb"), t("unit_gb")];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 ? 0 : 1;
  const n = new Intl.NumberFormat(lang, { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(v);
  return `${n} ${units[i]}`;
}
