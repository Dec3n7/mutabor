// Catalog of menu actions per file kind. Labels live in i18n.ts.

export type Kind = "video" | "audio" | "image" | "pdf" | "markup" | "unknown";

export interface FileInfo {
  path: string;
  name: string;
  ext: string;
  kind: Kind;
  size: number;
}

export interface Action {
  id: string;
  /** Output extension; the action is hidden when the input already has it. */
  target?: string;
  /** Extra search words besides the label. */
  keywords?: string;
  /** Fields of the settings submenu (Shift+Enter); none means it runs as is. */
  options?: Field[];
  /** Always opens the submenu: running without choices makes no sense. */
  ask?: boolean;
  /** Optional engine the action needs; downloaded on first use. */
  engine?: "mupdf" | "pandoc";
  /** Works on the whole selection at once and needs at least two files. */
  multi?: boolean;
  /** Short button text for quick actions; the full label becomes a tooltip. */
  glyph?: string;
  /** Planned but not implemented yet; kept in the catalog, hidden from the menu. */
  soon?: boolean;
}

/** Submenu fields; values mirror `recipes::Options` on the Rust side. */
export type Field = "quality" | "height" | "width" | "fps" | "start" | "end" | "side" | "ratio" | "page" | "dpi";
export type Quality = "high" | "normal" | "small";
export interface Options {
  quality?: Quality;
  height?: number;
  width?: number;
  fps?: number;
  /** Trim range in seconds. */
  start?: number;
  end?: number;
  side?: number;
  ratio?: [number, number];
  page?: "a4" | "original";
  dpi?: number;
}

/** Fields typed as text (a time) rather than picked from chips. */
export const TIME_FIELDS: Field[] = ["start", "end"];

/** Choices per field, in display order. A size of 0 keeps the original. */
export const CHOICES: Partial<Record<Field, (string | number)[]>> = {
  quality: ["high", "normal", "small"],
  height: [0, 2160, 1080, 720, 480],
  width: [0, 720, 480, 320],
  fps: [24, 15, 12, 10],
  side: [3840, 2560, 1920, 1280, 800, 512],
  ratio: ["1:1", "4:3", "3:2", "16:9", "9:16", "4:5"],
  page: ["a4", "original"],
  dpi: [72, 150, 300],
};

/** Choices for a field of this action; resizing has no "original" size. */
export function choicesFor(a: Action, f: Field): (string | number)[] {
  if (a.id === "resize" && f === "height") return [2160, 1440, 1080, 720, 480, 360];
  return CHOICES[f] ?? [];
}

/** Option value as shown in a chip: ratios are kept as [w, h]. */
export function chipValue(o: Options, f: Field): string | number | undefined {
  const v = o[f];
  return Array.isArray(v) ? v.join(":") : v;
}

export function setChip(o: Options, f: Field, v: string | number) {
  const rec = o as Record<string, unknown>;
  rec[f] = f === "ratio" ? String(v).split(":").map(Number) : v;
}

/** "1:02:03.5", "2:30" or "90" -> seconds; "" -> undefined; junk -> NaN. */
export function parseTime(text: string): number | undefined {
  const s = text.trim().replace(",", ".");
  if (s === "") return undefined;
  if (!/^\d+(:\d{1,2}){0,2}(\.\d+)?$/.test(s)) return NaN;
  return s.split(":").reduce((acc, part) => acc * 60 + Number(part), 0);
}

export function formatTime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = +(secs % 60).toFixed(2);
  const ss = (s < 10 ? "0" : "") + s;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

const VIDEO: Field[] = ["quality", "height"];
const TRIM: Field[] = ["start", "end"];
const QUALITY: Field[] = ["quality"];

const convert = (ext: string, keywords = "", options?: Field[]): Action => ({
  id: `to_${ext}`,
  target: ext,
  keywords: `${ext} ${keywords}`,
  options,
});

/** One-click buttons shown above the list. */
export const QUICK: Partial<Record<Kind, Action[]>> = {
  image: [
    { id: "rotate_left", glyph: "↺ 90°" },
    { id: "rotate_right", glyph: "↻ 90°" },
    { id: "flip_h", glyph: "⇆" },
    { id: "flip_v", glyph: "⇅" },
  ],
};

