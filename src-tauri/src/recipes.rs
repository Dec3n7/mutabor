//! Maps a menu action to ffmpeg arguments. Pure functions, no I/O.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    High,
    #[default]
    Normal,
    Small,
}

impl Quality {
    /// Picks the value for this quality from a (high, normal, small) triple.
    pub fn pick<T>(self, high: T, normal: T, small: T) -> T {
        match self {
            Quality::High => high,
            Quality::Normal => normal,
            Quality::Small => small,
        }
    }
}

/// User choices from the action's settings submenu. Missing fields fall back
/// to the action's own defaults; a size of 0 means "keep the original".
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub quality: Option<Quality>,
    /// Maximum video height in pixels.
    pub height: Option<u32>,
    /// Maximum GIF width in pixels.
    pub width: Option<u32>,
    /// GIF frame rate.
    pub fps: Option<u32>,
    /// Trim range in seconds; no end means "to the end".
    pub start: Option<f64>,
    pub end: Option<f64>,
    /// Longest side of a resized image in pixels.
    pub side: Option<u32>,
    /// Crop aspect ratio, width:height.
    pub ratio: Option<[u32; 2]>,
    /// PDF page size for images.
    pub page: Option<Page>,
    /// Resolution for PDF pages rendered as pictures.
    pub dpi: Option<u32>,
    /// Size limit for "compress to N MB", in megabytes.
    pub mb: Option<u32>,
    /// Playback speed factor.
    pub speed: Option<f32>,
    /// Moment in seconds (a still frame).
    pub at: Option<f64>,
    /// Exact crop area in the picture's pixels: x, y, width, height.
    pub rect: Option<[u32; 4]>,
    /// Upscaling model: photos or drawings.
    pub model: Option<Model>,
    /// Spoken language for speech recognition; none means "detect".
    pub speech: Option<SpeechLang>,
}

/// A two-letter language code ("ru"), kept `Copy` like the other options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechLang(pub [u8; 2]);

impl SpeechLang {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("en")
    }
}

