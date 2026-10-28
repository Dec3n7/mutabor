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
  fit_size: { ru: "Сжать до размера (МБ)", en: "Compress to size (MB)" },
  mute: { ru: "Убрать звук", en: "Remove sound" },
  speed: { ru: "Изменить скорость", en: "Change speed" },
  frame: { ru: "Кадр в PNG", en: "Frame to PNG" },
  ringtone: { ru: "Рингтон для iPhone (M4R)", en: "iPhone ringtone (M4R)" },
  voice: { ru: "Голосовое для Telegram", en: "Telegram voice message" },
  save: { ru: "Сохранить как PNG", en: "Save as PNG" },
  my_actions: { ru: "Мои действия", en: "My actions" },
  clipboard_image: { ru: "Картинка из буфера обмена", en: "Picture from the clipboard" },
  from_clipboard: { ru: "из буфера обмена", en: "from the clipboard" },
  mb_n: { ru: "{n} МБ", en: "{n} MB" },
  opt_mb: { ru: "Не больше", en: "At most" },
  opt_speed: { ru: "Скорость", en: "Speed" },
  opt_at: { ru: "Момент", en: "At" },
  engine_missing: { ru: "Сначала скачайте движок {engine} (в меню или в настройках)", en: "Download the {engine} engine first (menu or Settings)" },
  folder_not_last: {
    ru: "Шаг, который делает папку страниц, может быть только последним",
    en: "A step that makes a folder of pages must be the last one",
  },
  queued: { ru: "В очереди", en: "Queued" },
  queued_n: { ru: "в очереди: {n}", en: "{n} queued" },
  done_of: { ru: "готово {n} из {total}", en: "{n} of {total} done" },
  errors_n: { ru: "ошибок: {n}", en: "{n} failed" },
  copy: { ru: "Копировать", en: "Copy" },
  copied: { ru: "В буфере ✓", en: "Copied ✓" },
  copy_failed: { ru: "Не удалось скопировать", en: "Couldn't copy" },
  undo: { ru: "Отменить", en: "Undo" },
  undone: { ru: "Результат удалён", en: "Result deleted" },
  preset_discord: { ru: "Для Discord (≤10 МБ)", en: "For Discord (≤10 MB)" },
  pages_png: { ru: "Страницы в PNG", en: "Pages to PNG" },
  pages_jpg: { ru: "Страницы в JPG", en: "Pages to JPG" },
  extract_text: { ru: "Извлечь текст (TXT)", en: "Extract text (TXT)" },
  merge: { ru: "Объединить в один PDF", en: "Merge into one PDF" },
  split: { ru: "Разделить по страницам", en: "Split into pages" },
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
  opt_start: { ru: "Начало", en: "Start" },
  opt_end: { ru: "Конец", en: "End" },
  opt_side: { ru: "Длинная сторона не больше", en: "Longest side at most" },
  opt_ratio: { ru: "Пропорции (по центру)", en: "Aspect ratio (centred)" },
  opt_page: { ru: "Страница", en: "Page" },
  opt_dpi: { ru: "Разрешение (точек на дюйм)", en: "Resolution (dpi)" },
  hint_offer: { ru: "Enter — скачать · Esc — назад", en: "Enter download · Esc back" },
  needs_engine: { ru: "Нужно скачать {engine}", en: "Needs {engine} (download)" },
  engine_title: { ru: "Нужен движок {engine}", en: "{engine} is needed" },
  engine_text: {
    ru: "«{action}» работает через {engine}. Он скачается один раз с официального сайта ({size}), после проверки подписи SHA-256 действие выполнится.",
    en: "“{action}” runs on {engine}. It is downloaded once from the official site ({size}); after the SHA-256 check the action runs.",
  },
  engine_note: {
    ru: "Открытый код, лицензия {license}. Удалить можно в настройках.",
    en: "Open source, {license} licence. You can remove it in Settings.",
  },
  engine_download: { ru: "Скачать и выполнить", en: "Download and run" },
  engine_installed: { ru: "Движок установлен", en: "Engine installed" },
  s_engines: { ru: "Движки", en: "Engines" },
  s_engines_note: {
    ru: "Скачиваются по требованию с официальных сайтов и проверяются по SHA-256. FFmpeg встроен.",
    en: "Downloaded on demand from the official sites and checked by SHA-256. FFmpeg is built in.",
  },
  purpose_mupdf: { ru: "PDF: страницы в картинки, текст, сжатие, объединение", en: "PDF: pages to images, text, compress, merge" },
  purpose_pandoc: { ru: "Markdown, HTML, EPUB, FB2, TXT", en: "Markdown, HTML, EPUB, FB2, TXT" },
  s_engine_install: { ru: "Скачать", en: "Download" },
  s_engine_remove: { ru: "Удалить", en: "Remove" },
  s_engine_installed: { ru: "установлен", en: "installed" },
  s_engine_unavailable: { ru: "нет сборки для этой системы", en: "no build for this system" },
  s_engine_busy: { ru: "загрузка…", en: "downloading…" },
  page_a4: { ru: "A4", en: "A4" },
  page_original: { ru: "По размеру картинки", en: "Image size" },
  to_end: { ru: "до конца", en: "to the end" },
  bad_time: { ru: "Время пишется так: 1:30 или 0:05.5", en: "Write times like 1:30 or 0:05.5" },
  bad_range: {
    ru: "Неверный отрезок: начало должно быть раньше конца и в пределах файла",
    en: "Invalid range: the start must come before the end and within the file",
  },
  q_high: { ru: "Лучше", en: "Best" },
  q_normal: { ru: "Обычное", en: "Normal" },
  q_small: { ru: "Меньше файл", en: "Smaller" },
  original: { ru: "Как есть", en: "Original" },
  settings: { ru: "Настройки", en: "Settings" },
  files_n: { ru: "Файлов: {n}", en: "{n} files" },
  nothing_selected: {
    ru: "Выделите файлы в Проводнике или Finder (или скопируйте файл либо скриншот) и нажмите хоткей ещё раз.",
    en: "Select files in Explorer or Finder (or copy a file or a screenshot) and press the hotkey again.",
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
  hotkey_duplicate: { ru: "Это сочетание уже назначено другому действию.", en: "This combination is already assigned." },
  s_copy_result: { ru: "Класть результат в буфер обмена", en: "Put the result on the clipboard" },
  s_copy_result_note: {
    ru: "После конвертации готовые файлы сразу можно вставить в чат через Ctrl+V.",
    en: "After converting, paste the files straight into a chat with Ctrl+V.",
  },
  s_queue: { ru: "Одновременно конвертировать", en: "Convert at the same time" },
  s_queue_note: {
    ru: "Остальные выделенные файлы ждут в очереди. Видео нагружает процессор сильнее всего.",
    en: "Other selected files wait in a queue. Video is the heaviest on the processor.",
  },
  kind_video: { ru: "Видео", en: "Video" },
  kind_audio: { ru: "Аудио", en: "Audio" },
  kind_image: { ru: "Картинки", en: "Pictures" },
  kind_pdf: { ru: "PDF", en: "PDF" },
  kind_markup: { ru: "Книги и разметка", en: "Books and markup" },
  s_action_hotkeys: { ru: "Хоткеи для действий", en: "Action hotkeys" },
  s_action_hotkeys_note: {
    ru: "Сочетание запускает действие сразу, без меню, с последними выбранными параметрами.",
    en: "The combination runs the action at once, without the menu, with the last chosen options.",
  },
  s_assign: { ru: "Назначить сочетание", en: "Assign a combination" },
  s_remove: { ru: "Убрать", en: "Remove" },
  s_custom: { ru: "Мои действия", en: "My actions" },
  s_custom_note: {
    ru: "Цепочка шагов: каждый работает с результатом предыдущего. Можно вписать свою команду ffmpeg. Экспорт копирует JSON, им можно поделиться.",
    en: "A chain of steps, each working on the previous result. A step can be your own ffmpeg command. Export copies JSON you can share.",
  },
  s_custom_new: { ru: "+ Новое действие", en: "+ New action" },
  s_custom_name: { ru: "Название", en: "Name" },
  s_custom_incomplete: { ru: "Нужны название и хотя бы один заполненный шаг", en: "Needs a name and at least one complete step" },
  s_edit: { ru: "Изменить", en: "Edit" },
  s_step_add: { ru: "+ Шаг", en: "+ Step" },
  s_step_ffmpeg: { ru: "+ Команда ffmpeg", en: "+ ffmpeg command" },
  s_save: { ru: "Сохранить", en: "Save" },
  s_export: { ru: "Экспорт", en: "Export" },
  s_import: { ru: "Импорт", en: "Import" },
  s_import_go: { ru: "Добавить из JSON", en: "Add from JSON" },
  s_exported: { ru: "Скопировано в буфер: {n}", en: "Copied to the clipboard: {n}" },
  s_imported: { ru: "Добавлено действий: {n}", en: "Actions added: {n}" },
  s_import_none: { ru: "В JSON нет подходящих действий", en: "No usable actions in this JSON" },
  s_import_bad: { ru: "Это не JSON", en: "This isn't JSON" },
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
