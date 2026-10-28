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
    fn pick<T>(self, high: T, normal: T, small: T) -> T {
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
}

impl Options {
    /// The part worth remembering for next time: a trim range belongs to one file.
    pub fn reusable(mut self) -> Self {
        self.start = None;
        self.end = None;
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

/// A step after ffmpeg, done by Mutabor itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Post {
    /// ffmpeg writes a JPEG, which is then wrapped into a one-page PDF.
    JpegToPdf(Page),
}

/// What to run for one input file.
#[derive(Debug, PartialEq, Default)]
pub struct Plan {
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
const DISCORD_LIMIT_BYTES: f64 = 10.0 * 1024.0 * 1024.0;
const DISCORD_AUDIO_KBPS: f64 = 96.0;

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
        let ext = if matches!(input_ext, "mp4" | "mov" | "mkv" | "m4v") { input_ext } else { "mp4" };
        (ext, h264_at(q, 0))
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
        post: None,
    })
}

/// Actions that only make sense when there is sound to work with.
const NEEDS_AUDIO: &[&str] = &[
    "extract_mp3", "to_mp3", "to_ogg", "to_wav", "to_flac", "to_m4a", "to_opus", "normalize",
];

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
    if crate::files::kind_of(input_ext) == crate::files::Kind::Image {
        return image_plan(action, input_ext, q, opts);
    }
    if action == "trim" {
        return trim_plan(input_ext, probe, opts, q);
    }
    let duration = probe.duration;
    let height = opts.height.unwrap_or(0);
    match action {
        // Video
        "to_gif" => {
            let filter = gif_filter(opts.width.unwrap_or(480), opts.fps.unwrap_or(12).clamp(1, 50));
            plan_of("gif", "", vec!["-vf".into(), filter, "-loop".into(), "0".into()])
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
        "preset_discord" => {
            let secs = duration.filter(|d| *d > 0.0).ok_or(PlanError::NeedsDuration)?;
            // 8% headroom for container overhead and rate-control overshoot.
            let total_kbps = DISCORD_LIMIT_BYTES * 8.0 / 1000.0 * 0.92 / secs;
            let video = (total_kbps - DISCORD_AUDIO_KBPS).max(100.0).round();
            let mut a = args(&[
                "-map", "0:v:0", "-map", "0:a:0?",
                "-c:v", "libx264", "-preset", "medium", "-pix_fmt", "yuv420p",
                "-c:a", "aac", "-b:a", "96k", "-movflags", "+faststart",
            ]);
            a.extend([
                "-vf".into(), fit_height(720),
                "-b:v".into(), format!("{video}k"),
                "-maxrate".into(), format!("{video}k"),
                "-bufsize".into(), format!("{}k", video * 2.0),
            ]);
            plan_of("mp4", "_discord", a)
        }

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
const FLATTEN: &str = "format=rgba,split[fg][bg];[bg]format=rgb24,drawbox=c=white:t=fill[bg];[bg][fg]overlay=format=rgb";

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
        "crop" => {
            let [w, h] = opts.ratio.unwrap_or([1, 1]);
            let (w, h) = (w.max(1), h.max(1));
            (same, "_cropped", Some(format!("crop='min(iw,ih*{w}/{h})':'min(ih,iw*{h}/{w})'")))
        }
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

fn is_video(ext: &str) -> bool {
    crate::files::kind_of(ext) == crate::files::Kind::Video
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
        assert!(bytes < DISCORD_LIMIT_BYTES);
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
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("ffmpeg-"))
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
                             "preset_telegram", "preset_discord", "resize", "trim"].as_slice()),
            (&audio, "ogg", ["to_mp3", "to_ogg", "to_wav", "to_flac", "to_m4a", "to_opus", "normalize", "trim"]
                .as_slice()),
            (&png, "png", image_actions.as_slice()),
            (&jpg, "jpg", image_actions.as_slice()),
            (&gif, "gif", image_actions.as_slice()),
        ];
        for (input, ext, actions) in cases {
            for action in actions {
                let opts = Options { start: Some(1.0), end: Some(2.5), side: Some(32), ratio: Some([16, 9]),
                                     height: Some(240), ..Default::default() };
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
    fn unsupported_yet() {
        let none = Probe::default();
        assert_eq!(plan("pages_png", "pdf", none, &Options::default()), Err(PlanError::NotReady));
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