impl Serialize for SpeechLang {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SpeechLang {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_bytes() {
            [a, b] if a.is_ascii_lowercase() && b.is_ascii_lowercase() => Ok(SpeechLang([*a, *b])),
            _ => Err(serde::de::Error::custom("a two-letter language code")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    #[default]
    Photo,
    Anime,
}

impl Options {
    /// The part worth remembering for next time: times belong to one file.
    pub fn reusable(mut self) -> Self {
        self.start = None;
        self.end = None;
        self.at = None;
        self.rect = None;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Page {
    /// A4, image fitted with margins.
    #[default]
    A4,
    /// The page is the image's own size at 96 dpi.
    Original,
}

/// A step after the main program, done by Mutabor itself or by ffmpeg.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Post {
    /// ffmpeg writes a JPEG, which is then wrapped into a one-page PDF.
    JpegToPdf(Page),
    /// MuPDF renders PNG pages, ffmpeg turns them into JPEGs.
    PngPagesToJpg(Quality),
}

/// The program that does the work. ffmpeg ships with the app; the others
/// are optional engines downloaded on first use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Ffmpeg,
    /// MuPDF's `mutool`.
    Mutool,
    Pandoc,
    /// 7-Zip: `7z` on Windows, `7zz` elsewhere.
    SevenZip,
    /// Real-ESRGAN (ncnn/Vulkan) picture upscaler.
    Upscaler,
    /// whisper.cpp speech recognition.
    Whisper,
    /// Background removal, run inside the app (see `matting.rs`).
    Matting,
}

impl Tool {
    /// (engine, program) for the optional ones.
    pub fn engine(self) -> Option<(&'static str, &'static str)> {
        match self {
            Tool::Ffmpeg => None,
            Tool::Mutool => Some(("mupdf", "mutool")),
            Tool::Pandoc => Some(("pandoc", "pandoc")),
            Tool::SevenZip => Some(("sevenzip", "7z")),
            Tool::Upscaler => Some(("realesrgan", "realesrgan-ncnn-vulkan")),
            Tool::Whisper => Some(("whisper", "whisper-cli")),
            // A model-only engine: the "program" is the model file.
            Tool::Matting => Some(("bgremove", crate::matting::MODEL_FILE)),
        }
    }
}

/// What to run for one input file.
///
/// For ffmpeg, `args` go between `-i <input>` and the output. For the other
/// tools `args` is the whole command line, with `{in}`, `{out}` and `{inputs}`
/// standing for the input, the output and all inputs (merge), and `{page}` /
/// `{page3}` for the page number when `per_page` is set.
#[derive(Debug, PartialEq, Default)]
pub struct Plan {
    pub tool: Tool,
    /// The output is a folder (one file per page).
    pub dir: bool,
    /// Runs `args` once per PDF page.
    pub per_page: bool,
    /// The tool reads only JPEG/PNG/WebP: other pictures become a PNG first.
    pub png_input: bool,
    /// The tool reads only 16 kHz mono WAV: sound is converted first.
    pub wav_input: bool,
    /// Output extension without the dot.
    pub ext: String,
    /// Appended to the input's file stem, e.g. "_compressed".
    pub suffix: String,
    /// Arguments placed before `-i <input>` (fast seeking).
    pub input_args: Vec<String>,
    /// Arguments placed between `-i <input>` and the output path.
    pub args: Vec<String>,
    /// Length of the result in seconds when it differs from the input's.
    pub expected: Option<f64>,
    pub post: Option<Post>,
}

#[derive(Debug, PartialEq)]
pub enum PlanError {
    /// Known action whose engine is not wired yet (images, PDF, settings).
    NotReady,
    /// The action needs the media duration and ffmpeg did not report one.
    NeedsDuration,
    /// The action works on sound and the input has no audio stream.
    NoAudio,
    /// The trim range is empty or lies past the end of the file.
    BadRange,
}

/// What ffmpeg reports about an input before converting it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Probe {
    pub duration: Option<f64>,
    pub has_audio: bool,
}

impl Probe {
    /// Parses ffmpeg's banner (`ffmpeg -i <input>` stderr).
    pub fn parse(banner: &str) -> Self {
        Probe {
            duration: parse_duration(banner),
            has_audio: banner
                .lines()
                .any(|l| l.trim_start().starts_with("Stream #") && l.contains(": Audio:")),
        }
    }
}

fn gif_filter(width: u32, fps: u32) -> String {
    let scale = if width == 0 { "iw".to_string() } else { format!("'min({width},iw)'") };
    format!(
        "fps={fps},scale={scale}:-2:flags=lanczos,split[a][b];\
         [a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5"
    )
}
/// Caps height at `h` (0 = original) and keeps both sides even, as H.264 requires.
fn fit_height(h: u32) -> String {
    if h == 0 {
        "scale=trunc(iw/2)*2:trunc(ih/2)*2".into()
    } else {
        format!("scale=-2:'min({h},trunc(ih/2)*2)'")
    }
}
const DISCORD_MB: u32 = 10;
const DISCORD_AUDIO_KBPS: f64 = 96.0;
const MB: f64 = 1024.0 * 1024.0;

/// Total bitrate in kbit/s that keeps `secs` of media under `mb` megabytes,
/// with 8% headroom for container overhead and rate-control overshoot.
fn budget_kbps(mb: u32, secs: f64) -> f64 {
    mb as f64 * MB * 8.0 / 1000.0 * 0.92 / secs
}

/// atempo takes 0.5-2.0, so larger changes are chained.
fn atempo(mut speed: f64) -> String {
    let mut parts = Vec::new();
    while speed > 2.0 {
        parts.push("atempo=2".to_string());
        speed /= 2.0;
    }
    while speed < 0.5 {
        parts.push("atempo=0.5".to_string());
        speed /= 0.5;
    }
    parts.push(format!("atempo={speed:.4}"));
    parts.join(",")
}

/// Containers that take H.264 as is; others are re-wrapped as MP4.
fn h264_ext(input_ext: &str) -> &str {
    if matches!(input_ext, "mp4" | "mov" | "mkv" | "m4v") { input_ext } else { "mp4" }
}

/// `[start, end)` as fast-seek input args, `-t` and the resulting length.
fn segment(opts: &Options, duration: Option<f64>) -> (Vec<String>, Vec<String>, Option<f64>) {
    let start = opts.start.unwrap_or(0.0).max(0.0);
    let end = opts.end.or(duration).filter(|e| *e > start);
    let len = end.map(|e| e - start);
    let input = if start > 0.0 { vec!["-ss".into(), secs(start)] } else { vec![] };
    let t = match (opts.end, len) {
        (Some(_), Some(l)) => vec!["-t".into(), secs(l)],
        _ => vec![],
    };
    (input, t, len)
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// First video and audio stream only: subtitles and data streams often
/// cannot be stored in the target container.
fn h264(crf: &str, filter: &str, audio_kbps: &str) -> Vec<String> {
    args(&[
        "-map", "0:v:0", "-map", "0:a:0?",
        "-vf", filter,
        "-c:v", "libx264", "-preset", "medium", "-crf", crf, "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-b:a", audio_kbps,
        "-movflags", "+faststart",
    ])
}

fn plan_of(ext: &str, suffix: impl Into<String>, args: Vec<String>) -> Result<Plan, PlanError> {
    Ok(Plan { ext: ext.into(), suffix: suffix.into(), args, ..Default::default() })
}

fn secs(v: f64) -> String {
    format!("{v:.3}")
}

/// Cuts `[start, end)` out of a video or audio file. Re-encodes, so the cut
/// lands on the exact frame rather than the nearest keyframe.
fn trim_plan(input_ext: &str, probe: Probe, opts: &Options, q: Quality) -> Result<Plan, PlanError> {
    let start = opts.start.unwrap_or(0.0).max(0.0);
    let end = opts.end.or(probe.duration).filter(|e| *e > start + 0.05);
    if opts.end.is_some() && end.is_none() {
        return Err(PlanError::BadRange);
    }
    if probe.duration.is_some_and(|d| start >= d) || (start == 0.0 && opts.end.is_none()) {
        return Err(PlanError::BadRange);
    }
    let length = end.map(|e| e - start);
    let (ext, mut a) = if is_video(input_ext) {
        (h264_ext(input_ext), h264_at(q, 0))
    } else {
        // Same container; ffmpeg picks its default encoder (LAME, Vorbis, FLAC…).
        (input_ext, args(&["-vn"]))
    };
    if let Some(len) = length {
        a.extend(["-t".into(), secs(len)]);
    }
    Ok(Plan {
        ext: ext.into(),
        suffix: "_trimmed".into(),
        input_args: vec!["-ss".into(), secs(start)],
        args: a,
        expected: length,
        ..Default::default()
    })
}

/// Actions that only make sense when there is sound to work with.
const NEEDS_AUDIO: &[&str] = &[
    "extract_mp3", "to_mp3", "to_ogg", "to_wav", "to_flac", "to_m4a", "to_opus", "normalize", "ringtone", "voice",
    "subtitles", "transcribe",
];

/// Speech to subtitles (SRT) or plain text with whisper.cpp; the file is
/// named like the input, next to it.
fn speech_plan(action: &str, opts: &Options) -> Result<Plan, PlanError> {
    let (ext, format) = match action {
        "subtitles" => ("srt", "-osrt"),
        "transcribe" => ("txt", "-otxt"),
        _ => return Err(PlanError::NotReady),
    };
    let lang = opts.speech.map_or("auto".to_string(), |l| l.as_str().to_string());
    Ok(Plan {
        tool: Tool::Whisper,
        ext: ext.into(),
        wav_input: true,
        args: vec![
            "-m".into(), format!("{{enginedir}}/{WHISPER_MODEL}"),
            "-f".into(), "{in}".into(),
            "-l".into(), lang,
            format.into(),
            "-of".into(), "{outbase}".into(),
        ],
        ..Default::default()
    })
}

/// The speech model downloaded with whisper.cpp (see tools/manifest.json).
pub const WHISPER_MODEL: &str = "ggml-small-q5_1.bin";

/// The quality an action uses when the user picked none: compressing aims
/// small, lossy edits (rotate, flip) keep as much detail as possible.
pub fn default_quality(action: &str) -> Quality {
    match action {
        "compress" => Quality::Small,
        "rotate_left" | "rotate_right" | "flip_h" | "flip_v" | "strip_meta" | "crop" => Quality::High,
        _ => Quality::Normal,
    }
}

/// H.264 for MP4/MOV at the given quality and height cap.
fn h264_at(q: Quality, height: u32) -> Vec<String> {
    h264(q.pick("18", "22", "28"), &fit_height(height), q.pick("192k", "160k", "128k"))
}

fn mp3(q: Quality) -> Vec<String> {
    args(&["-vn", "-c:a", "libmp3lame", "-q:a", q.pick("0", "2", "6")])
}

pub fn plan(action: &str, input_ext: &str, probe: Probe, opts: &Options) -> Result<Plan, PlanError> {
    if NEEDS_AUDIO.contains(&action) && !probe.has_audio {
        return Err(PlanError::NoAudio);
    }
    let q = opts.quality.unwrap_or_else(|| default_quality(action));
    // Packing works on any files.
    match action {
        "pack_zip" => return tool_plan(Tool::SevenZip, "zip", "", &["a", "-tzip", "-y", "{out}", "{names}"]),
        "pack_7z" => return tool_plan(Tool::SevenZip, "7z", "", &["a", "-t7z", "-y", "{out}", "{names}"]),
        _ => {}
    }
    match crate::files::kind_of(input_ext) {
        crate::files::Kind::Archive => return archive_plan(action),
        crate::files::Kind::Image => return image_plan(action, input_ext, q, opts),
        crate::files::Kind::Pdf => return pdf_plan(action, q, opts),
        crate::files::Kind::Markup => return markup_plan(action, input_ext),
        _ => {}
    }
    if action == "trim" {
        return trim_plan(input_ext, probe, opts, q);
    }
    if matches!(action, "subtitles" | "transcribe") {
        return speech_plan(action, opts);
    }
    let duration = probe.duration;
    let height = opts.height.unwrap_or(0);
    let video = is_video(input_ext);
    match action {
        // Video
        "to_gif" => {
            let filter = gif_filter(opts.width.unwrap_or(480), opts.fps.unwrap_or(12).clamp(1, 50));
            let (input_args, t, len) = segment(opts, duration);
            let mut a = vec!["-vf".into(), filter, "-loop".into(), "0".into()];
            a.extend(t);
            Ok(Plan { ext: "gif".into(), input_args, args: a, expected: len, ..Default::default() })
        }
        "mute" => plan_of(input_ext, "_muted", args(&["-map", "0", "-map", "-0:a", "-c", "copy"])),
        "frame" => {
            let at = opts.at.unwrap_or(0.0).max(0.0);
            if duration.is_some_and(|d| at >= d) {
                return Err(PlanError::BadRange);
            }
            Ok(Plan {
                ext: "png".into(),
                suffix: "_frame".into(),
                input_args: vec!["-ss".into(), secs(at)],
                args: args(&["-frames:v", "1", "-update", "1"]),
                ..Default::default()
            })
        }
        "speed" => {
            let s = f64::from(opts.speed.unwrap_or(2.0)).clamp(0.25, 4.0);
            let mut p = if video {
                let filter = format!("setpts=PTS/{s},{}", fit_height(0));
                let mut a = h264(q.pick("18", "22", "28"), &filter, "160k");
                a.extend(["-af".into(), atempo(s)]);
                Plan { ext: h264_ext(input_ext).into(), args: a, ..Default::default() }
            } else {
                Plan { ext: input_ext.into(), args: vec!["-vn".into(), "-af".into(), atempo(s)], ..Default::default() }
            };
            p.suffix = format!("_x{s}");
            p.expected = duration.map(|d| d / s);
            Ok(p)
        }
        // A 30-second AAC clip in an M4R container: an iPhone ringtone.
        "ringtone" => {
            let start = opts.start.unwrap_or(0.0).max(0.0);
            Ok(Plan {
                ext: "m4r".into(),
                input_args: vec!["-ss".into(), secs(start)],
                args: args(&["-vn", "-t", "30", "-c:a", "aac", "-b:a", "192k", "-f", "ipod"]),
                expected: Some(duration.map_or(30.0, |d| (d - start).clamp(0.1, 30.0))),
                ..Default::default()
            })
        }
        // The format Telegram records voice messages in: mono Opus in OGG.
        "voice" => plan_of(
            "ogg",
            "_voice",
            args(&["-vn", "-ac", "1", "-ar", "48000", "-c:a", "libopus", "-b:a", "64k", "-application", "voip"]),
        ),
        "fit_size" | "preset_discord" => {
            let secs = duration.filter(|d| *d > 0.0).ok_or(PlanError::NeedsDuration)?;
            let discord = action == "preset_discord";
            let mb = if discord { DISCORD_MB } else { opts.mb.unwrap_or(25).max(1) };
            let total = budget_kbps(mb, secs);
            let suffix = if discord { "_discord".to_string() } else { format!("_{mb}mb") };
            if !video {
                let kbps = total.clamp(32.0, 320.0).round();
                return plan_of("mp3", suffix, args(&["-vn", "-c:a", "libmp3lame", "-b:a", &format!("{kbps}k")]));
            }
            let audio = if !probe.has_audio {
                0.0
            } else if total < 400.0 {
                64.0
            } else {
                DISCORD_AUDIO_KBPS
            };
            let v = (total - audio).max(50.0).round();
            // Fewer pixels look better than blocky ones at a low bitrate.
            let h = if v < 600.0 {
                480
            } else if v < 1500.0 || discord {
                720
            } else {
                1080
            };
            let mut a = args(&[
                "-map", "0:v:0", "-map", "0:a:0?",
                "-c:v", "libx264", "-preset", "medium", "-pix_fmt", "yuv420p",
                "-c:a", "aac", "-b:a", &format!("{}k", audio.max(32.0)), "-movflags", "+faststart",
            ]);
            a.extend([
                "-vf".into(), fit_height(h),
                "-b:v".into(), format!("{v}k"),
                "-maxrate".into(), format!("{v}k"),
                "-bufsize".into(), format!("{}k", v * 2.0),
            ]);
            plan_of("mp4", suffix, a)
        }
        "extract_mp3" => plan_of("mp3", "", mp3(q)),
        "to_mp4" => plan_of("mp4", "", h264_at(q, height)),
        "to_mov" => plan_of("mov", "", h264_at(q, height)),
        "to_webm" => {
            let mut a = args(&["-map", "0:v:0", "-map", "0:a:0?"]);
            if height > 0 {
                a.extend(["-vf".into(), fit_height(height)]);
            }
            a.extend(args(&[
                "-c:v", "libvpx-vp9", "-crf", q.pick("28", "32", "40"), "-b:v", "0", "-row-mt", "1",
                "-c:a", "libopus", "-b:a", q.pick("160k", "128k", "96k"),
            ]));
            plan_of("webm", "", a)
        }
        // MKV holds nearly any codec, so a lossless remux is enough.
        "to_mkv" => plan_of("mkv", "", args(&["-map", "0", "-c", "copy"])),
        "compress" if is_video(input_ext) => {
            plan_of("mp4", "_compressed", h264_at(q, opts.height.unwrap_or(1080)))
        }
        "resize" if is_video(input_ext) => {
            let h = opts.height.unwrap_or(720);
            plan_of("mp4", format!("_{h}p"), h264_at(q, h))
        }
        "preset_telegram" => plan_of("mp4", "_telegram", h264("23", &fit_height(1080), "160k")),

        // Audio. `-vn` drops cover art, which several containers reject.
        "to_mp3" => plan_of("mp3", "", mp3(q)),
        "to_ogg" => plan_of("ogg", "", args(&["-vn", "-c:a", "libvorbis", "-q:a", q.pick("7", "5", "3")])),
        "to_wav" => plan_of("wav", "", args(&["-vn", "-c:a", "pcm_s16le"])),
        "to_flac" => plan_of("flac", "", args(&["-vn", "-c:a", "flac"])),
        "to_m4a" => plan_of("m4a", "", args(&["-vn", "-c:a", "aac", "-b:a", q.pick("256k", "192k", "128k")])),
        "to_opus" => plan_of("opus", "", args(&["-vn", "-c:a", "libopus", "-b:a", q.pick("160k", "128k", "80k")])),
        "normalize" => plan_of(
            input_ext,
            "_normalized",
            args(&["-vn", "-af", "loudnorm=I=-16:TP=-1.5:LRA=11"]),
        ),

        _ => Err(PlanError::NotReady),
    }
}

/// Paints transparent areas white, for formats that cannot store alpha
/// (otherwise they turn black).
pub const FLATTEN: &str = "format=rgba,split[fg][bg];[bg]format=rgb24,drawbox=c=white:t=fill[bg];[bg][fg]overlay=format=rgb";

/// Where an edited image goes when its own format cannot be written back.
fn writable_ext(input_ext: &str) -> &str {
    match input_ext {
        "jpeg" | "heic" | "heif" => "jpg",
        "tif" => "tiff",
        other => other,
    }
}

/// Encoder arguments for a still image, applying `filters` first.
/// Animated GIF and WebP inputs stay animated where the target allows it.
fn encode_image(ext: &str, input_ext: &str, filters: &[&str], quality: Quality) -> Result<Vec<String>, PlanError> {
    let animated = input_ext == "gif" && matches!(ext, "gif" | "webp");
    let mut chain: Vec<&str> = filters.to_vec();
    let mut a = Vec::new();
    match ext {
        "jpg" => {
            chain.extend([FLATTEN, "format=yuvj444p"]);
            a.extend(args(&["-c:v", "mjpeg", "-q:v", quality.pick("2", "3", "6")]));
        }
        // PNG is lossless: quality only trades encoding time for size.
        "png" => {
            let level = quality.pick("6", "6", "9");
            a.extend(args(&["-c:v", "png", "-pred", "mixed", "-compression_level", level]));
        }
        "webp" => {
            let codec = if animated { "libwebp_anim" } else { "libwebp" };
            a.extend(args(&["-c:v", codec, "-quality", quality.pick("92", "85", "70")]));
            if animated {
                a.extend(args(&["-loop", "0"]));
            }
        }
        "avif" => {
            chain.push(FLATTEN);
            let crf = quality.pick("22", "28", "40");
            a.extend(args(&["-c:v", "libaom-av1", "-still-picture", "1", "-cpu-used", "6", "-crf", crf]));
        }
        "bmp" => a.extend(args(&["-c:v", "bmp"])),
        "tiff" => a.extend(args(&["-c:v", "tiff"])),
        "gif" => a.extend(args(&["-c:v", "gif", "-loop", "0"])),
        // ICO entries are at most 256 px; PNG-in-ICO keeps transparency.
        "ico" => {
            chain.extend([
                "scale='min(256,iw)':'min(256,ih)':force_original_aspect_ratio=decrease",
                // The ICO muxer only accepts RGBA PNGs, even from opaque JPG.
                "format=rgba",
            ]);
            a.extend(args(&["-c:v", "png"]));
        }
        _ => return Err(PlanError::NotReady),
    }
    let mut out = Vec::new();
    if !chain.is_empty() {
        out.extend(["-vf".to_string(), chain.join(",")]);
    }
    out.extend(a);
    // Drop EXIF and other metadata (GPS included).
    out.extend(args(&["-map_metadata", "-1"]));
    if !animated {
        out.extend(args(&["-frames:v", "1", "-update", "1"]));
    }
    Ok(out)
}

fn image_plan(action: &str, input_ext: &str, quality: Quality, opts: &Options) -> Result<Plan, PlanError> {
    // No SVG decoder in ffmpeg; SVG arrives with the libvips engine.
    if input_ext == "svg" {
        return Err(PlanError::NotReady);
    }
    let same = writable_ext(input_ext);
    let (ext, suffix, filter): (&str, &str, Option<String>) = match action {
        "to_jpg" => ("jpg", "", None),
        "to_png" => ("png", "", None),
        "to_webp" => ("webp", "", None),
        "to_avif" => ("avif", "", None),
        "to_ico" => ("ico", "", None),
        // A picture pasted from the clipboard, kept as a file in Pictures.
        "save" => ("png", "", None),
        "upscale" => return upscale_plan(input_ext, opts),
        "remove_bg" => {
            return Ok(Plan { tool: Tool::Matting, ext: "png".into(), suffix: "_nobg".into(), ..Default::default() });
        }
        "rotate_left" => (same, "_rotated", Some("transpose=2".into())),
        "rotate_right" => (same, "_rotated", Some("transpose=1".into())),
        "flip_h" => (same, "_flipped", Some("hflip".into())),
        "flip_v" => (same, "_flipped", Some("vflip".into())),
        "compress" => (same, "_compressed", None),
        "strip_meta" => (same, "_clean", None),
        // Never enlarges: a small picture stays as it is.
        "resize" => {
            let side = opts.side.unwrap_or(1920).max(1);
            let fit = format!("scale='min({side},iw)':'min({side},ih)':force_original_aspect_ratio=decrease");
            (same, "_resized", Some(fit))
        }
        // The largest centred area with the chosen proportions.
        // An area drawn in the crop window, else the largest centred area
        // with the chosen proportions (hotkeys and chains).
        "crop" => match opts.rect {
            Some([x, y, w, h]) => (same, "_cropped", Some(format!("crop={}:{}:{x}:{y}", w.max(1), h.max(1)))),
            None => {
                let [w, h] = opts.ratio.unwrap_or([1, 1]);
                let (w, h) = (w.max(1), h.max(1));
                (same, "_cropped", Some(format!("crop='min(iw,ih*{w}/{h})':'min(ih,iw*{h}/{w})'")))
            }
        },
        "to_pdf" => {
            let page = opts.page.unwrap_or_default();
            let a = encode_image("jpg", input_ext, &[], Quality::High)?;
            return Ok(Plan { ext: "pdf".into(), args: a, post: Some(Post::JpegToPdf(page)), ..Default::default() });
        }
        _ => return Err(PlanError::NotReady),
    };
    let filters: Vec<&str> = filter.as_deref().into_iter().collect();
    plan_of(ext, suffix, encode_image(ext, input_ext, &filters, quality)?)
}

fn tool_plan(tool: Tool, ext: &str, suffix: &str, list: &[&str]) -> Result<Plan, PlanError> {
    Ok(Plan { tool, ext: ext.into(), suffix: suffix.into(), args: args(list), ..Default::default() })
}

fn pdf_plan(action: &str, q: Quality, opts: &Options) -> Result<Plan, PlanError> {
    let dpi = opts.dpi.unwrap_or(150).clamp(36, 600).to_string();
    let pages = |post| Plan {
        tool: Tool::Mutool,
        dir: true,
        suffix: "_pages".into(),
        args: args(&["draw", "-q", "-r", &dpi, "-o", "{out}/page-%03d.png", "{in}"]),
        post,
        ..Default::default()
    };
    match action {
        "pages_png" => Ok(pages(None)),
        "pages_jpg" => Ok(pages(Some(Post::PngPagesToJpg(q)))),
        "extract_text" => tool_plan(Tool::Mutool, "txt", "", &["draw", "-q", "-F", "txt", "-o", "{out}", "{in}"]),
        // Rewrites the file: drops unused objects, deflates streams and
        // re-encodes pictures above the target resolution as JPEG.
        "compress" => {
            let dpi = q.pick("200", "150", "96");
            let jpeg = q.pick("jpeg:85", "jpeg:72", "jpeg:55");
            tool_plan(
                Tool::Mutool,
                "pdf",
                "_compressed",
                &[
                    "clean", "-gggg", "-z", "-f", "-i", "-Z",
                    "--color-image-subsample-dpi", dpi, "--gray-image-subsample-dpi", dpi,
                    "--color-image-recompress-method", jpeg, "--gray-image-recompress-method", jpeg,
                    "{in}", "{out}",
                ],
            )
        }
        "split" => Ok(Plan {
            tool: Tool::Mutool,
            dir: true,
            per_page: true,
            suffix: "_pages".into(),
            args: args(&["merge", "-o", "{out}/page-{page3}.pdf", "{in}", "{page}"]),
            ..Default::default()
        }),
        "merge" => tool_plan(Tool::Mutool, "pdf", "_merged", &["merge", "-o", "{out}", "{inputs}"]),
        _ => Err(PlanError::NotReady),
    }
}

/// Pandoc reader for an input extension.
fn pandoc_from(ext: &str) -> Option<&'static str> {
    Some(match ext {
        // Plain text is read as Markdown: paragraphs survive, nothing is lost.
        "md" | "markdown" | "txt" => "markdown",
        "html" | "htm" => "html",
        "epub" => "epub",
        "fb2" => "fb2",
        "docx" => "docx",
        _ => return None,
    })
}

fn markup_plan(action: &str, input_ext: &str) -> Result<Plan, PlanError> {
    let from = pandoc_from(input_ext).ok_or(PlanError::NotReady)?;
    let (ext, writer): (&str, &[&str]) = match action {
        "to_md" => ("md", &["-t", "gfm"]),
        // Standalone page with pictures inlined, so it opens anywhere.
        "to_html" => ("html", &["-t", "html5", "-s", "--embed-resources"]),
        "to_epub" => ("epub", &["-t", "epub3"]),
        "to_fb2" => ("fb2", &["-t", "fb2"]),
        "to_txt" => ("txt", &["-t", "plain"]),
        _ => return Err(PlanError::NotReady),
    };
    let mut list = vec!["{in}", "-f", from];
    list.extend(writer);
    list.extend(["-o", "{out}"]);
    tool_plan(Tool::Pandoc, ext, "", &list)
}

/// Unpacks into a folder named after the archive, next to it.
fn archive_plan(action: &str) -> Result<Plan, PlanError> {
    match action {
        "extract" => Ok(Plan {
            tool: Tool::SevenZip,
            dir: true,
            args: args(&["x", "-y", "{in}", "-o{out}"]),
            ..Default::default()
        }),
        _ => Err(PlanError::NotReady),
    }
}

/// Real-ESRGAN, four times larger; it writes PNG.
fn upscale_plan(input_ext: &str, opts: &Options) -> Result<Plan, PlanError> {
    let model = match opts.model.unwrap_or_default() {
        Model::Photo => "realesrgan-x4plus",
        Model::Anime => "realesrgan-x4plus-anime",
    };
    Ok(Plan {
        tool: Tool::Upscaler,
        ext: "png".into(),
        suffix: "_x4".into(),
        png_input: !matches!(input_ext, "jpg" | "jpeg" | "png" | "webp"),
        args: args(&["-i", "{in}", "-o", "{out}", "-m", "{tooldir}/models", "-n", model, "-s", "4", "-f", "png"]),
        ..Default::default()
    })
}

fn is_video(ext: &str) -> bool {
    crate::files::kind_of(ext) == crate::files::Kind::Video
}

/// Picture size from the first video stream in ffmpeg's banner:
/// "Stream #0:0: Video: mjpeg ..., yuvj420p(pc), 4032x3024, ..." -> (4032, 3024).
pub fn parse_size(banner: &str) -> Option<(u32, u32)> {
    let line = banner.lines().find(|l| l.trim_start().starts_with("Stream #") && l.contains(": Video:"))?;
    line.split([',', ' ', '[']).find_map(|part| {
        let (w, h) = part.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    })
}

/// Parses "Duration: 00:01:02.50" from ffmpeg's banner output.
fn parse_duration(stderr: &str) -> Option<f64> {
    let rest = stderr.split("Duration: ").nth(1)?;
    let stamp = rest.split(',').next()?.trim();
    let mut parts = stamp.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(duration: f64) -> Probe {
        Probe { duration: Some(duration), has_audio: true }
    }

    #[test]
    fn audio_detection() {
        let silent = "  Duration: 00:00:02.87, start: 0.000000\n  Stream #0:0[0x1](und): Video: h264 (Main), yuv420p, 960x720";
        let p = Probe::parse(silent);
        assert!(!p.has_audio);
        assert_eq!(plan("extract_mp3", "mp4", p, &Options::default()), Err(PlanError::NoAudio));
        assert!(plan("to_gif", "mp4", p, &Options::default()).is_ok());
        let with_audio = format!("{silent}\n  Stream #0:1[0x2](und): Audio: aac (LC), 44100 Hz, stereo");
        assert!(Probe::parse(&with_audio).has_audio);
    }

    #[test]
    fn picture_size() {
        let banner = "Input #0, image2
  Stream #0:0: Video: mjpeg (Baseline), yuvj420p(pc, bt470bg/unknown/unknown), 4032x3024 [SAR 72:72 DAR 4:3], 25 fps";
        assert_eq!(parse_size(banner), Some((4032, 3024)));
        assert_eq!(parse_size("  Stream #0:0: Audio: mp3, 44100 Hz"), None);
    }

    #[test]
    fn duration() {
        let banner = "Input #0, mov,mp4\n  Duration: 00:01:02.50, start: 0.000000, bitrate: 900 kb/s";
        assert_eq!(parse_duration(banner), Some(62.5));
        assert_eq!(parse_duration("  Duration: N/A, bitrate: N/A"), None);
    }

    #[test]
    fn discord_fits_budget() {
        let p = plan("preset_discord", "mp4", probe(60.0), &Options::default()).unwrap();
        let i = p.args.iter().position(|a| a == "-b:v").unwrap();
        let kbps: f64 = p.args[i + 1].trim_end_matches('k').parse().unwrap();
        let bytes = (kbps + DISCORD_AUDIO_KBPS) * 1000.0 / 8.0 * 60.0;
        assert!(bytes < DISCORD_MB as f64 * MB);
        // Any limit: 25 MB for a 10-minute video still fits, at a lower resolution.
        let opts = Options { mb: Some(25), ..Default::default() };
        let p = plan("fit_size", "mp4", probe(600.0), &opts).unwrap();
        assert_eq!(p.suffix, "_25mb");
        assert!(p.args.iter().any(|a| a.contains("min(480,")));
        assert_eq!(atempo(4.0), "atempo=2,atempo=2.0000");
        assert_eq!(atempo(0.25), "atempo=0.5,atempo=0.5000");
        let unknown = Probe { duration: None, has_audio: true };
        assert_eq!(plan("preset_discord", "mp4", unknown, &Options::default()), Err(PlanError::NeedsDuration));
    }

    /// Runs every ffmpeg recipe on generated samples.
    /// Needs `npm run tools` first: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn recipes_run_on_real_ffmpeg() {
        use std::process::Command;
        let bin = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries"))
            .expect("run `npm run tools` first")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("mutabor-ffmpeg-"))
            .expect("ffmpeg sidecar");
        let dir = std::env::temp_dir().join("mutabor-recipes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let out = Command::new(&bin).args(["-hide_banner", "-y"]).args(args).output().unwrap();
            assert!(out.status.success(), "{args:?}\n{}", String::from_utf8_lossy(&out.stderr));
        };
        let video = dir.join("sample.mp4");
        let audio = dir.join("sample.ogg");
        // Odd height on purpose: H.264 needs even sides.
        run(&[
            "-f", "lavfi", "-i", "testsrc=duration=3:size=641x361:rate=25",
            "-f", "lavfi", "-i", "sine=frequency=440:duration=3",
            "-c:v", "libx264", "-c:a", "aac", "-shortest", video.to_str().unwrap(),
        ]);
        run(&["-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-c:a", "libvorbis", audio.to_str().unwrap()]);
        // Half-transparent red, odd size: alpha must turn white, not black, in JPG.
        let png = dir.join("alpha.png");
        let jpg = dir.join("photo.jpg");
        let gif = dir.join("anim.gif");
        run(&["-f", "lavfi", "-i", "color=c=red@0.5:s=33x21,format=rgba", "-frames:v", "1", png.to_str().unwrap()]);
        run(&["-f", "lavfi", "-i", "testsrc=size=64x48", "-frames:v", "1", jpg.to_str().unwrap()]);
        run(&["-f", "lavfi", "-i", "testsrc=duration=1:size=40x30:rate=5", gif.to_str().unwrap()]);