export const ACTIONS: Record<Kind, Action[]> = {
  video: [
    convert("gif", "анимация animation", ["width", "fps"]),
    { id: "extract_mp3", keywords: "mp3 audio звук", options: QUALITY },
    convert("mp4", "", VIDEO),
    convert("webm", "", VIDEO),
    convert("mov", "", VIDEO),
    convert("mkv"),
    { id: "compress", keywords: "сжать size", options: VIDEO },
    { id: "trim", keywords: "обрезать cut время", options: [...TRIM, "quality"], ask: true },
    { id: "resize", keywords: "размер 1080 720", options: ["height", "quality"] },
    { id: "preset_telegram", keywords: "telegram телеграм" },
    { id: "preset_discord", keywords: "discord дискорд" },
  ],
  audio: [
    convert("mp3", "", QUALITY),
    convert("ogg", "voice голосовое", QUALITY),
    convert("wav"),
    convert("flac"),
    convert("m4a", "", QUALITY),
    convert("opus", "", QUALITY),
    { id: "trim", keywords: "обрезать cut время", options: TRIM, ask: true },
    { id: "normalize", keywords: "громкость loudness" },
  ],
  image: [
    convert("jpg", "jpeg", QUALITY),
    convert("png"),
    convert("webp", "", QUALITY),
    convert("avif", "", QUALITY),
    convert("ico", "icon иконка"),
    convert("pdf", "", ["page"]),
    { id: "resize", keywords: "размер уменьшить", options: ["side", "quality"] },
    { id: "crop", keywords: "обрезать кадрировать", options: ["ratio"] },
    { id: "compress", keywords: "сжать size", options: QUALITY },
    { id: "strip_meta", keywords: "exif gps метаданные" },
  ],
  pdf: [
    { id: "merge", keywords: "объединить склеить join", engine: "mupdf", multi: true },
    { id: "pages_png", keywords: "png картинки images", engine: "mupdf", options: ["dpi"] },
    { id: "pages_jpg", keywords: "jpg картинки images", engine: "mupdf", options: ["dpi", "quality"] },
    { id: "extract_text", keywords: "txt текст", engine: "mupdf" },
    { id: "compress", keywords: "сжать size", engine: "mupdf", options: QUALITY },
    { id: "split", keywords: "разделить страницы pages", engine: "mupdf" },
  ],
  markup: ["md", "html", "epub", "fb2", "txt"].map((e) => ({
    ...convert(e, e === "md" ? "markdown" : e === "epub" || e === "fb2" ? "книга book" : ""),
    engine: "pandoc" as const,
  })),
  unknown: [],
};

const SAME: Record<string, string> = { jpeg: "jpg", markdown: "md", htm: "html" };
const ext = (f: FileInfo) => SAME[f.ext] ?? f.ext;

/** Inputs no engine can read yet (SVG waits for libvips). */
const UNREADABLE = new Set(["svg"]);

/** Mirror of `engines::EngineInfo` on the Rust side. */
export interface EngineInfo {
  name: string;
  title: string;
  version: string;
  license: string;
  purpose: string;
  size: number;
  available: boolean;
  installed: boolean;
}

/** The engine an action still has to download, if any. */
export function missingEngine(a: Action, engines: EngineInfo[]): EngineInfo | undefined {
  const e = a.engine && engines.find((x) => x.name === a.engine);
  return e && !e.installed ? e : undefined;
}

/** True when some selected file has a type the menu knows but cannot handle yet. */
export function notSupportedYet(files: FileInfo[], engines: EngineInfo[] = []): boolean {
  const usable = (a: Action) => !a.soon && engines.find((e) => e.name === a.engine)?.available !== false;
  return files.some((f) => UNREADABLE.has(f.ext) || !ACTIONS[f.kind].some(usable));
}

/** Actions valid for every selected file, in catalog order. Actions whose
 * engine has no build for this system are left out. */
export function actionsFor(files: FileInfo[], engines: EngineInfo[] = []): Action[] {
  if (files.length === 0 || notSupportedYet(files, engines)) return [];
  const kinds = new Set(files.map((f) => f.kind));
  const [first, ...rest] = [...kinds];
  const common = ACTIONS[first].filter((a) => rest.every((k) => ACTIONS[k].some((b) => b.id === a.id)));
  return common.filter(
    (a) =>
      !a.soon &&
      engines.find((e) => e.name === a.engine)?.available !== false &&
      (!a.multi || files.length > 1) &&
      (!a.target || files.some((f) => ext(f) !== a.target)),
  );
}

export function quickFor(files: FileInfo[]): Action[] {
  if (notSupportedYet(files)) return [];
  const kinds = new Set(files.map((f) => f.kind));
  return kinds.size === 1 ? (QUICK[files[0].kind] ?? []) : [];
}

const MB = 1024 * 1024;

/** The action most likely wanted for this selection, if any. */
export function suggest(files: FileInfo[]): string | undefined {
  if (files.length === 0) return;
  const all = (p: (f: FileInfo) => boolean) => files.every(p);
  if (all((f) => f.ext === "heic" || f.ext === "heif")) return "to_jpg";
  if (all((f) => f.ext === "ogg" || f.ext === "oga" || f.ext === "opus")) return "to_mp3";
  if (all((f) => f.kind === "video" && f.size > 500 * MB)) return "compress";
}

/** Submenu starting values: what the Rust side would use without a choice. */
export function defaultOptions(a: Action, files: FileInfo[], quality: Quality): Options {
  const video = files.some((f) => f.kind === "video");
  const o: Options = {};
  for (const f of a.options ?? []) {
    if (f === "quality") o.quality = a.id === "compress" ? "small" : quality;
    if (f === "height") o.height = a.id === "resize" ? 720 : a.id === "compress" && video ? 1080 : 0;
    if (f === "side") o.side = 1920;
    if (f === "ratio") o.ratio = [1, 1];
    if (f === "page") o.page = "a4";
    if (f === "width") o.width = 480;
    if (f === "fps") o.fps = 12;
  }
  return o;
}
