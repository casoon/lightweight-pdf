//! Example: a concept/strategy document — cover page (placeholder logo
//! image, title, colored "CONCEPT · <doc-id>" label, metadata rows, tag
//! pills), a table-of-contents page, and content pages with numbered
//! headings and a colored callout box (the "key takeaway" pattern used
//! throughout the source system's templates for concepts/reports/
//! documentation).
//!
//! All names, figures and text below are fictional demo data — this file
//! exists purely to demonstrate layout, not to reproduce any real
//! document.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_concept`

use lightweight_pdf::*;

/// Dummy logo: white "LOGO" lettering on a silver-gray rectangle, baseline
/// JPEG (560x160px, 3.5:1) so it embeds without the optional `png` feature.
const LOGO_JPEG: &[u8] = include_bytes!("assets/logo.jpg");

const ACCENT: Color = Color(0xE0, 0x50, 0x40);
const GRAY_TEXT: Color = Color(0x88, 0x88, 0x88);
const PILL_BG: Color = Color(0xFB, 0xE4, 0xE1);

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
        .background(PILL_BG)
        .into()
}

fn toc_entry(title: &str, page: u32) -> Element {
    Row::new()
        .child(Text::new(title).flex(1.0))
        .child(Text::new(page.to_string()))
        .into()
}

fn main() {
    let doc_id = "SMP-CON-DEMO-0001";
    let title = "Smart Home Control: Concept";

    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(20.0 * 72.0 / 25.4))
        .header(Header::new(20.0, move |_ctx| {
            Row::new()
                .child(Text::new(doc_id).size(8.0).flex(1.0))
                .child(Text::new(title).size(8.0).flex(1.0).align(Align::Center))
                .child(Text::new("Version 1.0").size(8.0).flex(1.0).align(Align::End))
                .into()
        }))
        .header_visible_from(2)
        .footer(Footer::new(24.0, |ctx| {
            Column::new()
                .gap(4.0)
                .child(Line::new())
                .child(
                    Row::new()
                        .child(Text::new("Sample Studio Ltd. (internal)").size(8.0).flex(1.0))
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
            .child(Text::new("CONCEPT").bold().color(ACCENT))
            .child(Text::new(format!("\u{b7} {doc_id}")).color(GRAY_TEXT)),
    );
    doc.add(Spacer::new(24.0));
    doc.add(Column::new().gap(4.0).width(320.0).children(vec![
        meta_row("Project", Text::new("Sample Residence (Demo)").into()),
        meta_row("Client", Text::new("Sample Studio Ltd. (internal)").into()),
        meta_row("Version", Text::new("1.0").into()),
        meta_row("Status", Text::new("DRAFT").bold().color(ACCENT).into()),
        meta_row("Created", Text::new("02/02/2026").into()),
    ]));
    doc.add(Spacer::new(200.0));
    doc.add(
        Row::new()
            .gap(8.0)
            .child(tag_pill("SMART HOME"))
            .child(tag_pill("CONCEPT"))
            .child(tag_pill("DEMO")),
    );
    doc.add(Element::PageBreak);

    // --- table of contents -------------------------------------------------
    doc.add(Text::new("Table of Contents").heading2().color(ACCENT));
    doc.add(Spacer::new(10.0));
    doc.add(Column::new().gap(4.0).children(vec![
        toc_entry("1  Background", 3),
        toc_entry("2  Objective", 3),
        toc_entry("3  Architecture", 3),
        toc_entry("3.1  Control Layer", 3),
        toc_entry("3.2  Device Layer", 4),
        toc_entry("4  Implementation Plan", 4),
    ]));
    doc.add(Element::PageBreak);

    // --- content -----------------------------------------------------------
    doc.add(Text::new("1 Background").heading2().color(ACCENT));
    doc.add(Text::new(
        "The sample residence currently runs separate point solutions for lighting, \
         heating and access control that don't talk to each other. Each component is \
         controlled through its own app (demo text).",
    ));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("2 Objective").heading2().color(ACCENT));
    doc.add(Text::new(
        "A central, locally hosted control layer should make every device addressable \
         through one unified protocol \u{2014} no cloud dependency, and it must keep \
         working offline (demo text).",
    ));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("3 Architecture").heading2().color(ACCENT));
    doc.add(Spacer::new(6.0));
    doc.add(Text::new("3.1 Control Layer").heading3());
    doc.add(Text::new(
        "A local hub owns automation rules and state management; devices are reached \
         through an open, vendor-neutral protocol (demo text).",
    ));
    doc.add(Spacer::new(10.0));
    doc.add(
        Column::new()
            .padding(12.0)
            .border(Border::solid(1.5, ACCENT))
            .background(PILL_BG)
            .child(Text::new("Key Takeaway").bold())
            .child(Spacer::new(4.0))
            .child(Text::new(
                "Without a local control layer, every extension stays an isolated \
                 solution. No expansion before the protocol is settled.",
            )),
    );
    doc.add(Spacer::new(12.0));
    doc.add(Text::new("3.2 Device Layer").heading3());
    doc.add(Text::new(
        "Existing devices are brought in gradually through bridge modules; new \
         purchases follow the chosen protocol standard exclusively (demo text).",
    ));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("4 Implementation Plan").heading2().color(ACCENT));
    doc.add(Spacer::new(6.0));
    doc.add(
        List::new()
            .numbered(Text::new("Hub selection and test setup (demo)"))
            .numbered(Text::new("Migrate lighting control (demo)"))
            .numbered(Text::new("Migrate heating and access control (demo)"))
            .numbered(Text::new("Define and test automation rules (demo)")),
    );

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_concept.pdf", &bytes).expect("write examples/demo_concept.pdf");
    println!("wrote examples/demo_concept.pdf ({} bytes)", bytes.len());
}
