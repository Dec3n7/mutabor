//! Background removal: IS-Net (the model rembg calls "isnet-general-use")
//! run by tract, a pure-Rust ONNX runtime compiled into the app, so only the
//! model is downloaded. ffmpeg does the resizing and the final alpha merge.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tract_onnx::prelude::*;

/// IS-Net works on 1024 × 1024 pictures.
pub const SIZE: usize = 1024;

/// The model as tools/manifest.json downloads it.
pub const MODEL_FILE: &str = "isnet-general-use.onnx";

type Runnable = std::sync::Arc<TypedSimplePlan>;

/// The optimised model stays loaded between pictures; one inference at a
/// time, since a single run already uses all cores and plenty of memory.
static MODEL: Mutex<Option<(PathBuf, Runnable)>> = Mutex::new(None);

fn load(path: &Path) -> TractResult<Runnable> {
    tract_onnx::onnx()
        .model_for_path(path)?
        .with_input_fact(0, f32::fact([1, 3, SIZE, SIZE]).into())?
        .into_optimized()?
        .into_runnable()
}

/// The foreground mask (0 = background, 255 = subject) for a SIZE × SIZE
/// RGB picture given as packed bytes.
pub fn mask(model_path: &Path, rgb: &[u8]) -> Result<Vec<u8>, String> {
    if rgb.len() != SIZE * SIZE * 3 {
        return Err(format!("expected {} bytes of RGB, got {}", SIZE * SIZE * 3, rgb.len()));
    }
    let mut slot = MODEL.lock().unwrap();
    if slot.as_ref().is_none_or(|(p, _)| p != model_path) {
        *slot = Some((model_path.to_path_buf(), load(model_path).map_err(|e| format!("model: {e}"))?));
    }
    let (_, model) = slot.as_ref().expect("loaded above");

    // Same preprocessing as rembg: scale by the brightest value, centre on 0.5.
    let max = rgb.iter().copied().max().unwrap_or(255).max(1) as f32;
    let input: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, SIZE, SIZE), |(_, c, y, x)| {
        rgb[(y * SIZE + x) * 3 + c] as f32 / max - 0.5
    })
    .into();
    let outputs = model.run(tvec!(input.into())).map_err(|e| format!("inference: {e}"))?;
    let pred = outputs[0].to_plain_array_view::<f32>().map_err(|e| e.to_string())?;
    let values: Vec<f32> = pred.iter().copied().take(SIZE * SIZE).collect();
    if values.len() != SIZE * SIZE {
        return Err("unexpected model output".into());
    }
    let (lo, hi) = values.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let span = (hi - lo).max(1e-6);
    Ok(values.iter().map(|v| (((v - lo) / span) * 255.0).round() as u8).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the model in ~/Downloads: `cargo test matting -- --ignored`.
    /// A dark square on a light background must come out as the subject.
    #[test]
    #[ignore]
    fn finds_the_subject() {
        let model = PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default())
            .join("Downloads")
            .join("isnet-general-use.onnx");
        let mut rgb = vec![235u8; SIZE * SIZE * 3];
        for y in 384..640 {
            for x in 384..640 {
                let i = (y * SIZE + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[180, 30, 30]);
            }
        }
        let started = std::time::Instant::now();
        let m = mask(&model, &rgb).unwrap();
        eprintln!("inference took {:?}", started.elapsed());
        let at = |x: usize, y: usize| m[y * SIZE + x];
        assert!(at(512, 512) > 128, "centre should be subject, got {}", at(512, 512));
        assert!(at(40, 40) < 128, "corner should be background, got {}", at(40, 40));
    }
}
