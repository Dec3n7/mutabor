//! Runs conversions as child processes (ffmpeg, MuPDF, Pandoc) and engine
//! downloads, and reports their progress to the toast window through events.
//!
//! One menu choice is a batch: one job per selected file (merging is a
//! single job). Jobs wait in a queue so that only a few files of each kind
//! are converted at once (`settings.parallel`).

use crate::files::Kind;
use crate::recipes::{self, Options, Plan, PlanError, Post, Probe, Tool};
use crate::settings::{CustomAction, Settings, Step as ChainStep};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

pub const TOAST: &str = "toast";

struct Running {
    batch: u64,
    /// The process of the current step, if one is running.
    child: Option<Child>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct Batch {
    left: usize,
    outputs: Vec<PathBuf>,
    copy: bool,
}

#[derive(Default)]
pub struct Jobs {
    next_id: AtomicU64,
    running: Mutex<HashMap<u64, Running>>,
    batches: Mutex<HashMap<u64, Batch>>,
    /// Jobs running per file kind, for the queue.
    busy: Mutex<HashMap<Kind, usize>>,
    freed: Condvar,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum Event {
    /// A new batch of `total` jobs for `action` (an action id or "custom:<id>").
    Batch { batch: u64, action: String, total: usize },
    /// A job joined the queue. `steady`: no percentages will follow.
    Started { id: u64, batch: u64, name: String, steady: bool },
    /// The job left the queue and is running.
    Running { id: u64 },
    Progress { id: u64, percent: u8 },
    Done { id: u64, output: String, before: u64, after: u64 },
    /// An engine finished downloading.
    Installed { id: u64 },
    Failed { id: u64, message: String },
    Cancelled { id: u64 },
    /// Every job of the batch ended; `copied`: results are on the clipboard.
    Finished { batch: u64, copied: bool },
    /// A message without a job, e.g. a hotkey pressed with nothing selected.
    Notice { message: String },
}

/// Sidecars are copied next to the main executable by the Tauri bundler.
fn ffmpeg_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })
}

fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn ffmpeg() -> Command {
    command(&ffmpeg_path())
}

/// `video.mp4` + ("_compressed", "mp4") -> `video_compressed.mp4`, or
/// `video_compressed (1).mp4` when taken. An empty `ext` names a folder.
/// Never returns the input itself.
#[cfg(test)]
fn output_path(input: &Path, suffix: &str, ext: &str) -> PathBuf {
    output_in(input.parent().unwrap_or(Path::new(".")), input, suffix, ext)
}

fn output_in(dir: &Path, input: &Path, suffix: &str, ext: &str) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let base = format!("{stem}{suffix}");
    let dot = if ext.is_empty() { String::new() } else { format!(".{ext}") };
    (0..)
        .map(|n| match n {
            0 => dir.join(format!("{base}{dot}")),
            n => dir.join(format!("{base} ({n}){dot}")),
        })
        .find(|p| p != input && !p.exists())
        .expect("unbounded range")
}

