//! Issue #18: template + data JSON → `Document`, no Rust code involved.

use lightweight_pdf::*;
use lightweight_pdf_test_support as support;

fn read(relative: &str) -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../").to_string() + relative).expect("fixture file should exist")
}

#[test]
fn documented_invoice_template_and_data_produce_a_valid_pdf_with_all_line_items() {
    let template = read("examples/invoice-template.json");
    let data = read("examples/invoice-data.json");

    let doc = Document::from_template(&template, &data, MissingPlaceholder::Error).expect("template + data should resolve and parse");
    let (bytes, warnings) = doc.render_with_diagnostics().expect("should render");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("RE-2026-0100"));
    assert!(extracted.contains("Acme Software GmbH"));
    assert!(extracted.contains("Beratung Softwarearchitektur"));
    assert!(extracted.contains("Reisekosten"));
    assert!(extracted.contains("1.200,00"));
}

#[test]
fn missing_data_field_is_a_clear_error_by_default() {
    let result = Document::from_template(
        r#"{"schema_version":1,"document":{"page_format":"A4","children":[
            {"type":"text","content":"{{missing.field}}"}
        ]}}"#,
        r#"{}"#,
        MissingPlaceholder::Error,
    );
    let Err(err) = result else {
        panic!("a missing placeholder must fail by default");
    };
    let message = err.to_string();
    assert!(
        message.contains("missing.field"),
        "expected the error to name the path, got: {message}"
    );
}

#[test]
fn missing_data_field_can_resolve_to_empty_instead_of_erroring() {
    let doc = Document::from_template(
        r#"{"schema_version":1,"document":{"page_format":"A4","children":[
            {"type":"text","content":"before-{{missing.field}}-after"}
        ]}}"#,
        r#"{}"#,
        MissingPlaceholder::Empty,
    )
    .expect("MissingPlaceholder::Empty must not fail");
    let (bytes, warnings) = doc.render_with_diagnostics().expect("should render");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("before--after"), "got: {extracted}");
}
