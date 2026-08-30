//! End-to-end tests for the `lwpdf` binary itself (issue #19) — spawns
//! the actual compiled executable via Cargo's `CARGO_BIN_EXE_lwpdf` (no
//! extra dependency needed for that), rather than testing the library
//! logic directly.

use std::process::Command;

fn lwpdf() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lwpdf"))
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn help_documents_every_subcommand() {
    let output = lwpdf().arg("--help").output().expect("lwpdf --help should run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for name in ["render", "validate", "fonts", "schema"] {
        assert!(stdout.contains(name), "expected --help to document {name:?}, got:\n{stdout}");
    }
}

#[test]
fn schema_prints_a_valid_json_schema_for_the_document_format() {
    let output = lwpdf().arg("schema").output().expect("lwpdf schema should run");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("schema output should be valid JSON");
    assert_eq!(value["title"], "DocumentSchema");
    assert!(
        value["$defs"]["Document"].is_object(),
        "expected a Document definition, got:\n{stdout}"
    );
}

#[test]
fn fonts_lists_the_bundled_default_weights() {
    let output = lwpdf().arg("fonts").output().expect("lwpdf fonts should run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sans-regular"));
    assert!(stdout.contains("sans-bold"));
}

#[test]
fn render_from_template_and_data_produces_a_pdf_and_exits_zero() {
    let out_path = std::env::temp_dir().join("lwpdf-cli-test-render.pdf");
    // Best-effort: clear a leftover file from a previous failed run. Not an
    // error if there's nothing to remove — the assertions below are what
    // actually verify this test's outcome, not this cleanup.
    let _ = std::fs::remove_file(&out_path);

    let output = lwpdf()
        .current_dir(repo_root())
        .args([
            "render",
            "examples/invoice-template.json",
            "--data",
            "examples/invoice-data.json",
            "-o",
        ])
        .arg(&out_path)
        .output()
        .expect("lwpdf render should run");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_path.exists(), "expected {out_path:?} to be written");
    let bytes = std::fs::read(&out_path).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "expected a PDF file");
    // Best-effort cleanup — this test's assertions already ran; a failure
    // to remove the temp file isn't this test's concern.
    std::fs::remove_file(&out_path).ok();
}

#[test]
fn validate_exits_zero_for_a_valid_document() {
    let output = lwpdf()
        .current_dir(repo_root())
        .args(["validate", "examples/document.json"])
        .output()
        .expect("lwpdf validate should run");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn missing_input_file_exits_with_code_two() {
    let output = lwpdf()
        .args(["validate", "/nonexistent/path/does-not-exist.json"])
        .output()
        .expect("lwpdf validate should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn render_error_exits_with_code_one() {
    let template_path = std::env::temp_dir().join("lwpdf-cli-test-render-error.json");
    std::fs::write(
        &template_path,
        r#"{"schema_version":1,"document":{"page_format":"A4","children":[
            {"type":"text","content":"hi","style":{"font":"sans-italic"}}
        ]}}"#,
    )
    .unwrap();
    let out_path = std::env::temp_dir().join("lwpdf-cli-test-render-error.pdf");

    let output = lwpdf()
        .args(["render", template_path.to_str().unwrap(), "-o", out_path.to_str().unwrap()])
        .output()
        .expect("lwpdf render should run");

    assert_eq!(output.status.code(), Some(1), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    // Best-effort cleanup — this test's assertion already ran.
    std::fs::remove_file(&template_path).ok();
}
