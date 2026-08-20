use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique-per-call temp file name: `process::id()` alone collides between
/// tests running concurrently in the same test binary process.
fn unique_temp_path(prefix: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}-{}-{:?}-{n}.pdf", std::process::id(), std::thread::current().id()))
}

/// Runs `qpdf --check` on the given bytes via a temp file and returns
/// whether it succeeded, plus stdout+stderr for diagnostics.
pub fn qpdf_check(bytes: &[u8]) -> (bool, String) {
    let path = unique_temp_path("lightweight-pdf-qpdf-check");
    std::fs::write(&path, bytes).expect("write temp pdf");
    let output = Command::new("qpdf").arg("--check").arg(&path).output();
    let _ = std::fs::remove_file(&path);
    match output {
        Ok(out) => {
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text)
        }
        Err(e) => (false, format!("failed to run qpdf: {e}")),
    }
}

/// Extracts text via `pdftotext -layout` for assertions on visible content.
/// Not every test binary that includes this module uses every helper.
#[allow(dead_code)]
pub fn pdftotext(bytes: &[u8]) -> String {
    let path = unique_temp_path("lightweight-pdf-writertotext");
    std::fs::write(&path, bytes).expect("write temp pdf");
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg(&path)
        .arg("-")
        .output()
        .expect("run pdftotext");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Extracts text via `pdftotext -raw`, preserving content-stream glyph
/// order instead of reconstructing visual line layout — needed for rotated
/// text (`-layout`/default mode fragments diagonal runs across several
/// output lines, a poppler heuristic quirk unrelated to rendering
/// correctness).
#[allow(dead_code)]
pub fn pdftotext_raw(bytes: &[u8]) -> String {
    let path = unique_temp_path("lightweight-pdf-writertotext-raw");
    std::fs::write(&path, bytes).expect("write temp pdf");
    let output = Command::new("pdftotext")
        .arg("-raw")
        .arg(&path)
        .arg("-")
        .output()
        .expect("run pdftotext");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Extracts text for a single page (1-based) via `pdftotext -layout -f N -l N`.
#[allow(dead_code)]
pub fn pdftotext_page(bytes: &[u8], page_1based: usize) -> String {
    let path = unique_temp_path("lightweight-pdf-writertotext-page");
    std::fs::write(&path, bytes).expect("write temp pdf");
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-f")
        .arg(page_1based.to_string())
        .arg("-l")
        .arg(page_1based.to_string())
        .arg(&path)
        .arg("-")
        .output()
        .expect("run pdftotext");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Authoritative page count via `qpdf --show-npages` (do not derive this
/// from form-feed counting in `pdftotext` output — it is not reliable).
#[allow(dead_code)]
pub fn page_count(bytes: &[u8]) -> usize {
    let path = unique_temp_path("lightweight-pdf-count");
    std::fs::write(&path, bytes).expect("write temp pdf");
    let output = Command::new("qpdf").arg("--show-npages").arg(&path).output().expect("run qpdf");
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("qpdf --show-npages output")
}
