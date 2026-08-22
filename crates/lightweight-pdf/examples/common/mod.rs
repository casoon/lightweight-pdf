//! Shared boilerplate for this crate's standalone examples: render, write
//! the PDF to disk, and report the byte count. Not auto-discovered as its
//! own example by Cargo (this directory has no `main.rs`) — each example
//! pulls it in via `#[path = "common/mod.rs"] mod common;`.

use lightweight_pdf::DocumentExt;

pub(crate) fn write_pdf(doc: &impl DocumentExt, filename: &str) {
    let bytes = doc.render().expect("render should succeed");
    std::fs::write(filename, &bytes).expect("write pdf");
    println!("wrote {filename} ({} bytes)", bytes.len());
}