/// Results sit next to the input, except for pictures pasted from the
/// clipboard: those go to Pictures\Mutabor.
fn result_dir(app: &AppHandle, input: &Path) -> PathBuf {
    if crate::clipboard::is_clipboard_image(input) {
        if let Ok(pictures) = app.path().picture_dir() {
            let dir = pictures.join("Mutabor");
            if std::fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }
    }
    input.parent().unwrap_or(Path::new(".")).to_path_buf()
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn probe(input: &Path) -> Probe {
    // `ffmpeg -i` without an output exits with an error but prints the banner.
    match ffmpeg().arg("-hide_banner").arg("-i").arg(input).output() {
        Ok(out) => Probe::parse(&String::from_utf8_lossy(&out.stderr)),
        Err(_) => Probe::default(),
    }
}

/// Probing runs ffmpeg; only audio and video need it.
fn probe_if_media(input: &Path) -> Probe {
    match crate::files::kind_of(&ext_of(input)) {
        Kind::Video | Kind::Audio => probe(input),
        _ => Probe::default(),
    }
}

fn size_of(p: &Path) -> u64 {
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p)
            .map(|d| d.flatten().map(|e| size_of(&e.path())).sum())
            .unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

fn remove(p: &Path) {
    if p.is_dir() {
        let _ = std::fs::remove_dir_all(p);
    } else {
        let _ = std::fs::remove_file(p);
    }
}

fn show_toast(app: &AppHandle) {
    let Some(win) = app.get_webview_window(TOAST) else { return };
    if let (Ok(Some(m)), Ok(size)) = (win.primary_monitor(), win.outer_size()) {
        // Bottom-right corner, above a typical taskbar.
        let x = m.position().x + m.size().width as i32 - size.width as i32 - 16;
        let y = m.position().y + m.size().height as i32 - size.height as i32 - 64;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    let _ = win.show();
}

/// Shows a one-line message in the toast (`message` is an i18n key or text).
pub fn notice(app: &AppHandle, message: &str) {
    let _ = app.emit_to(TOAST, "job", Event::Notice { message: message.into() });
    show_toast(app);
}

fn plan_error(e: PlanError) -> String {
    match e {
        PlanError::NotReady => "not_ready",
        PlanError::NeedsDuration => "no_duration",
        PlanError::NoAudio => "no_audio",
        PlanError::BadRange => "bad_range",
    }
    .into()
}

fn program_for(app: &AppHandle, tool: Tool) -> Result<PathBuf, String> {
    match tool.engine() {
        None => Ok(ffmpeg_path()),
        Some((engine, program)) => {
            crate::engines::program(app, engine, program).ok_or_else(|| format!("engine_missing:{engine}"))
        }
    }
}

/// Splits a command line typed by the user: spaces separate arguments,
/// "double" or 'single' quotes keep spaces inside one.
fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// "Сжать для Discord" -> "_Сжать-для-Discord": safe in a file name.
fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { String::new() } else { format!("_{s}") }
}

/// One process of a job.
struct Step {
    cmd: Command,
    /// Parse ffmpeg's `-progress` output for percentages.
    ffmpeg: bool,
}

/// Fills `{in}`, `{out}`, `{inputs}`, `{page}` and `{page3}` in a tool's arguments.
fn expand(args: &[String], inputs: &[PathBuf], out: &Path, page: Option<u32>) -> Vec<String> {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let mut v = Vec::new();
    for a in args {
        if a == "{inputs}" {
            v.extend(inputs.iter().map(|p| path(p)));
            continue;
        }
        let mut a = a.replace("{in}", &path(&inputs[0])).replace("{out}", &path(out));
        if let Some(n) = page {
            a = a.replace("{page3}", &format!("{n:03}")).replace("{page}", &n.to_string());
        }
        v.push(a);
    }
    v
}

/// Page count of a PDF, via `mutool show <pdf> trailer/Root/Pages/Count`.
fn pdf_pages(mutool: &Path, pdf: &Path) -> Option<u32> {
    let out = command(mutool).arg("show").arg(pdf).arg("trailer/Root/Pages/Count").output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// What one job does.
enum Work {
    /// A planned conversion; the plan was checked before queueing. An SVG
    /// input is drawn into a PNG of `svg_side` first.
    Plan { plan: Plan, program: PathBuf, duration: Option<f64>, svg_side: Option<u32> },
    /// A user-defined chain; each step is planned when its input exists.
    Chain(CustomAction),
}

struct Job {
    id: u64,
    batch: u64,
    inputs: Vec<PathBuf>,
    kind: Kind,
    work: Work,
    cancelled: Arc<AtomicBool>,
}

/// Maps a step's own 0–100 onto its share of the job's progress bar.
#[derive(Clone, Copy)]
struct Span(u8, u8);

impl Span {
    const FULL: Span = Span(0, 100);
    fn at(self, p: u8) -> u8 {
        (self.0 as u32 + p as u32 * (self.1 - self.0) as u32 / 100).min(99) as u8
    }
}

impl Jobs {
    /// Queues one job per input (merging is one job for all of them) and
    /// returns at once. Errors only when no job could be queued; files that
    /// cannot take the action show up as failed in the toast.
    pub fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        action: &str,
        inputs: Vec<String>,
        opts: &Options,
        settings: &Settings,
    ) -> Result<(), String> {
        let mut inputs: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
        if inputs.is_empty() {
            return Err("nothing_selected".into());
        }
        let custom = match action.strip_prefix("custom:") {
            Some(id) => Some(settings.custom.iter().find(|c| c.id == id).cloned().ok_or("not_ready")?),
            None => None,
        };
        let groups: Vec<Vec<PathBuf>> = if action == "merge" {
            // Explorer reports the selection in click order; file names are more predictable.
            inputs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
            vec![inputs]
        } else {
            inputs.into_iter().map(|p| vec![p]).collect()
        };

        let batch = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut jobs = Vec::new();
        let mut rejected = Vec::new();
        for group in groups {
            let kind = crate::files::kind_of(&ext_of(&group[0]));
            let work = match &custom {
                Some(c) => Ok(Work::Chain(c.clone())),
                None => self.prepare(app, action, &group[0], opts),
            };
            match work {
                Ok(work) => jobs.push(Job {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    batch,
                    inputs: group,
                    kind,
                    work,
                    cancelled: Arc::new(AtomicBool::new(false)),
                }),
                Err(e) => rejected.push((group, e)),
            }
        }
        if jobs.is_empty() {
            return Err(rejected.into_iter().next().map(|(_, e)| e).unwrap_or_default());
        }

        let total = jobs.len() + rejected.len();
        let _ = app.emit_to(TOAST, "job", Event::Batch { batch, action: action.into(), total });
        show_toast(app);
        for (group, message) in rejected {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let name = group[0].file_name().unwrap_or_default().to_string_lossy().into_owned();
            let _ = app.emit_to(TOAST, "job", Event::Started { id, batch, name, steady: true });
            let _ = app.emit_to(TOAST, "job", Event::Failed { id, message });
        }
        self.batches
            .lock()
            .unwrap()
            .insert(batch, Batch { left: jobs.len(), outputs: Vec::new(), copy: settings.copy_result });

        let parallel = settings.parallel;
        for job in jobs {
            let name = job.inputs[0].file_name().unwrap_or_default().to_string_lossy().into_owned();
            let steady = matches!(&job.work, Work::Plan { plan, .. } if plan.tool != Tool::Ffmpeg && !plan.per_page);
            self.running.lock().unwrap().insert(
                job.id,
                Running { batch, child: None, cancelled: job.cancelled.clone() },
            );
            let _ = app.emit_to(TOAST, "job", Event::Started { id: job.id, batch, name, steady });
            let jobs = self.clone();
            let app = app.clone();
            let limit = parallel.limit(job.kind);
            std::thread::spawn(move || jobs.run_job(&app, job, limit));
        }
        Ok(())
    }

    fn prepare(&self, app: &AppHandle, action: &str, input: &Path, opts: &Options) -> Result<Work, String> {
        let probe = probe_if_media(input);
        let ext = ext_of(input);
        // An SVG becomes a PNG before the recipe runs.
        let ext = if ext == "svg" { "png".to_string() } else { ext };
        let plan = recipes::plan(action, &ext, probe, opts).map_err(plan_error)?;
        let program = program_for(app, plan.tool)?;
        let duration = plan.expected.or(probe.duration);
        Ok(Work::Plan { plan, program, duration, svg_side: opts.side })
    }

    /// Draws an SVG into a temporary PNG; other files pass through.
    fn rasterize(&self, id: u64, input: &Path, side: Option<u32>) -> Result<(PathBuf, Option<PathBuf>), String> {
        if ext_of(input) != "svg" {
            return Ok((input.to_path_buf(), None));
        }
        let png = std::env::temp_dir().join(format!("mutabor-svg-{id}-{}.png", self.next_id.fetch_add(1, Ordering::Relaxed)));
        crate::svg::render(input, &png, side)?;
        Ok((png.clone(), Some(png)))
    }

    /// Waits for a free slot for this kind of file; false when cancelled meanwhile.
    fn acquire(&self, kind: Kind, limit: usize, cancelled: &AtomicBool) -> bool {
        let mut busy = self.busy.lock().unwrap();
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return false;
            }
            let n = busy.entry(kind).or_insert(0);
            if *n < limit {
                *n += 1;
                return true;
            }
            busy = self.freed.wait_timeout(busy, Duration::from_millis(250)).unwrap().0;
        }
    }

    fn release(&self, kind: Kind) {
        if let Some(n) = self.busy.lock().unwrap().get_mut(&kind) {
            *n = n.saturating_sub(1);
        }
        self.freed.notify_all();
    }

    fn run_job(&self, app: &AppHandle, job: Job, limit: usize) {
        let result = if self.acquire(job.kind, limit, &job.cancelled) {
            let _ = app.emit_to(TOAST, "job", Event::Running { id: job.id });
            let r = match &job.work {
                Work::Plan { plan, program, duration, svg_side } => {
                    let dir = result_dir(app, &job.inputs[0]);
                    let ext = if plan.dir { "" } else { &plan.ext };
                    // Named after the original even when an SVG was drawn first.
                    let output = output_in(&dir, &job.inputs[0], &plan.suffix, ext);
                    self.rasterize(job.id, &job.inputs[0], *svg_side).and_then(|(first, tmp)| {
                        let mut inputs = job.inputs.clone();
                        inputs[0] = first;
                        let r = self.run_plan(app, job.id, plan, program, &inputs, &output, *duration, Span::FULL);
                        if let Some(tmp) = tmp {
                            let _ = std::fs::remove_file(tmp);
                        }
                        r.map(|()| output)
                    })
                }
                Work::Chain(custom) => self.run_chain(app, job.id, custom, &job.inputs[0]),
            };
            self.release(job.kind);
            r
        } else {
            Err("cancelled".into())
        };
        self.running.lock().unwrap().remove(&job.id);

        let cancelled = job.cancelled.load(Ordering::Relaxed);
        let output = match (&result, cancelled) {
            (Ok(output), false) => Some(output.clone()),
            (Ok(output), true) => {
                remove(output);
                None
            }
            _ => None,
        };
        let event = match (&output, result) {
            (Some(output), _) => Event::Done {
                id: job.id,
                output: output.to_string_lossy().into_owned(),
                before: job.inputs.iter().map(|p| size_of(p)).sum(),
                after: size_of(output),
            },
            (None, _) if cancelled => Event::Cancelled { id: job.id },
            (None, Err(message)) => {
                eprintln!("job {} failed: {message}", job.id);
                Event::Failed { id: job.id, message }
            }
            (None, Ok(_)) => Event::Cancelled { id: job.id },
        };
        let _ = app.emit_to(TOAST, "job", event);
        self.finish(app, job.batch, output);
    }

    /// Counts a job off its batch; the last one copies the results if asked to.
    fn finish(&self, app: &AppHandle, batch: u64, output: Option<PathBuf>) {
        let done = {
            let mut batches = self.batches.lock().unwrap();
            let Some(b) = batches.get_mut(&batch) else { return };
            b.left = b.left.saturating_sub(1);
            b.outputs.extend(output);
            (b.left == 0).then(|| (b.copy, b.outputs.clone()))
        };
        if let Some((copy, outputs)) = done {
            let copied = copy && !outputs.is_empty() && crate::clipboard::copy_files(&outputs).is_ok();
            let _ = app.emit_to(TOAST, "job", Event::Finished { batch, copied });
        }
    }

    /// Runs a plan into `output`; removes a partial result on failure.
    #[allow(clippy::too_many_arguments)]
    fn run_plan(
        &self,
        app: &AppHandle,
        id: u64,
        plan: &Plan,
        program: &Path,
        inputs: &[PathBuf],
        output: &Path,
        duration: Option<f64>,
        span: Span,
    ) -> Result<(), String> {
        // With a post step ffmpeg writes an intermediate file next to the result.
        let written = match plan.post {
            Some(Post::JpegToPdf(_)) => output.with_extension("mutabor-tmp.jpg"),
            _ => output.to_path_buf(),
        };
        if plan.dir {
            std::fs::create_dir(output).map_err(|e| e.to_string())?;
        }
        let mut result = self.work(app, id, plan, program, inputs, &written, duration, span);
        if result.is_ok() {
            if let Some(Post::JpegToPdf(page)) = plan.post {
                result = crate::pdf::convert(&written, output, page);
            }
        }
        if written != output {
            let _ = std::fs::remove_file(&written);
        }
        if result.is_err() {
            remove(output);
        }
        result
    }

    /// Runs a user-defined chain: each step works on the previous result,
    /// intermediate files live in a temporary folder.
    fn run_chain(&self, app: &AppHandle, id: u64, custom: &CustomAction, input: &Path) -> Result<PathBuf, String> {
        let tmp = std::env::temp_dir().join(format!("mutabor-chain-{id}"));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let n = custom.steps.len();
        let result = (|| {
            if n == 0 {
                return Err("not_ready".to_string());
            }
            let mut current = input.to_path_buf();
            for (i, step) in custom.steps.iter().enumerate() {
                if ext_of(&current) == "svg" {
                    let side = match step {
                        ChainStep::Action { options, .. } => options.side,
                        ChainStep::Ffmpeg { .. } => None,
                    };
                    let png = tmp.join(format!("step{i}-svg.png"));
                    crate::svg::render(&current, &png, side)?;
                    current = png;
                }
                let probe = probe_if_media(&current);
                let plan = match step {
                    ChainStep::Action { action, options } => {
                        recipes::plan(action, &ext_of(&current), probe, options).map_err(plan_error)?
                    }
                    ChainStep::Ffmpeg { ffmpeg, ext } => Plan {
                        ext: ext.trim_start_matches('.').to_lowercase(),
                        args: split_args(ffmpeg),
                        ..Default::default()
                    },
                };
                let last = i + 1 == n;
                if plan.dir && !last {
                    return Err("folder_not_last".into());
                }
                let program = program_for(app, plan.tool)?;
                let output = if last {
                    let ext = if plan.dir { "" } else { &plan.ext };
                    output_in(&result_dir(app, input), input, &slug(&custom.name), ext)
                } else {
                    tmp.join(format!("step{i}.{}", plan.ext))
                };
                let span = Span((i * 100 / n) as u8, ((i + 1) * 100 / n) as u8);
                let duration = plan.expected.or(probe.duration);
                self.run_plan(app, id, &plan, &program, std::slice::from_ref(&current), &output, duration, span)?;
                if self.running.lock().unwrap().get(&id).is_some_and(|r| r.cancelled.load(Ordering::Relaxed)) {
                    remove(&output);
                    return Err("cancelled".into());
                }
                current = output;
            }
            Ok(current)
        })();
        let _ = std::fs::remove_dir_all(&tmp);
        result
    }

    /// Builds and runs the processes of one plan in order; Err carries the
    /// last lines the failing program printed.
    #[allow(clippy::too_many_arguments)]
    fn work(
        &self,
        app: &AppHandle,
        id: u64,
        plan: &Plan,
        program: &Path,
        inputs: &[PathBuf],
        written: &Path,
        duration: Option<f64>,
        span: Span,
    ) -> Result<(), String> {
        let mut steps = Vec::new();
        match plan.tool {
            Tool::Ffmpeg => {
                let mut cmd = ffmpeg();
                cmd.args(["-hide_banner", "-nostdin", "-n"])
                    .args(&plan.input_args)
                    .arg("-i")
                    .arg(&inputs[0])
                    .args(&plan.args)
                    .args(["-progress", "pipe:1", "-nostats"])
                    .arg(written);
                steps.push(Step { cmd, ffmpeg: true });
            }
            _ => {
                let pages: Vec<Option<u32>> = if plan.per_page {
                    let n = pdf_pages(program, &inputs[0]).ok_or("couldn't count the PDF's pages")?;
                    (1..=n).map(Some).collect()
                } else {
                    vec![None]
                };
                for page in pages {
                    let mut cmd = command(program);
                    cmd.args(expand(&plan.args, inputs, written, page));
                    // Relative links (pictures in Markdown) resolve next to the input.
                    if let Some(dir) = inputs[0].parent() {
                        cmd.current_dir(dir);
                    }
                    steps.push(Step { cmd, ffmpeg: false });
                }
            }
        }
        if let Some(Post::PngPagesToJpg(q)) = plan.post {
            let mut cmd = ffmpeg();
            cmd.args(["-hide_banner", "-nostdin", "-n", "-i"])
                .arg(written.join("page-%03d.png"))
                .args(["-q:v", q.pick("2", "3", "6"), "-start_number", "1"])
                .arg(written.join("page-%03d.jpg"));
            steps.push(Step { cmd, ffmpeg: false });
        }

        let total = steps.len();
        for (i, step) in steps.into_iter().enumerate() {
            let part = Span(span.at((i * 100 / total) as u8), span.at(((i + 1) * 100 / total) as u8));
            self.run_step(app, id, step, duration, part)?;
            if total > 1 {
                let _ = app.emit_to(TOAST, "job", Event::Progress { id, percent: part.1 });
            }
        }
        if matches!(plan.post, Some(Post::PngPagesToJpg(_))) {
            for e in std::fs::read_dir(written).map_err(|e| e.to_string())?.flatten() {
                if e.path().extension().is_some_and(|x| x == "png") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    fn run_step(&self, app: &AppHandle, id: u64, mut step: Step, duration: Option<f64>, span: Span) -> Result<(), String> {
        let mut child = step
            .cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        {
            let mut running = self.running.lock().unwrap();
            let Some(r) = running.get_mut(&id) else { return Err("cancelled".into()) };
            if r.cancelled.load(Ordering::Relaxed) {
                let _ = child.kill();
                return Err("cancelled".into());
            }
            r.child = Some(child);
        }

        // Drain stderr so the program never blocks on a full pipe; keep the tail for errors.
        let tail = Arc::new(Mutex::new(VecDeque::<String>::new()));
        let tail_w = tail.clone();
        let stderr_thread = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut t = tail_w.lock().unwrap();
                if t.len() == 8 {
                    t.pop_front();
                }
                t.push_back(line);
            }
        });

        let mut last = 0u8;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !step.ffmpeg {
                continue;
            }
            // `out_time_us` and the misnamed `out_time_ms` are both microseconds.
            let Some(us) = line
                .strip_prefix("out_time_us=")
                .or_else(|| line.strip_prefix("out_time_ms="))
                .and_then(|v| v.parse::<f64>().ok())
            else {
                continue;
            };
            if let Some(total) = duration.filter(|d| *d > 0.0) {
                let percent = span.at((us / 1e6 / total * 100.0).clamp(0.0, 100.0) as u8);
                if percent > last {
                    last = percent;
                    let _ = app.emit_to(TOAST, "job", Event::Progress { id, percent });
                }
            }
        }
        let _ = stderr_thread.join();

        let child = self.running.lock().unwrap().get_mut(&id).and_then(|r| r.child.take());
        let ok = child.is_some_and(|mut c| c.wait().is_ok_and(|s| s.success()));
        if ok {
            Ok(())
        } else {
            let tail = tail.lock().unwrap();
            Err(tail.iter().cloned().collect::<Vec<_>>().join("\n"))
        }
    }

    /// Downloads an engine, showing progress in the toast. Blocks until done.
    pub fn install_engine(&self, app: &AppHandle, engine: &str, title: &str) -> Result<(), String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .unwrap()
            .insert(id, Running { batch: id, child: None, cancelled: cancelled.clone() });
        let _ = app.emit_to(TOAST, "job", Event::Batch { batch: id, action: "engine".into(), total: 1 });
        let _ = app.emit_to(TOAST, "job", Event::Started { id, batch: id, name: title.into(), steady: false });
        let _ = app.emit_to(TOAST, "job", Event::Running { id });
        show_toast(app);
        let result = crate::engines::install(app, engine, &cancelled, |percent| {
            let _ = app.emit_to(TOAST, "job", Event::Progress { id, percent });
        });
        self.running.lock().unwrap().remove(&id);
        let event = match &result {
            Ok(()) => Event::Installed { id },
            Err(_) if cancelled.load(Ordering::Relaxed) => Event::Cancelled { id },
            Err(e) => Event::Failed { id, message: e.clone() },
        };
        let _ = app.emit_to(TOAST, "job", event);
        let _ = app.emit_to(TOAST, "job", Event::Finished { batch: id, copied: false });
        result
    }

    pub fn cancel(&self, id: u64) {
        if let Some(job) = self.running.lock().unwrap().get_mut(&id) {
            job.cancelled.store(true, Ordering::Relaxed);
            if let Some(child) = job.child.as_mut() {
                let _ = child.kill();
            }
        }
        self.freed.notify_all();
    }

    /// Cancels every job of a batch, queued or running.
    pub fn cancel_batch(&self, batch: u64) {
        for job in self.running.lock().unwrap().values_mut().filter(|j| j.batch == batch) {
            job.cancelled.store(true, Ordering::Relaxed);
            if let Some(child) = job.child.as_mut() {
                let _ = child.kill();
            }
        }
        self.freed.notify_all();
    }

    /// Deletes the results a batch produced. Only paths Mutabor itself wrote
    /// for that batch are touched.
    pub fn undo(&self, batch: u64) -> usize {
        let outputs = self.batches.lock().unwrap().get_mut(&batch).map(|b| std::mem::take(&mut b.outputs));
        let outputs = outputs.unwrap_or_default();
        for p in &outputs {
            remove(p);
        }
        outputs.len()
    }

    pub fn copy(&self, batch: u64) -> Result<(), String> {
        let outputs = self.batches.lock().unwrap().get(&batch).map(|b| b.outputs.clone()).unwrap_or_default();
        crate::clipboard::copy_files(&outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names() {
        let dir = std::env::temp_dir().join("mutabor-test-names");
        let _ = std::fs::create_dir_all(&dir);
        let input = dir.join("clip.mp4");
        std::fs::write(&input, b"x").unwrap();
        // Same extension and no suffix must not overwrite the source.
        assert_eq!(output_path(&input, "", "mp4"), dir.join("clip (1).mp4"));
        assert_eq!(output_path(&input, "_compressed", "mp4"), dir.join("clip_compressed.mp4"));
        assert_eq!(output_path(&input, "_pages", ""), dir.join("clip_pages"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs the MuPDF and Pandoc recipes on generated samples.
    /// Needs `npm run tools -- --optional` first: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn tools_run_on_real_binaries() {
        let bin = |name: &str| {
            std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/binaries"))
                .expect("run `npm run tools -- --optional` first")
                .flatten()
                .map(|e| e.path())
                .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(&format!("{name}-")))
                .unwrap_or_else(|| panic!("{name} sidecar"))
        };
        let (ffmpeg, mutool, pandoc) = (bin("ffmpeg"), bin("mutool"), bin("pandoc"));
        let dir = std::env::temp_dir().join("mutabor-tools");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ok = |cmd: &mut Command| {
            let out = cmd.output().unwrap();
            assert!(out.status.success(), "{cmd:?}\n{}", String::from_utf8_lossy(&out.stderr));
        };

        // Our own image-to-PDF writer must produce a PDF MuPDF accepts.
        let jpg = dir.join("photo.jpg");
        ok(Command::new(&ffmpeg).args(["-v", "error", "-f", "lavfi", "-i", "testsrc=size=640x480", "-frames:v", "1"]).arg(&jpg));
        let one = dir.join("one.pdf");
        crate::pdf::convert(&jpg, &one, recipes::Page::A4).unwrap();
        assert_eq!(pdf_pages(&mutool, &one), Some(1));
        let two = dir.join("two.pdf");
        ok(Command::new(&mutool).args(["merge", "-o"]).arg(&two).arg(&one).arg(&one));
        assert_eq!(pdf_pages(&mutool, &two), Some(2));

        let run = |action: &str, inputs: &[PathBuf]| -> PathBuf {
            let ext = inputs[0].extension().unwrap().to_string_lossy().into_owned();
            let p = recipes::plan(action, &ext, Probe::default(), &Options::default()).unwrap();
            let program = if p.tool == Tool::Mutool { &mutool } else { &pandoc };
            let out = output_path(&inputs[0], &format!("_{action}{}", p.suffix), if p.dir { "" } else { &p.ext });
            if p.dir {
                std::fs::create_dir(&out).unwrap();
            }
            let pages: Vec<Option<u32>> = if p.per_page {
                (1..=pdf_pages(program, &inputs[0]).unwrap()).map(Some).collect()
            } else {
                vec![None]
            };
            for page in pages {
                ok(Command::new(program).args(expand(&p.args, inputs, &out, page)));
            }
            assert!(size_of(&out) > 0, "{action}: empty result");
            out
        };
        let count = |d: &Path| std::fs::read_dir(d).unwrap().count();
        assert_eq!(count(&run("pages_png", &[two.clone()])), 2);
        assert_eq!(count(&run("split", &[two.clone()])), 2);
        assert_eq!(pdf_pages(&mutool, &run("merge", &[one.clone(), two.clone()])), Some(3));
        run("extract_text", &[two.clone()]);
        run("compress", &[two.clone()]);

        let md = dir.join("book.md");
        std::fs::write(&md, "# Глава 1\n\nПривет, **мир**.\n").unwrap();
        for action in ["to_html", "to_epub", "to_fb2", "to_txt"] {
            run(action, &[md.clone()]);
        }
        let epub = dir.join("book_to_epub.epub");
        let back = run("to_md", &[epub]);
        assert!(std::fs::read_to_string(back).unwrap().contains("**мир**"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_command_lines() {
        assert_eq!(split_args(r#"-vf "scale=640:-2, hflip" -an"#), ["-vf", "scale=640:-2, hflip", "-an"]);
        assert_eq!(split_args("  -c:v  copy "), ["-c:v", "copy"]);
        assert_eq!(split_args("-metadata title=''"), ["-metadata", "title="]);
        assert_eq!(slug("Сжать для Discord!"), "_Сжать-для-Discord");
        assert_eq!(slug("!!"), "");
    }

    #[test]
    fn placeholders() {
        let inputs = [PathBuf::from("a.pdf"), PathBuf::from("b.pdf")];
        let args: Vec<String> = ["merge", "-o", "{out}/p-{page3}.pdf", "{in}", "{page}", "{inputs}"]
            .map(String::from)
            .to_vec();
        let v = expand(&args, &inputs, Path::new("out"), Some(7));
        assert_eq!(v, ["merge", "-o", "out/p-007.pdf", "a.pdf", "7", "a.pdf", "b.pdf"]);
    }
}
