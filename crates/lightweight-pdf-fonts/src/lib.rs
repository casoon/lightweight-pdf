//! Font data and metrics for `lightweight-pdf`. Knows nothing about the document
//! model or PDF types (`plan/00a-contracts-and-artifacts.md`, point 3).

mod font;
mod subset;

pub use font::*;
pub use subset::*;