        let image_actions = ["to_jpg", "to_png", "to_webp", "to_avif", "to_ico", "rotate_left", "rotate_right",
                             "flip_h", "flip_v", "compress", "strip_meta", "resize", "crop", "to_pdf"];
        let cases = [
            (&video, "mp4", ["to_gif", "extract_mp3", "to_mp4", "to_mov", "to_webm", "to_mkv", "compress",
                             "preset_telegram", "preset_discord", "resize", "trim", "fit_size", "mute", "frame",
                             "speed", "ringtone", "voice"].as_slice()),
            (&audio, "ogg", ["to_mp3", "to_ogg", "to_wav", "to_flac", "to_m4a", "to_opus", "normalize", "trim",
                             "fit_size", "speed", "ringtone", "voice"].as_slice()),
            (&png, "png", image_actions.as_slice()),
            (&jpg, "jpg", image_actions.as_slice()),
            (&gif, "gif", image_actions.as_slice()),
        ];
        for (input, ext, actions) in cases {
            for action in actions {
                let opts = Options { start: Some(1.0), end: Some(2.5), side: Some(32), ratio: Some([16, 9]),
                                     height: Some(240), mb: Some(1), at: Some(1.0), speed: Some(2.0),
                                     ..Default::default() };
                let p = plan(action, ext, probe(3.0), &opts).unwrap();
                let stem = input.file_stem().unwrap().to_string_lossy();
                let out = dir.join(format!("{stem}_{action}{}.{}", p.suffix, p.ext));
                let written = if p.post.is_some() { out.with_extension("tmp.jpg") } else { out.clone() };
                let mut args = p.input_args.clone();
                args.extend(["-i".to_string(), input.to_string_lossy().into_owned()]);
                args.extend(p.args.iter().cloned());
                args.push(written.to_string_lossy().into_owned());
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                run(&args);
                if let Some(Post::JpegToPdf(page)) = p.post {
                    crate::pdf::convert(&written, &out, page).unwrap();
                }
                assert!(std::fs::metadata(&out).unwrap().len() > 0, "{action}: empty output");
            }
        }

