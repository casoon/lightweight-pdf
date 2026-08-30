//! Pixel-diff snapshot testing for rendered PDFs (issue #21): render →
//! rasterize (`pdftoppm`, part of `poppler-utils` — already a required
//! tool for this workspace's own text-extraction tests, so this adds no
//! new system dependency) → compare each page against a checked-in
//! reference PNG, with a small per-pixel tolerance for renderer noise.
//!
//! Deliberately independent of `lightweight-pdf-test-support` (that
//! crate is this workspace's own internal dev-dependency, never
//! published) — this crate stands on its own, usable to pin *any* PDF
//! (not just ones built with `lightweight-pdf`) against visual
//! regressions.
//!
//! Reference images are low-DPI grayscale PNG (`DEFAULT_DPI`) on purpose
//! — this is a regression trip-wire, not a print-quality visual proof,
//! and keeping them small keeps the repository's history small.
//!
//! ```no_run
//! # fn render() -> Vec<u8> { vec![] }
//! let dir = std::path::Path::new("test-fixtures/snapshots");
//! lightweight_pdf_testing::assert_snapshot(dir, "invoice", &render());
//! ```
//!
//! Set `UPDATE_SNAPSHOTS=1` to (re)write the reference images instead of
//! comparing against them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Low on purpose (see module doc) — this is a regression trip-wire, not
/// a print-quality visual proof.
pub const DEFAULT_DPI: u32 = 72;
/// Maximum per-pixel grayscale value difference (0-255) still counted as
/// "the same" — absorbs the small amount of renderer anti-aliasing noise
/// between otherwise-identical runs.
pub const DEFAULT_TOLERANCE: u8 = 12;

