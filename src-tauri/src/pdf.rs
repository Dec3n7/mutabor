//! Writes a one-page PDF around a JPEG. The JPEG is embedded as is
//! (DCTDecode), so there is no second lossy pass.

use crate::recipes::Page;
use std::io::Write;
use std::path::Path;

/// A4 in points.
const A4: (f64, f64) = (595.28, 841.89);
const MARGIN: f64 = 28.35; // 1 cm
/// 96 dpi: one pixel is 0.75 pt.
const PT_PER_PX: f64 = 0.75;

#[derive(Debug, PartialEq)]
pub struct JpegInfo {
    pub width: u32,
    pub height: u32,
    pub components: u8,
}

/// Reads the frame header (SOFn) of a JPEG.
pub fn jpeg_info(data: &[u8]) -> Option<JpegInfo> {
    if data.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marker = data[i + 1];
        // Fill bytes and markers without a length.
        if marker == 0xFF || (0xD0..=0xD9).contains(&marker) || marker == 0x01 {
            i += if marker == 0xFF { 1 } else { 2 };
            continue;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        let is_sof = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_sof {
            let seg = data.get(i + 4..i + 2 + len)?;
            return Some(JpegInfo {
                height: u16::from_be_bytes([seg[1], seg[2]]) as u32,
                width: u16::from_be_bytes([seg[3], seg[4]]) as u32,
                components: seg[5],
            });
        }
        i += 2 + len;
    }
    None
}

/// Page size and the image's placement on it, all in points.
fn layout(w: u32, h: u32, page: Page) -> ((f64, f64), (f64, f64, f64, f64)) {
    let (iw, ih) = (w as f64 * PT_PER_PX, h as f64 * PT_PER_PX);
    match page {
        Page::Original => ((iw, ih), (0.0, 0.0, iw, ih)),
        Page::A4 => {
            let (pw, ph) = if w > h { (A4.1, A4.0) } else { A4 };
            // Fit inside the margins, never enlarging past the natural size.
            let scale = ((pw - 2.0 * MARGIN) / iw).min((ph - 2.0 * MARGIN) / ih).min(1.0);
            let (dw, dh) = (iw * scale, ih * scale);
            ((pw, ph), ((pw - dw) / 2.0, (ph - dh) / 2.0, dw, dh))
        }
    }
}

pub fn jpeg_to_pdf(jpeg: &[u8], page: Page) -> Result<Vec<u8>, String> {
    let info = jpeg_info(jpeg).ok_or("not a JPEG")?;
    let color = match info.components {
        1 => "/DeviceGray",
        3 => "/DeviceRGB",
        // Adobe CMYK JPEGs store inverted values.
        4 => "/DeviceCMYK /Decode [1 0 1 0 1 0 1 0]",
        _ => return Err("unsupported JPEG colour model".into()),
    };
    let ((pw, ph), (x, y, dw, dh)) = layout(info.width, info.height, page);
    let content = format!("q {dw:.2} 0 0 {dh:.2} {x:.2} {y:.2} cm /Im0 Do Q");

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    let mut obj = |out: &mut Vec<u8>, head: String, stream: Option<&[u8]>| {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{head}\n", offsets.len());
        if let Some(bytes) = stream {
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(bytes);
            out.extend_from_slice(b"\nendstream\n");
        }
        out.extend_from_slice(b"endobj\n");
    };
    obj(&mut out, "<< /Type /Catalog /Pages 2 0 R >>".into(), None);
    obj(&mut out, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(), None);
    obj(
        &mut out,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] \
             /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>"
        ),
        None,
    );
    obj(
        &mut out,
        format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {color} \
             /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>",
            info.width,
            info.height,
            jpeg.len()
        ),
        Some(jpeg),
    );
    obj(&mut out, format!("<< /Length {} >>", content.len()), Some(content.as_bytes()));

    let xref = out.len();
    let _ = write!(out, "xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
    for off in &offsets {
        let _ = write!(out, "{off:010} 00000 n \n");
    }
    let _ = write!(out, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1);
    Ok(out)
}

pub fn convert(jpeg_path: &Path, pdf_path: &Path, page: Page) -> Result<(), String> {
    let jpeg = std::fs::read(jpeg_path).map_err(|e| e.to_string())?;
    std::fs::write(pdf_path, jpeg_to_pdf(&jpeg, page)?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smallest useful JPEG header: SOI, APP0 stub, SOF0 for 300x200 RGB.
    fn header() -> Vec<u8> {
        let mut d = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        d.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0xC8, 0x01, 0x2C, 0x03]);
        d.extend([0; 9]);
        d
    }

    #[test]
    fn reads_size() {
        assert_eq!(jpeg_info(&header()), Some(JpegInfo { width: 300, height: 200, components: 3 }));
        assert_eq!(jpeg_info(b"not a jpeg"), None);
    }

    #[test]
    fn landscape_fits_a4() {
        let ((pw, ph), (x, y, dw, dh)) = layout(4000, 3000, Page::A4);
        assert!(pw > ph, "landscape image, landscape page");
        assert!(x >= MARGIN - 0.01 && y >= MARGIN - 0.01 && dw <= pw && dh <= ph);
        // Small pictures keep their natural size instead of being blown up.
        let (_, (_, _, dw, _)) = layout(100, 100, Page::A4);
        assert_eq!(dw, 75.0);
    }

    #[test]
    fn xref_points_at_objects() {
        let pdf = jpeg_to_pdf(&header(), Page::Original).unwrap();
        // Offsets are in bytes; the binary comment line rules out a lossy string.
        let tail = std::str::from_utf8(&pdf[pdf.len() - 40..]).unwrap();
        let xref: usize = tail.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(pdf[xref..].starts_with(b"xref"));
        let table = std::str::from_utf8(&pdf[xref..]).unwrap();
        let first: usize = table.lines().nth(3).unwrap()[..10].parse().unwrap();
        assert!(pdf[first..].starts_with(b"1 0 obj"));
    }
}