        // Transparent red over white is pink (≈255,128,128); over black it would be ≈128,0,0.
        let jpg_out = dir.join("alpha_to_jpg.jpg");
        let pixel = Command::new(&bin)
            .args(["-hide_banner", "-i", jpg_out.to_str().unwrap(), "-frames:v", "1",
                   "-vf", "crop=1:1:16:10", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap()
            .stdout;
        assert!(pixel[0] > 230 && (110..150).contains(&pixel[1]) && (110..150).contains(&pixel[2]),
                "alpha not flattened to white: {pixel:?}");

        // Rotation swaps the sides: 64x48 becomes 48x64.
        let banner = Command::new(&bin)
            .args(["-hide_banner", "-i", dir.join("photo_rotate_left_rotated.jpg").to_str().unwrap()])
            .output()
            .unwrap()
            .stderr;
        assert!(String::from_utf8_lossy(&banner).contains("48x64"), "rotation did not swap sides");
        let info = |name: &str| {
            let out = Command::new(&bin).args(["-hide_banner", "-i", dir.join(name).to_str().unwrap()]).output();
            String::from_utf8_lossy(&out.unwrap().stderr).into_owned()
        };
        // 64x48 cropped to 16:9 is 64x36; resized to a 32 px longest side is 32x24.
        assert!(info("photo_crop_cropped.jpg").contains("64x36"), "crop ratio");
        let rect = Options { rect: Some([10, 5, 30, 20]), ..Default::default() };
        let p = plan("crop", "jpg", Probe::default(), &rect).unwrap();
        let out = dir.join("photo_rect.jpg");
        let mut args = vec!["-i".to_string(), jpg.to_string_lossy().into_owned()];
        args.extend(p.args.iter().cloned());
        args.push(out.to_string_lossy().into_owned());
        run(&args.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(info("photo_rect.jpg").contains("30x20"), "crop rect");
        assert!(info("photo_resize_resized.jpg").contains("32x24"), "resize");
        // Trimming 1.0–2.5 s leaves 1.5 s.
        let trimmed = Probe::parse(&info("sample_trim_trimmed.mp4")).duration.unwrap();
        assert!((trimmed - 1.5).abs() < 0.1, "trim length {trimmed}");
        assert!(info("sample_resize_240p.mp4").contains("x240"), "video resize");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn options_change_args() {
        let has = |p: &Plan, pair: [&str; 2]| p.args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]);
        let none = Probe::default();
        let opts = |quality, height| Options { quality: Some(quality), height, ..Default::default() };

        let small = plan("to_mp4", "mkv", none, &opts(Quality::Small, Some(720))).unwrap();
        assert!(has(&small, ["-crf", "28"]));
        assert!(small.args.iter().any(|a| a.contains("min(720,")));
        // Video compression defaults to small and 1080p; conversions keep the size.
        assert!(has(&plan("compress", "mp4", none, &Options::default()).unwrap(), ["-crf", "28"]));
        assert!(!plan("to_mp4", "mkv", none, &Options::default()).unwrap().args.iter().any(|a| a.contains("min(")));

        let gif = plan("to_gif", "mp4", none, &Options { width: Some(0), fps: Some(24), ..Default::default() }).unwrap();
        assert!(gif.args[1].starts_with("fps=24,scale=iw:"));

        assert!(has(&plan("to_jpg", "png", none, &opts(Quality::High, None)).unwrap(), ["-q:v", "2"]));
        assert!(has(&plan("compress", "jpg", none, &Options::default()).unwrap(), ["-q:v", "6"]));
        assert!(has(&plan("to_mp3", "wav", probe(1.0), &opts(Quality::High, None)).unwrap(), ["-q:a", "0"]));

        // The frontend sends lowercase names and may omit any field.
        let parsed: Options = serde_json::from_str(r#"{"quality":"small","height":480}"#).unwrap();
        assert_eq!(parsed, opts(Quality::Small, Some(480)));
    }

    #[test]
    fn speech_recipes() {
        let o: Options = serde_json::from_str(r#"{"speech":"ru"}"#).unwrap();
        let p = plan("subtitles", "mp4", probe(10.0), &o).unwrap();
        assert_eq!((p.tool, p.ext.as_str(), p.wav_input), (Tool::Whisper, "srt", true));
        assert!(p.args.windows(2).any(|w| w[0] == "-l" && w[1] == "ru"));
        let auto = plan("transcribe", "ogg", probe(10.0), &Options::default()).unwrap();
        assert!(auto.args.windows(2).any(|w| w[0] == "-l" && w[1] == "auto"));
        assert!(serde_json::from_str::<Options>(r#"{"speech":"russian"}"#).is_err());
        let silent = Probe { duration: Some(3.0), has_audio: false };
        assert_eq!(plan("subtitles", "mp4", silent, &o), Err(PlanError::NoAudio));
    }

    #[test]
    fn plugin_recipes() {
        let none = Probe::default();
        let o = Options::default();
        let x = plan("extract", "rar", none, &o).unwrap();
        assert_eq!((x.tool, x.dir), (Tool::SevenZip, true));
        assert!(x.args.contains(&"-o{out}".to_string()));
        // Packing works whatever the files are.
        for ext in ["mp4", "jpg", "pdf", "zip"] {
            assert_eq!(plan("pack_zip", ext, none, &o).unwrap().ext, "zip");
        }
        let up = plan("upscale", "heic", none, &Options { model: Some(Model::Anime), ..Default::default() }).unwrap();
        assert!(up.png_input, "HEIC goes through PNG first");
        assert!(up.args.contains(&"realesrgan-x4plus-anime".to_string()));
        assert!(!plan("upscale", "jpg", none, &o).unwrap().png_input);
    }

    #[test]
    fn unsupported_yet() {
        let none = Probe::default();
        // Markup to PDF would need LaTeX, which is not shipped.
        assert_eq!(plan("to_pdf", "md", none, &Options::default()), Err(PlanError::NotReady));
        // A trim needs a range that lies inside the file.
        let range = |start, end| Options { start, end, ..Default::default() };
        assert_eq!(plan("trim", "mp4", probe(10.0), &range(None, None)), Err(PlanError::BadRange));
        assert_eq!(plan("trim", "mp4", probe(10.0), &range(Some(12.0), None)), Err(PlanError::BadRange));
        assert_eq!(plan("trim", "mp4", probe(10.0), &range(Some(5.0), Some(4.0))), Err(PlanError::BadRange));
        assert_eq!(plan("trim", "mp3", probe(10.0), &range(Some(5.0), None)).unwrap().expected, Some(5.0));
        assert_eq!(plan("to_png", "svg", none, &Options::default()), Err(PlanError::NotReady));
        assert_eq!(plan("normalize", "flac", probe(1.0), &Options::default()).unwrap().ext, "flac");
        // HEIC cannot be written back, so edits land in JPG.
        assert_eq!(plan("rotate_left", "heic", none, &Options::default()).unwrap().ext, "jpg");
        assert_eq!(plan("flip_h", "png", none, &Options::default()).unwrap().suffix, "_flipped");
    }
}