#[derive(Debug)]
pub enum SnapshotError {
    Rasterize(String),
    Decode(String),
    /// No reference image exists yet for this page — not necessarily a
    /// bug, just needs a `UPDATE_SNAPSHOTS=1` run once.
    NoReference(PathBuf),
    /// The rendered document has a different number of pages than the
    /// checked-in reference set.
    PageCountMismatch {
        expected: usize,
        actual: usize,
    },
    Mismatch {
        page: usize,
        reference_path: PathBuf,
        diff_path: PathBuf,
        differing_pixels: usize,
        total_pixels: usize,
    },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::Rasterize(msg) => write!(f, "rasterizing the PDF failed: {msg}"),
            SnapshotError::Decode(msg) => write!(f, "decoding a snapshot PNG failed: {msg}"),
            SnapshotError::NoReference(path) => {
                write!(
                    f,
                    "no reference snapshot at {} — run once with UPDATE_SNAPSHOTS=1 to create it",
                    path.display()
                )
            }
            SnapshotError::PageCountMismatch { expected, actual } => {
                write!(f, "expected {expected} page(s) (reference), rendered {actual}")
            }
            SnapshotError::Mismatch {
                page,
                reference_path,
                diff_path,
                differing_pixels,
                total_pixels,
            } => {
                write!(
                    f,
                    "page {page} doesn't match {} — {differing_pixels}/{total_pixels} pixels differ beyond tolerance; diff image written to {}",
                    reference_path.display(),
                    diff_path.display()
                )
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

/// `check_snapshot` with `DEFAULT_DPI`/`DEFAULT_TOLERANCE`, panicking
/// with a descriptive message on any failure — the usual entry point
/// from a `#[test]` function.
pub fn assert_snapshot(snapshot_dir: &Path, name: &str, pdf_bytes: &[u8]) {
    if let Err(err) = check_snapshot(snapshot_dir, name, pdf_bytes, DEFAULT_DPI, DEFAULT_TOLERANCE) {
        panic!("{err}");
    }
}

/// Rasterizes `pdf_bytes` at `dpi` and compares every page against
/// `<snapshot_dir>/<name>-<page>.png`. With `UPDATE_SNAPSHOTS` set (any
/// value), (re)writes the reference images instead of comparing.
pub fn check_snapshot(snapshot_dir: &Path, name: &str, pdf_bytes: &[u8], dpi: u32, tolerance: u8) -> Result<(), SnapshotError> {
    let rendered_pages = rasterize(pdf_bytes, dpi)?;

    std::fs::create_dir_all(snapshot_dir).map_err(|e| SnapshotError::Rasterize(format!("create {}: {e}", snapshot_dir.display())))?;

    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        // Remove any reference pages beyond the newly-rendered count, so
        // a page-count shrink doesn't leave a stale reference behind.
        let mut stale = rendered_pages.len() + 1;
        while reference_path(snapshot_dir, name, stale).exists() {
            // Best-effort: `.exists()` above already confirmed there's
            // something to remove; a failure here just leaves the stale
            // file for next time, it doesn't affect this run's snapshots.
            std::fs::remove_file(reference_path(snapshot_dir, name, stale)).ok();
            stale += 1;
        }
        for (i, page_png) in rendered_pages.iter().enumerate() {
            let path = reference_path(snapshot_dir, name, i + 1);
            let gray = decode_to_gray(page_png)?;
            write_gray_png(&path, gray.width, gray.height, &gray.pixels)?;
        }
        eprintln!("updated {} snapshot page(s) for {name:?}", rendered_pages.len());
        return Ok(());
    }

    if reference_path(snapshot_dir, name, rendered_pages.len() + 1).exists() {
        let mut expected = rendered_pages.len() + 1;
        while reference_path(snapshot_dir, name, expected + 1).exists() {
            expected += 1;
        }
        return Err(SnapshotError::PageCountMismatch {
            expected,
            actual: rendered_pages.len(),
        });
    }

    for (i, rendered_png) in rendered_pages.iter().enumerate() {
        let page = i + 1;
        let reference_path = reference_path(snapshot_dir, name, page);
        if !reference_path.exists() {
            return Err(SnapshotError::NoReference(reference_path));
        }
        let reference_png =
            std::fs::read(&reference_path).map_err(|e| SnapshotError::Decode(format!("read {}: {e}", reference_path.display())))?;
        compare_page(page, &reference_path, &reference_png, rendered_png, tolerance)?;
    }

    Ok(())
}

fn reference_path(snapshot_dir: &Path, name: &str, page: usize) -> PathBuf {
    snapshot_dir.join(format!("{name}-{page}.png"))
}

static UNIQUE: AtomicU64 = AtomicU64::new(0);

/// Rasterizes `pdf_bytes` at `dpi` via `pdftoppm -gray -png`, returning
/// each page's raw PNG bytes in order — `pdftoppm` always numbers pages
/// from 1 (`<prefix>-1.png`, `<prefix>-2.png`, ...), even for a
/// single-page document.
fn rasterize(pdf_bytes: &[u8], dpi: u32) -> Result<Vec<Vec<u8>>, SnapshotError> {
    let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("lightweight-pdf-testing-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| SnapshotError::Rasterize(format!("create temp dir: {e}")))?;
    let pdf_path = dir.join("input.pdf");
    std::fs::write(&pdf_path, pdf_bytes).map_err(|e| SnapshotError::Rasterize(format!("write temp pdf: {e}")))?;
    let prefix = dir.join("page");

    let output = Command::new("pdftoppm")
        .arg("-gray")
        .arg("-png")
        .arg("-r")
        .arg(dpi.to_string())
        .arg(&pdf_path)
        .arg(&prefix)
        .output()
        .map_err(|e| SnapshotError::Rasterize(format!("run pdftoppm: {e}")))?;
    if !output.status.success() {
        // Best-effort cleanup of the temp dir before returning the real
        // error below — a failure to remove it doesn't change the outcome
        // of this rasterize call.
        std::fs::remove_dir_all(&dir).ok();
        return Err(SnapshotError::Rasterize(format!(
            "pdftoppm failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let mut pages = Vec::new();
    let mut page = 1usize;
    loop {
        let path = dir.join(format!("page-{page}.png"));
        let Ok(bytes) = std::fs::read(&path) else { break };
        pages.push(bytes);
        page += 1;
    }
    // Best-effort cleanup — the PNG bytes are already read into `pages`
    // above, so a failure to remove the temp dir doesn't affect the result.
    std::fs::remove_dir_all(&dir).ok();
    Ok(pages)
}

struct GrayImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Decodes any 8-bit PNG `pdftoppm -gray` might actually emit and
/// normalizes it to one grayscale byte per pixel. Despite the `-gray`
/// flag, poppler has been observed to still emit a Truecolor (RGB) PNG
/// with R == G == B rather than a literal single-channel grayscale one —
/// every channel layout it could plausibly produce is handled here so
/// callers never have to care, and everything this crate itself *writes*
/// (`write_gray_png`) is always the literal single-channel format
/// regardless of what pdftoppm handed us, keeping reference files small.
fn decode_to_gray(bytes: &[u8]) -> Result<GrayImage, SnapshotError> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| SnapshotError::Decode(e.to_string()))?;
    let mut buf = vec![
        0u8;
        reader
            .output_buffer_size()
            .ok_or_else(|| SnapshotError::Decode("empty image".into()))?
    ];
    let info = reader.next_frame(&mut buf).map_err(|e| SnapshotError::Decode(e.to_string()))?;
    buf.truncate(info.buffer_size());
    if info.bit_depth != png::BitDepth::Eight {
        return Err(SnapshotError::Decode(format!("expected an 8-bit PNG, got {:?}", info.bit_depth)));
    }
    let pixels = match info.color_type {
        png::ColorType::Grayscale => buf,
        png::ColorType::GrayscaleAlpha => buf.as_chunks::<2>().0.iter().map(|px| px[0]).collect(),
        png::ColorType::Rgb => buf.as_chunks::<3>().0.iter().map(|px| px[0]).collect(),
        png::ColorType::Rgba => buf.as_chunks::<4>().0.iter().map(|px| px[0]).collect(),
        other => return Err(SnapshotError::Decode(format!("unsupported PNG color type {other:?}"))),
    };
    Ok(GrayImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

fn compare_page(page: usize, reference_path: &Path, reference_png: &[u8], rendered_png: &[u8], tolerance: u8) -> Result<(), SnapshotError> {
    let reference = decode_to_gray(reference_png)?;
    let rendered = decode_to_gray(rendered_png)?;

    if reference.width != rendered.width || reference.height != rendered.height {
        return Err(SnapshotError::Decode(format!(
            "page {page}: reference is {}x{}, rendered is {}x{} — DPI mismatch?",
            reference.width, reference.height, rendered.width, rendered.height
        )));
    }

    let mut differing = 0usize;
    let mut diff_pixels = Vec::with_capacity(reference.pixels.len());
    for (&r, &v) in reference.pixels.iter().zip(&rendered.pixels) {
        let delta = r.abs_diff(v);
        if delta > tolerance {
            differing += 1;
            diff_pixels.push(255u8); // highlight in the diff image
        } else {
            diff_pixels.push(0u8);
        }
    }

    if differing == 0 {
        return Ok(());
    }

    let diff_path = reference_path.with_extension("diff.png");
    write_gray_png(&diff_path, reference.width, reference.height, &diff_pixels)?;
    Err(SnapshotError::Mismatch {
        page,
        reference_path: reference_path.to_path_buf(),
        diff_path,
        differing_pixels: differing,
        total_pixels: reference.pixels.len(),
    })
}

fn write_gray_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), SnapshotError> {
    let file = std::fs::File::create(path).map_err(|e| SnapshotError::Rasterize(format!("create {}: {e}", path.display())))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| SnapshotError::Rasterize(e.to_string()))?;
    writer
        .write_image_data(pixels)
        .map_err(|e| SnapshotError::Rasterize(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_pdf() -> Vec<u8> {
        // Minimal one-page, blank PDF — enough for `pdftoppm` to rasterize
        // without needing the rest of this workspace's writer.
        b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF"
            .to_vec()
    }

    // All three scenarios live in one `#[test]` function rather than
    // three: `check_snapshot`'s only way to switch into "write the
    // reference" mode is the process-global `UPDATE_SNAPSHOTS` env var,
    // and `cargo test` runs `#[test]` functions on separate threads of
    // the *same* process by default — separate tests toggling a shared
    // env var would race each other.
    #[test]
    fn snapshot_lifecycle() {
        let dir = std::env::temp_dir().join(format!("lightweight-pdf-testing-test-{}", std::process::id()));
        // Best-effort: clear a leftover dir from a previous failed run;
        // `create_dir_all` inside `check_snapshot` below is what actually
        // needs to succeed.
        std::fs::remove_dir_all(&dir).ok();
        let pdf = tiny_pdf();

        // 1. No reference yet.
        let err = check_snapshot(&dir, "blank", &pdf, DEFAULT_DPI, DEFAULT_TOLERANCE).unwrap_err();
        assert!(matches!(err, SnapshotError::NoReference(_)), "got: {err}");

        // 2. UPDATE_SNAPSHOTS=1 writes it, then a normal comparison
        // against the identical PDF succeeds.
        std::env::set_var("UPDATE_SNAPSHOTS", "1");
        check_snapshot(&dir, "blank", &pdf, DEFAULT_DPI, DEFAULT_TOLERANCE).expect("update should succeed");
        std::env::remove_var("UPDATE_SNAPSHOTS");
        check_snapshot(&dir, "blank", &pdf, DEFAULT_DPI, DEFAULT_TOLERANCE)
            .expect("comparing against the just-written reference should succeed");

        // 3. A visibly different page is reported as a mismatch, with a
        // diff image written next to the reference.
        let different_pdf = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R/Resources<<>>>>endobj\n4 0 obj<</Length 40>>stream\n0 0 0 rg 0 0 200 200 re f\nendstream endobj\ntrailer<</Root 1 0 R>>\n%%EOF".to_vec();
        let err = check_snapshot(&dir, "blank", &different_pdf, DEFAULT_DPI, DEFAULT_TOLERANCE).unwrap_err();
        let SnapshotError::Mismatch { diff_path, .. } = &err else {
            panic!("got: {err}");
        };
        assert!(diff_path.exists(), "expected a diff image at {}", diff_path.display());

        // Best-effort cleanup — this test's assertions already ran.
        std::fs::remove_dir_all(&dir).ok();
    }
}
