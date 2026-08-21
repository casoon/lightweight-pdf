//! Example: API/technical documentation — same cover/ToC/header/footer
//! pattern as `demo_concept` (this source system reuses one template for
//! concepts, reports and documentation, only the cover label text
//! differs), with endpoint sections, a monospace-styled request/response
//! block, and an error-code table.
//!
//! All names, endpoints and figures below are fictional demo data — this
//! file exists purely to demonstrate layout, not to reproduce any real
//! document.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_documentation`

use lightweight_pdf::*;

/// Dummy logo: white "LOGO" lettering on a silver-gray rectangle, baseline
/// JPEG (560x160px, 3.5:1) so it embeds without the optional `png` feature.
const LOGO_JPEG: &[u8] = include_bytes!("assets/logo.jpg");

const ACCENT: Color = Color(0xE0, 0x50, 0x40);
const GRAY_TEXT: Color = Color(0x88, 0x88, 0x88);
const CODE_BG: Color = Color(0xF5, 0xF5, 0xF5);

fn meta_row(label: &str, mut value: Element) -> Element {
    if let Some(common) = value.common_mut() {
        common.flex = Some(1.0);
        common.overflow = Overflow::Ellipsis;
    }
    Row::new().child(Text::new(label).color(GRAY_TEXT).width(90.0)).child(value).into()
}

fn tag_pill(label: &str) -> Element {
    Text::new(label)
        .bold()
        .size(8.0)
        .color(ACCENT)
        .padding(6.0)
        .background(Color(0xFB, 0xE4, 0xE1))
        .into()
}

fn toc_entry(title: &str, page: u32) -> Element {
    Row::new()
        .child(Text::new(title).flex(1.0))
        .child(Text::new(page.to_string()))
        .into()
}

/// A `code`-block-alike: monospace-adjacent styling isn't available (no
/// bundled monospace font, see README's `default-fonts` note) — a shaded
/// box with `Text` stands in for it, matching this document's own
/// convention for inline technical snippets.
fn code_block(lines: &[&str]) -> Element {
    Column::new()
        .gap(2.0)
        .padding(8.0)
        .background(CODE_BG)
        .children(lines.iter().map(|l| Text::new(*l).size(9.0)))
        .into()
}

fn main() {
    let doc_id = "SMP-DOC-DEMO-0001";
    let title = "Sample API Documentation";

    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(20.0 * 72.0 / 25.4))
        .header(Header::new(20.0, move |_ctx| {
            Row::new()
                .child(Text::new(doc_id).size(8.0).flex(1.0))
                .child(Text::new(title).size(8.0).flex(1.0).align(Align::Center))
                .child(Text::new("Version 2.0").size(8.0).flex(1.0).align(Align::End))
                .into()
        }))
        .header_visible_from(2)
        .footer(Footer::new(24.0, |ctx| {
            Column::new()
                .gap(4.0)
                .child(Line::new())
                .child(
                    Row::new()
                        .child(Text::new("Sample Studio Ltd.").size(8.0).flex(1.0))
                        .child(
                            Text::new(format!("Page {} of {}", ctx.page, ctx.total_pages))
                                .size(8.0)
                                .flex(1.0)
                                .align(Align::Center),
                        )
                        .child(Text::new("02/02/2026").size(8.0).align(Align::End).flex(1.0)),
                )
                .into()
        }));

    // --- cover page ------------------------------------------------------
    doc.add(Spacer::new(60.0));
    doc.add(
        Column::new()
            .align(Align::Center)
            .child(Image::new(LOGO_JPEG).expect("valid demo logo JPEG").width(160.0).height(45.7)),
    );
    doc.add(Spacer::new(70.0));
    doc.add(Text::new(title).heading1());
    doc.add(Spacer::new(20.0));
    doc.add(
        Row::new()
            .gap(4.0)
            .child(Text::new("DOCUMENTATION").bold().color(ACCENT))
            .child(Text::new(format!("\u{b7} {doc_id}")).color(GRAY_TEXT)),
    );
    doc.add(Spacer::new(24.0));
    doc.add(Column::new().gap(4.0).width(320.0).children(vec![
        meta_row("Project", Text::new("Sample Platform API (Demo)").into()),
        meta_row("Client", Text::new("Sample Studio Ltd.").into()),
        meta_row("Version", Text::new("2.0").into()),
        meta_row("Status", Text::new("FINAL").bold().color(ACCENT).into()),
        meta_row("Created", Text::new("02/02/2026").into()),
    ]));
    doc.add(Spacer::new(200.0));
    doc.add(
        Row::new()
            .gap(8.0)
            .child(tag_pill("API"))
            .child(tag_pill("REST"))
            .child(tag_pill("DEMO")),
    );
    doc.add(Element::PageBreak);

    // --- table of contents -------------------------------------------------
    doc.add(Text::new("Table of Contents").heading2().color(ACCENT));
    doc.add(Spacer::new(10.0));
    doc.add(Column::new().gap(4.0).children(vec![
        toc_entry("1  Overview", 3),
        toc_entry("2  Authentication", 3),
        toc_entry("3  Endpoints", 3),
        toc_entry("3.1  GET /v2/items", 3),
        toc_entry("4  Error Handling", 4),
    ]));
    doc.add(Element::PageBreak);

    // --- content -----------------------------------------------------------
    doc.add(Text::new("1 Overview").heading2().color(ACCENT));
    doc.add(Text::new(
        "The Sample API exposes resources of the Sample Platform as a REST interface. \
         All responses are JSON-encoded (demo text).",
    ));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("2 Authentication").heading2().color(ACCENT));
    doc.add(Text::new(
        "Every request needs a bearer token in the Authorization header (demo text):",
    ));
    doc.add(Spacer::new(6.0));
    doc.add(code_block(&["Authorization: Bearer REDACTED-DEMO-PLACEHOLDER"]));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("3 Endpoints").heading2().color(ACCENT));
    doc.add(Spacer::new(6.0));
    doc.add(Text::new("3.1 GET /v2/items").heading3());
    doc.add(Text::new("Returns a list of sample objects (demo text)."));
    doc.add(Spacer::new(6.0));
    doc.add(code_block(&[
        "GET /v2/items?limit=20 HTTP/1.1",
        "Host: api-demo.sample-design.example",
        "",
        "{ \"items\": [ { \"id\": \"itm_001\", \"name\": \"Sample\" } ], \"total\": 1 }",
    ]));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("4 Error Handling").heading2().color(ACCENT));
    doc.add(Spacer::new(6.0));
    doc.add(
        Table::new()
            .columns([TableColumn::fixed(60.0), TableColumn::fixed(140.0), TableColumn::flex(1.0)])
            .header(["Code", "Meaning", "Description"])
            .rows(vec![
                vec!["400", "Bad Request", "Invalid or missing parameters"],
                vec!["401", "Unauthorized", "Token missing or invalid"],
                vec!["404", "Not Found", "Resource does not exist"],
                vec!["429", "Too Many Requests", "Rate limit exceeded"],
            ]),
    );

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_documentation.pdf", &bytes).expect("write examples/demo_documentation.pdf");
    println!("wrote examples/demo_documentation.pdf ({} bytes)", bytes.len());
}
