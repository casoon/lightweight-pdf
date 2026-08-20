//! Minimal, zero-dependency PDF object/xref/stream writer for `lightweight-pdf`.
//! Never sees `Element`/`RenderNode` — only flat write operations
//! (`plan/00a-contracts-and-artifacts.md`, point 3).

mod content;
mod doc;
mod writer;

pub use content::*;
pub use doc::*;
pub use writer::*;
