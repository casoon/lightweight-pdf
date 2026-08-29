//! Shared shell-out helpers (`qpdf`/`pdftotext`) for `lightweight-pdf`'s
//! integration tests. A real crate rather than a per-file `tests/support/mod.rs`
//! module: each `crates/lightweight-pdf/tests/*.rs` file compiles as its own
//! binary crate, so a shared module included via `mod support;` gets
//! recompiled once per test file and needs `#[allow(dead_code)]` on any
//! helper a given file doesn't call. A dev-dependency library crate is
//! compiled once, and `pub` items in a `lib` crate aren't subject to that
//! per-binary dead-code check at all.
//!
//! Every fallible step returns `Result` instead of panicking, so nothing
//! here is a panicking construct reachable from a `pub` path — callers
//! (test functions, which are not part of any public API) `.unwrap()`/`.expect()`
//! at the call site instead.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique-per-call temp file name: `process::id()` alone collides between
/// tests running concurrently in the same test binary process.
fn unique_temp_path(prefix: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}-{}-{:?}-{n}.pdf", std::process::id(), std::thread::current().id()))
}

fn remove_temp_file(path: &Path) {
    if let Err(err) = std::fs::remove_file(path) {
        eprintln!("warning: failed to remove temp file {}: {err}", path.display());
    }
}

/// Writes `bytes` to a fresh unique temp file, runs `f` against its path,
/// then removes the temp file regardless of `f`'s outcome — the shared
/// lifecycle every helper below needs around its own external-tool call.
fn with_temp_pdf<T>(bytes: &[u8], prefix: &str, f: impl FnOnce(&Path) -> Result<T, String>) -> Result<T, String> {
    let path = unique_temp_path(prefix);
    std::fs::write(&path, bytes).map_err(|e| format!("write temp pdf: {e}"))?;
    let result = f(&path);
    remove_temp_file(&path);
    result
}

/// Runs `pdftotext <extra_args> <path> -` and returns stdout as a `String`.
fn run_pdftotext(path: &Path, extra_args: &[&str]) -> Result<String, String> {
    let output = Command::new("pdftotext")
        .args(extra_args)
        .arg(path)
        .arg("-")
        .output()
        .map_err(|e| format!("run pdftotext: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Runs `qpdf --check` on the given bytes via a temp file and returns
/// whether it succeeded, plus stdout+stderr for diagnostics.
pub fn qpdf_check(bytes: &[u8]) -> Result<(bool, String), String> {
    with_temp_pdf(bytes, "lightweight-pdf-qpdf-check", |path| {
        Ok(match Command::new("qpdf").arg("--check").arg(path).output() {
            Ok(out) => {
                let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                (out.status.success(), text)
            }
            Err(e) => (false, format!("failed to run qpdf: {e}")),
        })
    })
}

/// Runs `qpdf --stream-data=uncompress` and returns the resulting bytes —
/// same PDF, every stream decompressed back to plain text. Tests that
/// string-search content-stream operators (a `cm` matrix, a color
/// operator, a `Tj` count) need this now that streams are `/FlateDecode`-
/// compressed by default (ADR-016); string-searching `bytes` directly
/// only still works for content that never lived in a stream (dict
/// entries like `/Title`, `/Annots`, `/Outlines`).
pub fn decompressed(bytes: &[u8]) -> Result<Vec<u8>, String> {
    with_temp_pdf(bytes, "lightweight-pdf-decompress-in", |in_path| {
        let out_path = in_path.with_extension("out.pdf");
        let output = Command::new("qpdf")
            .arg("--stream-data=uncompress")
            .arg(in_path)
            .arg(&out_path)
            .output()
            .map_err(|e| format!("run qpdf: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "qpdf --stream-data=uncompress failed:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let result = std::fs::read(&out_path).map_err(|e| format!("read decompressed pdf: {e}"));
        remove_temp_file(&out_path);
        result
    })
}

/// Extracts text via `pdftotext -layout` for assertions on visible content.
pub fn pdftotext(bytes: &[u8]) -> Result<String, String> {
    with_temp_pdf(bytes, "lightweight-pdf-writertotext", |path| run_pdftotext(path, &["-layout"]))
}

/// Extracts text via `pdftotext -raw`, preserving content-stream glyph
/// order instead of reconstructing visual line layout — needed for rotated
/// text (`-layout`/default mode fragments diagonal runs across several
/// output lines, a poppler heuristic quirk unrelated to rendering
/// correctness).
pub fn pdftotext_raw(bytes: &[u8]) -> Result<String, String> {
    with_temp_pdf(bytes, "lightweight-pdf-writertotext-raw", |path| run_pdftotext(path, &["-raw"]))
}

/// Extracts text for a single page (1-based) via `pdftotext -layout -f N -l N`.
pub fn pdftotext_page(bytes: &[u8], page_1based: usize) -> Result<String, String> {
    let page = page_1based.to_string();
    with_temp_pdf(bytes, "lightweight-pdf-writertotext-page", |path| {
        run_pdftotext(path, &["-layout", "-f", &page, "-l", &page])
    })
}

/// Runs `pdfinfo` and returns stdout as a `String` — reads back `/Info`
/// dictionary fields (Title, Author, CreationDate, Producer, ...) the way a
/// consumer of the PDF actually would, rather than grepping raw bytes.
pub fn pdfinfo(bytes: &[u8]) -> Result<String, String> {
    with_temp_pdf(bytes, "lightweight-pdf-pdfinfo", |path| {
        let output = Command::new("pdfinfo")
            .arg(path)
            .output()
            .map_err(|e| format!("run pdfinfo: {e}"))?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
}

/// Authoritative page count via `qpdf --show-npages` (do not derive this
/// from form-feed counting in `pdftotext` output — it is not reliable).
pub fn page_count(bytes: &[u8]) -> Result<usize, String> {
    with_temp_pdf(bytes, "lightweight-pdf-count", |path| {
        let output = Command::new("qpdf")
            .arg("--show-npages")
            .arg(path)
            .output()
            .map_err(|e| format!("run qpdf: {e}"))?;
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .map_err(|e| format!("parse qpdf --show-npages output: {e}"))
    })
}
