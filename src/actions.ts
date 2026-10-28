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
  /** Opens a settings submenu before running (not wired yet). */
  configurable?: boolean;
  /** Short button text for quick actions; the full label becomes a tooltip. */
  glyph?: string;
  /** Planned but not implemented yet; kept in the catalog, hidden from the menu. */
  soon?: boolean;
}

const convert = (ext: string, keywords = ""): Action => ({
  id: `to_${ext}`,
  target: ext,
  keywords: `${ext} ${keywords}`,
  configurable: true,
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
    convert("gif", "анимация animation"),
    { id: "extract_mp3", keywords: "mp3 audio звук", configurable: true },
    convert("mp4"),
    convert("webm"),
    convert("mov"),
    convert("mkv"),
    { id: "compress", keywords: "сжать size", configurable: true },
    { id: "trim", keywords: "обрезать cut", configurable: true, soon: true },
    { id: "resize", keywords: "размер 1080 720", configurable: true, soon: true },
    { id: "preset_telegram", keywords: "telegram телеграм" },
    { id: "preset_discord", keywords: "discord дискорд" },
  ],
  audio: [
    convert("mp3"),
    convert("ogg", "voice голосовое"),
    convert("wav"),
    convert("flac"),
    convert("m4a"),
    convert("opus"),
    { id: "normalize", keywords: "громкость loudness" },
  ],
  image: [
    convert("jpg", "jpeg"),
    convert("png"),
    convert("webp"),
    convert("avif"),
    convert("ico", "icon иконка"),
    { ...convert("pdf"), soon: true },
    { id: "resize", keywords: "размер", configurable: true, soon: true },
    { id: "crop", keywords: "обрезать", configurable: true, soon: true },
    { id: "compress", keywords: "сжать size", configurable: true },
    { id: "strip_meta", keywords: "exif gps метаданные" },
  ],
  pdf: [
    { id: "pages_png", keywords: "png картинки images", configurable: true, soon: true },
    { id: "pages_jpg", keywords: "jpg картинки images", configurable: true, soon: true },
    { id: "extract_text", keywords: "txt текст", soon: true },
    { id: "compress", keywords: "сжать size", configurable: true, soon: true },
  ],
  markup: ["md", "html", "epub", "fb2", "txt"].map((e) => ({ ...convert(e, e === "md" ? "markdown" : ""), soon: true })),
  unknown: [],
};

const ext = (f: FileInfo) => (f.ext === "jpeg" ? "jpg" : f.ext);

/** Inputs no engine can read yet (SVG waits for libvips). */
const UNREADABLE = new Set(["svg"]);

/** True when some selected file has a type the menu knows but cannot handle yet. */
export function notSupportedYet(files: FileInfo[]): boolean {
  return files.some((f) => UNREADABLE.has(f.ext) || ACTIONS[f.kind].every((a) => a.soon));
}

/** Actions valid for every selected file, in catalog order. */
export function actionsFor(files: FileInfo[]): Action[] {
  if (files.length === 0 || notSupportedYet(files)) return [];
  const kinds = new Set(files.map((f) => f.kind));
  const [first, ...rest] = [...kinds];
  const common = ACTIONS[first].filter((a) => rest.every((k) => ACTIONS[k].some((b) => b.id === a.id)));
  return common.filter((a) => !a.soon && (!a.target || files.some((f) => ext(f) !== a.target)));
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
