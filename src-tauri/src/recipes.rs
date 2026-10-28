//! Maps a menu action to ffmpeg arguments. Pure functions, no I/O.

/// What to run for one input file.
#[derive(Debug, PartialEq)]
pub struct Plan {
    /// Output extension without the dot.
    pub ext: String,
    /// Appended to the input's file stem, e.g. "_compressed".
    pub suffix: &'static str,
    /// Arguments placed between `-i <input>` and the output path.
    pub args: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum PlanError {
    /// Known action whose engine is not wired yet (images, PDF, settings).
    NotReady,
    /// The action needs the media duration and ffmpeg did not report one.
    NeedsDuration,
    /// The action works on sound and the input has no audio stream.
    NoAudio,
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

const GIF_FILTER: &str = "fps=12,scale='min(480,iw)':-2:flags=lanczos,split[a][b];\
                          [a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5";
/// Caps height at `h` and keeps both sides even, as H.264 requires.
fn fit_height(h: u32) -> String {
    format!("scale=-2:'min({h},trunc(ih/2)*2)'")
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

fn plan_of(ext: &str, suffix: &'static str, args: Vec<String>) -> Result<Plan, PlanError> {
    Ok(Plan { ext: ext.into(), suffix, args })
}

/// Actions that only make sense when there is sound to work with.
const NEEDS_AUDIO: &[&str] = &[
    "extract_mp3", "to_mp3", "to_ogg", "to_wav", "to_flac", "to_m4a", "to_opus", "normalize",
];

pub fn plan(action: &str, input_ext: &str, probe: Probe) -> Result<Plan, PlanError> {
    if NEEDS_AUDIO.contains(&action) && !probe.has_audio {
        return Err(PlanError::NoAudio);
    }
    if crate::files::kind_of(input_ext) == crate::files::Kind::Image {
        return image_plan(action, input_ext);
    }
    let duration = probe.duration;
    let even = "scale=trunc(iw/2)*2:trunc(ih/2)*2";
    match action {
        // Video
        "to_gif" => plan_of("gif", "", args(&["-vf", GIF_FILTER, "-loop", "0"])),
        "extract_mp3" => plan_of("mp3", "", args(&["-vn", "-c:a", "libmp3lame", "-q:a", "2"])),
        "to_mp4" => plan_of("mp4", "", h264("20", even, "160k")),
        "to_mov" => plan_of("mov", "", h264("20", even, "160k")),
        "to_webm" => plan_of(
            "webm",
            "",
            args(&[
                "-map", "0:v:0", "-map", "0:a:0?",
                "-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0", "-row-mt", "1",
                "-c:a", "libopus", "-b:a", "128k",
            ]),
        ),
        // MKV holds nearly any codec, so a lossless remux is enough.
        "to_mkv" => plan_of("mkv", "", args(&["-map", "0", "-c", "copy"])),
        "compress" if is_video(input_ext) => {
            plan_of("mp4", "_compressed", h264("28", &fit_height(1080), "128k"))
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
        "to_mp3" => plan_of("mp3", "", args(&["-vn", "-c:a", "libmp3lame", "-q:a", "2"])),
        "to_ogg" => plan_of("ogg", "", args(&["-vn", "-c:a", "libvorbis", "-q:a", "5"])),
        "to_wav" => plan_of("wav", "", args(&["-vn", "-c:a", "pcm_s16le"])),
        "to_flac" => plan_of("flac", "", args(&["-vn", "-c:a", "flac"])),
        "to_m4a" => plan_of("m4a", "", args(&["-vn", "-c:a", "aac", "-b:a", "192k"])),
        "to_opus" => plan_of("opus", "", args(&["-vn", "-c:a", "libopus", "-b:a", "128k"])),
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
            let q = if quality == Quality::Small { "6" } else { "2" };
            a.extend(args(&["-c:v", "mjpeg", "-q:v", q]));
        }
        "png" => {
            let level = if quality == Quality::Small { "9" } else { "6" };
            a.extend(args(&["-c:v", "png", "-pred", "mixed", "-compression_level", level]));
        }
        "webp" => {
            let q = if quality == Quality::Small { "70" } else { "88" };
            let codec = if animated { "libwebp_anim" } else { "libwebp" };
            a.extend(args(&["-c:v", codec, "-quality", q]));
            if animated {
                a.extend(args(&["-loop", "0"]));
            }
        }
        "avif" => {
            chain.push(FLATTEN);
            let crf = if quality == Quality::Small { "40" } else { "28" };
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

#[derive(Clone, Copy, PartialEq)]
enum Quality {
    High,
    Small,
}

fn image_plan(action: &str, input_ext: &str) -> Result<Plan, PlanError> {
    // No SVG decoder in ffmpeg; SVG arrives with the libvips engine.
    if input_ext == "svg" {
        return Err(PlanError::NotReady);
    }
    let same = writable_ext(input_ext);
    let (ext, suffix, filters, quality): (&str, &'static str, &[&str], Quality) = match action {
        "to_jpg" => ("jpg", "", &[], Quality::High),
        "to_png" => ("png", "", &[], Quality::High),
        "to_webp" => ("webp", "", &[], Quality::High),
        "to_avif" => ("avif", "", &[], Quality::High),
        "to_ico" => ("ico", "", &[], Quality::High),
        "rotate_left" => (same, "_rotated", &["transpose=2"], Quality::High),
        "rotate_right" => (same, "_rotated", &["transpose=1"], Quality::High),
        "flip_h" => (same, "_flipped", &["hflip"], Quality::High),
        "flip_v" => (same, "_flipped", &["vflip"], Quality::High),
        "compress" => (same, "_compressed", &[], Quality::Small),
        "strip_meta" => (same, "_clean", &[], Quality::High),
        // to_pdf, resize and crop need libvips or a settings submenu.
        _ => return Err(PlanError::NotReady),
    };
    plan_of(ext, suffix, encode_image(ext, input_ext, filters, quality)?)
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
        assert_eq!(plan("extract_mp3", "mp4", p), Err(PlanError::NoAudio));
        assert!(plan("to_gif", "mp4", p).is_ok());
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
        let p = plan("preset_discord", "mp4", probe(60.0)).unwrap();
        let i = p.args.iter().position(|a| a == "-b:v").unwrap();
        let kbps: f64 = p.args[i + 1].trim_end_matches('k').parse().unwrap();
        let bytes = (kbps + DISCORD_AUDIO_KBPS) * 1000.0 / 8.0 * 60.0;
        assert!(bytes < DISCORD_LIMIT_BYTES);
        let unknown = Probe { duration: None, has_audio: true };
        assert_eq!(plan("preset_discord", "mp4", unknown), Err(PlanError::NeedsDuration));
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
                             "flip_h", "flip_v", "compress", "strip_meta"];
        let cases = [
            (&video, "mp4", ["to_gif", "extract_mp3", "to_mp4", "to_mov", "to_webm", "to_mkv", "compress",
                             "preset_telegram", "preset_discord"].as_slice()),
            (&audio, "ogg", ["to_mp3", "to_ogg", "to_wav", "to_flac", "to_m4a", "to_opus", "normalize"].as_slice()),
            (&png, "png", image_actions.as_slice()),
            (&jpg, "jpg", image_actions.as_slice()),
            (&gif, "gif", image_actions.as_slice()),
        ];
        for (input, ext, actions) in cases {
            for action in actions {
                let p = plan(action, ext, probe(3.0)).unwrap();
                let stem = input.file_stem().unwrap().to_string_lossy();
                let out = dir.join(format!("{stem}_{action}{}.{}", p.suffix, p.ext));
                let mut args = vec!["-i".to_string(), input.to_string_lossy().into_owned()];
                args.extend(p.args.iter().cloned());
                args.push(out.to_string_lossy().into_owned());
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                run(&args);
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_yet() {
        let none = Probe::default();
        assert_eq!(plan("to_pdf", "png", none), Err(PlanError::NotReady));
        assert_eq!(plan("resize", "jpg", none), Err(PlanError::NotReady));
        assert_eq!(plan("to_png", "svg", none), Err(PlanError::NotReady));
        assert_eq!(plan("normalize", "flac", probe(1.0)).unwrap().ext, "flac");
        // HEIC cannot be written back, so edits land in JPG.
        assert_eq!(plan("rotate_left", "heic", none).unwrap().ext, "jpg");
        assert_eq!(plan("flip_h", "png", none).unwrap().suffix, "_flipped");
    }
}
