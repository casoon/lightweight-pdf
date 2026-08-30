//! `lwpdf` (issue #19): renders a JSON document — or a JSON template plus
//! a separate data document (issue #18) — to a PDF file, no Rust code
//! needed. Deliberately its own crate: `clap` and everything it pulls in
//! stays out of the `lightweight-pdf` library's own dependency tree.

use clap::{Parser, Subcommand};
use lightweight_pdf::*;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "lwpdf",
    version,
    about = "Render PDFs from JSON documents/templates — no Rust code needed."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Renders a JSON document (or template + data) to a PDF file.
    Render {
        /// Path to a document JSON file, or a template JSON file if
        /// `--data` is also given.
        input: PathBuf,
        /// Data JSON file — when given, `input` is treated as a template
        /// with `{{placeholders}}`/`$each` (issue #18), resolved against
        /// this data before rendering.
        #[arg(long)]
        data: Option<PathBuf>,
        /// Output PDF path.
        #[arg(short, long)]
        output: PathBuf,
        /// A missing template placeholder resolves to an empty string
        /// instead of failing.
        #[arg(long)]
        allow_missing: bool,
    },
    /// Parses (and, with `--data`, resolves) a document/template without
    /// rendering it — checks the input is valid before spending the time
    /// to actually produce a PDF.
    Validate {
        input: PathBuf,
        #[arg(long)]
        data: Option<PathBuf>,
        #[arg(long)]
        allow_missing: bool,
    },
    /// Lists the font weights available by default (bundled Source Sans 3).
    Fonts,
    /// Prints the JSON Schema for the document/template format to stdout
    /// — generated from the Rust types (`schemars`), not hand-maintained;
    /// the npm package's TypeScript types are generated from this same
    /// schema (issue #22).
    Schema,
}

enum CliError {
    /// Bad input: missing/unreadable file, malformed JSON, an unresolved
    /// template placeholder, ... — exit code 2.
    Input(String),
    /// The input parsed fine but rendering itself failed (e.g. a missing
    /// font weight) — exit code 1.
    Render(String),
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::from(0),
        Err(CliError::Input(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
        Err(CliError::Render(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}

fn read_to_string(path: &Path) -> Result<String, CliError> {
    std::fs::read_to_string(path).map_err(|e| CliError::Input(format!("reading {}: {e}", path.display())))
}

/// Shared by `render`/`validate`: loads `input` as a plain document, or —
/// if `data` is given — as a template resolved against it.
fn load_document(input: &Path, data: Option<&PathBuf>, allow_missing: bool) -> Result<Document, CliError> {
    let input_json = read_to_string(input)?;
    let on_missing = if allow_missing {
        MissingPlaceholder::Empty
    } else {
        MissingPlaceholder::Error
    };
    let result = match data {
        Some(data_path) => {
            let data_json = read_to_string(data_path)?;
            Document::from_template(&input_json, &data_json, on_missing)
        }
        None => Document::from_json(&input_json),
    };
    result.map_err(|e| CliError::Input(e.to_string()))
}

fn describe_warning(warning: &LayoutWarning) -> String {
    let kind = match warning.kind {
        LayoutWarningKind::TextClipped => "text clipped".to_string(),
        LayoutWarningKind::ContentOverflow => "content overflow".to_string(),
        LayoutWarningKind::ForcedPageBreak => "an element larger than one page was forced onto its own page".to_string(),
        LayoutWarningKind::HeaderFooterOverflow => "header/footer content taller than its reserved band".to_string(),
        LayoutWarningKind::MissingGlyph { ch, font } => format!("missing glyph {ch:?} in font {:?}", font.0),
        LayoutWarningKind::TableRowOverflow => "table row taller than the available space".to_string(),
    };
    format!("page {}: {kind} ({})", warning.page, warning.element_hint)
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Render {
            input,
            data,
            output,
            allow_missing,
        } => {
            let doc = load_document(&input, data.as_ref(), allow_missing)?;
            let (bytes, warnings) = doc.render_with_diagnostics().map_err(|e| CliError::Render(e.to_string()))?;
            for warning in &warnings {
                eprintln!("warning: {}", describe_warning(warning));
            }
            std::fs::write(&output, &bytes).map_err(|e| CliError::Render(format!("writing {}: {e}", output.display())))?;
            eprintln!("wrote {} ({} bytes)", output.display(), bytes.len());
            Ok(())
        }
        Command::Validate {
            input,
            data,
            allow_missing,
        } => {
            load_document(&input, data.as_ref(), allow_missing)?;
            eprintln!("ok: {} is valid", input.display());
            Ok(())
        }
        Command::Fonts => {
            let fonts = FontRegistry::with_defaults().map_err(|e| CliError::Render(e.to_string()))?;
            for (key, _) in fonts.font_entries() {
                println!("{}", key.0);
            }
            Ok(())
        }
        Command::Schema => {
            let schema = schemars::schema_for!(DocumentSchema);
            let json = serde_json::to_string_pretty(&schema).expect("a generated JSON Schema always serializes");
            println!("{json}");
            Ok(())
        }
    }
}
