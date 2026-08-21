//! Example: a website-audit-style report — same cover/ToC/header/footer
//! pattern as `demo_concept`/`demo_documentation`, with a scorecard table
//! and "top issues" subsections (the WebCheck report structure from the
//! source system: result first, then the handful of issues that matter
//! most, impact before technical cause).
//!
//! All names, scores and findings below are fictional demo data — this
//! file exists purely to demonstrate layout, not to reproduce any real
//! document.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_report`

use lightweight_pdf::*;

/// Dummy logo: white "LOGO" lettering on a silver-gray rectangle, baseline
/// JPEG (560x160px, 3.5:1) so it embeds without the optional `png` feature.
const LOGO_JPEG: &[u8] = include_bytes!("assets/logo.jpg");

const ACCENT: Color = Color(0xE0, 0x50, 0x40);
const GRAY_TEXT: Color = Color(0x88, 0x88, 0x88);

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

struct Issue {
    title: &'static str,
    affected: &'static str,
    impact: &'static str,
    cause: &'static str,
}

fn issue_section(n: usize, issue: &Issue) -> Vec<Element> {
    vec![
        Text::new(format!("1.2.{n} {}", issue.title)).heading3().into(),
        Text::new(issue.affected).color(GRAY_TEXT).into(),
        Text::new(issue.impact).into(),
        Text::new(issue.cause).into(),
        Spacer::new(10.0).into(),
    ]
}

fn main() {
    let doc_id = "SMP-RPT-DEMO-0001";
    let title = "Website Analysis: sample-shop.example";

    let scorecard = [
        ("Accessibility", "58 of 100", "Significant action needed"),
        ("Performance", "74 of 100", "Solid baseline"),
        ("SEO", "81 of 100", "Professional"),
        ("Security", "69 of 100", "Gaps in headers"),
        ("Mobile", "77 of 100", "Good"),
    ];

    let issues = [
        Issue {
            title: "1. Some users can't perceive content",
            affected: "42 affected elements (demo)",
            impact: "The structure isn't parseable for screen readers (demo text).",
            cause: "Tables and lists are styled visually but not marked up as such in the source code (demo text).",
        },
        Issue {
            title: "2. Text is hard to read under real-world conditions",
            affected: "9 affected elements (demo)",
            impact: "Light text on a light background makes reading in sunlight difficult (demo text).",
            cause: "Contrast ratio as low as 2.1:1 in places \u{2014} the minimum is 4.5:1 (demo text).",
        },
    ];

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
                        .child(Text::new("Sample Shop Ltd.").size(8.0).flex(1.0))
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
            .child(Text::new("REPORT").bold().color(ACCENT))
            .child(Text::new(format!("\u{b7} {doc_id}")).color(GRAY_TEXT)),
    );
    doc.add(Spacer::new(24.0));
    doc.add(Column::new().gap(4.0).width(320.0).children(vec![
        meta_row("Project", Text::new("WebCheck (Demo)").into()),
        meta_row("Client", Text::new("Sample Shop Ltd.").into()),
        meta_row("Version", Text::new("1.0").into()),
        meta_row("Status", Text::new("COMPLETED").bold().color(ACCENT).into()),
        meta_row("Created", Text::new("02/02/2026").into()),
    ]));
    doc.add(Spacer::new(200.0));
    doc.add(
        Row::new()
            .gap(8.0)
            .child(tag_pill("WEBSITE ANALYSIS"))
            .child(tag_pill("ACCESSIBILITY"))
            .child(tag_pill("PERFORMANCE")),
    );
    doc.add(Element::PageBreak);

    // --- table of contents -------------------------------------------------
    doc.add(Text::new("Table of Contents").heading2().color(ACCENT));
    doc.add(Spacer::new(10.0));
    doc.add(Column::new().gap(4.0).children(vec![
        toc_entry("1  Result", 3),
        toc_entry("1.1  Scorecard", 3),
        toc_entry("1.2  Top Issues", 3),
        toc_entry("2  Quick Wins", 4),
    ]));
    doc.add(Element::PageBreak);

    // --- content -----------------------------------------------------------
    doc.add(Text::new("1 Result").heading2().color(ACCENT));
    doc.add(Text::new(
        "The website works in principle \u{2014} but loses impact in several places (demo text).",
    ));
    doc.add(Spacer::new(10.0));

    doc.add(Text::new("1.1 Scorecard").heading3());
    doc.add(Spacer::new(6.0));
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(90.0), TableColumn::fixed(180.0)])
            .header(["Area", "Score", "Assessment"])
            .striped(Color::rgb(0xF5, 0xF5, 0xF5))
            .rows(scorecard.iter().map(|(area, score, note)| vec![*area, *score, *note])),
    );
    doc.add(Spacer::new(14.0));

    doc.add(Text::new("1.2 Top Issues").heading3());
    doc.add(Text::new(
        "The following points currently have the biggest impact on how the website functions (demo text).",
    ));
    doc.add(Spacer::new(10.0));
    for (i, issue) in issues.iter().enumerate() {
        for el in issue_section(i + 1, issue) {
            doc.add(el);
        }
    }

    doc.add(Text::new("2 Quick Wins").heading2().color(ACCENT));
    doc.add(Spacer::new(6.0));
    doc.add(
        List::new()
            .bullet(Text::new("Raise contrast ratios to 4.5:1 (effort: low, demo)"))
            .bullet(Text::new("Add ARIA labels to interactive elements (effort: medium, demo)"))
            .bullet(Text::new("Place a visible call-to-action on the homepage (effort: low, demo)")),
    );

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_report.pdf", &bytes).expect("write examples/demo_report.pdf");
    println!("wrote examples/demo_report.pdf ({} bytes)", bytes.len());
}
