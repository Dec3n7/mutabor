// UI strings. Japanese is planned (see spec); add a column per language.

type Lang = "ru" | "en";

const STRINGS: Record<string, Record<Lang, string>> = {
  to_gif: { ru: "В GIF", en: "To GIF" },
  to_mp4: { ru: "В MP4", en: "To MP4" },
  to_webm: { ru: "В WebM", en: "To WebM" },
  to_mov: { ru: "В MOV", en: "To MOV" },
  to_mkv: { ru: "В MKV", en: "To MKV" },
  to_mp3: { ru: "В MP3", en: "To MP3" },
  to_ogg: { ru: "В OGG", en: "To OGG" },
  to_wav: { ru: "В WAV", en: "To WAV" },
  to_flac: { ru: "В FLAC", en: "To FLAC" },
  to_m4a: { ru: "В M4A", en: "To M4A" },
  to_opus: { ru: "В Opus", en: "To Opus" },
  to_jpg: { ru: "В JPG", en: "To JPG" },
  to_png: { ru: "В PNG", en: "To PNG" },
  to_webp: { ru: "В WebP", en: "To WebP" },
  to_avif: { ru: "В AVIF", en: "To AVIF" },
  to_ico: { ru: "В ICO", en: "To ICO" },
  to_pdf: { ru: "В PDF", en: "To PDF" },
  to_md: { ru: "В Markdown", en: "To Markdown" },
  to_html: { ru: "В HTML", en: "To HTML" },
  to_epub: { ru: "В EPUB", en: "To EPUB" },
  to_fb2: { ru: "В FB2", en: "To FB2" },
  to_txt: { ru: "В TXT", en: "To TXT" },
  extract_mp3: { ru: "Извлечь звук (MP3)", en: "Extract audio (MP3)" },
  compress: { ru: "Сжать", en: "Compress" },
  trim: { ru: "Обрезать по времени", en: "Trim" },
  resize: { ru: "Изменить размер", en: "Resize" },
  crop: { ru: "Обрезать", en: "Crop" },
  strip_meta: { ru: "Удалить метаданные (GPS)", en: "Remove metadata (GPS)" },
  normalize: { ru: "Выровнять громкость", en: "Normalize loudness" },
  preset_telegram: { ru: "Для Telegram", en: "For Telegram" },
  preset_discord: { ru: "Для Discord (≤10 МБ)", en: "For Discord (≤10 MB)" },
  pages_png: { ru: "Страницы в PNG", en: "Pages to PNG" },
  pages_jpg: { ru: "Страницы в JPG", en: "Pages to JPG" },
  extract_text: { ru: "Извлечь текст", en: "Extract text" },
  rotate_left: { ru: "Повернуть на 90° влево", en: "Rotate 90° left" },
  rotate_right: { ru: "Повернуть на 90° вправо", en: "Rotate 90° right" },
  flip_h: { ru: "Отразить по горизонтали", en: "Flip horizontally" },
  flip_v: { ru: "Отразить по вертикали", en: "Flip vertically" },

  suggested: { ru: "Подсказка", en: "Suggested" },
  all_actions: { ru: "Действия", en: "Actions" },
  search: { ru: "Поиск действия…", en: "Search actions…" },
  hint: { ru: "↑↓ Enter · ⇧Enter — параметры · Esc", en: "↑↓ Enter · ⇧Enter options · Esc" },
  hint_options: { ru: "↑↓ ←→ · Enter — запустить · Esc — назад", en: "↑↓ ←→ · Enter run · Esc back" },
  options_of: { ru: "Параметры: {name}", en: "Options: {name}" },
  run: { ru: "Запустить", en: "Run" },
  opt_quality: { ru: "Качество", en: "Quality" },
  opt_height: { ru: "Разрешение", en: "Resolution" },
  opt_width: { ru: "Ширина", en: "Width" },
  opt_fps: { ru: "Кадров в секунду", en: "Frames per second" },
  q_high: { ru: "Лучше", en: "Best" },
  q_normal: { ru: "Обычное", en: "Normal" },
  q_small: { ru: "Меньше файл", en: "Smaller" },
  original: { ru: "Как есть", en: "Original" },
  settings: { ru: "Настройки", en: "Settings" },
  files_n: { ru: "Файлов: {n}", en: "{n} files" },
  nothing_selected: {
    ru: "Выделите файл в Проводнике или Finder и нажмите хоткей ещё раз.",
    en: "Select a file in Explorer or Finder and press the hotkey again.",
  },
  no_common: { ru: "Для этих файлов нет общих действий.", en: "No actions fit all selected files." },
  not_supported_yet: {
    ru: "Этот тип файлов пока не поддерживается — появится в следующих версиях.",
    en: "This file type isn't supported yet; it's coming in a later version.",
  },
  no_match: { ru: "Ничего не найдено", en: "No matches" },
  not_ready: { ru: "Это действие ещё не реализовано", en: "This action is not implemented yet" },
  no_audio: { ru: "В этом файле нет звуковой дорожки", en: "This file has no audio track" },
  no_duration: { ru: "Не удалось определить длительность файла", en: "Couldn't read the file's duration" },
  start_failed: { ru: "Не удалось запустить: {msg}", en: "Couldn't start: {msg}" },
  converting: { ru: "Конвертация…", en: "Converting…" },
  cancel: { ru: "Отмена", en: "Cancel" },
  cancelled: { ru: "Отменено", en: "Cancelled" },
  failed: { ru: "Ошибка конвертации", en: "Conversion failed" },
  show_in_folder: { ru: "Показать в папке", en: "Show in folder" },

  // Settings window
  s_title: { ru: "Настройки Mutabor", en: "Mutabor settings" },
  s_hotkey: { ru: "Сочетание клавиш", en: "Hotkey" },
  s_hotkey_note: {
    ru: "Открывает меню для файлов, выделенных в Проводнике или Finder.",
    en: "Opens the menu for the files selected in Explorer or Finder.",
  },
  s_hotkey_press: { ru: "Нажмите сочетание… (Esc — отмена)", en: "Press a combination… (Esc cancels)" },
  s_hotkey_reset: { ru: "По умолчанию", en: "Default" },
  s_hotkey_need_mod: {
    ru: "Нужен хотя бы один из Ctrl, Alt, Win/⌘ и обычная клавиша.",
    en: "Use at least one of Ctrl, Alt, Win/⌘ plus a regular key.",
  },
  hotkey_taken: {
    ru: "Это сочетание занято другой программой.",
    en: "Another program already uses this combination.",
  },
  hotkey_invalid: { ru: "Такое сочетание не поддерживается.", en: "This combination isn't supported." },
  s_language: { ru: "Язык", en: "Language" },
  s_lang_auto: { ru: "Как в системе", en: "System" },
  s_quality: { ru: "Качество по умолчанию", en: "Default quality" },
  s_quality_note: {
    ru: "Для конвертации без открытия параметров. «Сжать» всегда стремится к меньшему файлу.",
    en: "Used when you run an action without opening its options. Compress always aims small.",
  },
  s_remember: { ru: "Запоминать параметры, выбранные для действия", en: "Remember the options chosen for each action" },
  s_forget: { ru: "Забыть выбранное ({n})", en: "Forget saved choices ({n})" },
  s_saved: { ru: "Сохранено", en: "Saved" },
  s_save_failed: { ru: "Не удалось сохранить: {msg}", en: "Couldn't save: {msg}" },
  s_about: {
    ru: "Mutabor {v} · GPL-3.0 · работает без интернета и ничего не отправляет.",
    en: "Mutabor {v} · GPL-3.0 · works offline and sends nothing.",
  },
  s_source: { ru: "Исходный код", en: "Source code" },
};

const systemLang: Lang = navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
let lang: Lang = systemLang;

/** Applies the language setting: "auto" follows the system. */
export function setLanguage(pref: string) {
  lang = pref === "ru" || pref === "en" ? pref : systemLang;
  document.documentElement.lang = lang;
}

export function t(key: string, vars: Record<string, string | number> = {}): string {
  const s = STRINGS[key]?.[lang] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}

export function formatSize(bytes: number): string {
  const units = lang === "ru" ? ["Б", "КБ", "МБ", "ГБ"] : ["B", "KB", "MB", "GB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : 1).replace(".", lang === "ru" ? "," : ".")} ${units[i]}`;
}
