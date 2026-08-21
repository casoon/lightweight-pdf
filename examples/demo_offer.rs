//! Example: a quote/offer — the same letterhead/contact-box layout as
//! `demo_invoice`, but with lump-sum positions (no per-line VAT column,
//! VAT-inclusive unit prices), bullet-separated scope details under each
//! item, and labeled payment/delivery terms paragraphs instead of a tax
//! summary block.
//!
//! All names, addresses, amounts and bank data below are fictional demo
//! data — this file exists purely to demonstrate layout, not to reproduce
//! any real document.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_offer`

use lightweight_pdf::*;

/// PDF points per millimeter (72pt / 25.4mm).
const MM: f32 = 72.0 / 25.4;

/// Dummy logo: white "LOGO" lettering on a silver-gray rectangle, baseline
/// JPEG (560x160px, 3.5:1) so it embeds without the optional `png` feature.
const LOGO_JPEG: &[u8] = include_bytes!("assets/logo.jpg");

struct LineItem {
    title: &'static str,
    description: &'static str,
    scope: &'static [&'static str],
    qty: u32,
    unit: &'static str,
    unit_price_cents: i64,
}

/// English-style amount formatting without a currency symbol, e.g.
/// `1,234.56` — the library only ships `format_currency_de` (German comma-
/// decimal), so an English demo needs its own small formatter.
fn amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let whole = abs / 100;
    let frac = abs % 100;
    format!("{sign}{}.{frac:02}", group_thousands(whole))
}

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn main() {
    let items = [
        LineItem {
            title: "Website Development",
            description: "Development of a modern, responsive website (demo data)",
            scope: &[
                "Design and concept",
                "Frontend development (HTML, CSS, JavaScript)",
                "Responsive design for all devices",
            ],
            qty: 1,
            unit: "lump",
            unit_price_cents: 500_000,
        },
        LineItem {
            title: "Content Management System",
            description: "CMS setup and configuration (demo data)",
            scope: &["Installation", "Theme customization", "Editor training"],
            qty: 1,
            unit: "lump",
            unit_price_cents: 200_000,
        },
    ];

    let total: i64 = items.iter().map(|i| i.qty as i64 * i.unit_price_cents).sum();

    let top_margin = 20.0 * MM;
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(20.0 * MM, top_margin))
        .footer(Footer::new(62.0, |_ctx| {
            Column::new()
                .gap(4.0)
                .child(Line::new())
                .child(
                    Row::new()
                        .gap(16.0)
                        .child(
                            Column::new()
                                .gap(1.0)
                                .flex(1.0)
                                .child(Text::new("Sample Design Studio").bold().size(8.0))
                                .child(Text::new("John Doe").size(8.0))
                                .child(Text::new("1 Sample Street").size(8.0))
                                .child(Text::new("12345 Sampletown").size(8.0)),
                        )
                        .child(
                            Column::new()
                                .gap(1.0)
                                .flex(1.0)
                                .child(Text::new("Phone: +1 555 0100").size(8.0))
                                .child(Text::new("Email: hello@sample-design.example").size(8.0))
                                .child(Text::new("Web: www.sample-design.example").size(8.0)),
                        )
                        .child(
                            Column::new()
                                .gap(1.0)
                                .flex(1.0)
                                .child(Text::new("Owner:").size(8.0))
                                .child(Text::new("John Doe").size(8.0))
                                .child(Text::new("VAT ID:").size(8.0))
                                .child(Text::new("EU123456789").size(8.0)),
                        )
                        .child(
                            Column::new()
                                .gap(1.0)
                                .flex(1.0)
                                .child(Text::new("Bank: Sample Bank").size(8.0))
                                .child(Text::new("Account Holder: John Doe").size(8.0))
                                .child(Text::new("IBAN: DE12 3456 7890 1234 5678 90").size(8.0))
                                .child(Text::new("BIC/SWIFT: SMPLUS33").size(8.0)),
                        ),
                )
                .into()
        }));

    // --- letterhead: logo, right-aligned --------------------------------
    doc.add(
        Column::new()
            .align(Align::End)
            .child(Image::new(LOGO_JPEG).expect("valid demo logo JPEG").width(120.0).height(34.3)),
    );
    doc.add(Spacer::new(10.0 * MM));

    doc.add(
        Row::new()
            .gap(20.0)
            .child(
                Column::new()
                    .gap(2.0)
                    .flex(1.0)
                    .child(Text::new("John Doe \u{b7} 1 Sample Street \u{b7} 12345 Sampletown").size(7.0))
                    .child(Spacer::new(8.0))
                    .child(Text::new("Prospect Inc."))
                    .child(Text::new("Jane Roe"))
                    .child(Text::new("7 Prospect Avenue"))
                    .child(Text::new("98765 Prospectville")),
            )
            .child(
                Column::new()
                    .gap(2.0)
                    .width(190.0)
                    .child(Text::new("Inquiries to:").bold())
                    .child(Text::new("Sample Design Studio"))
                    .child(Text::new("+1 555 0100"))
                    .child(Text::new("hello@sample-design.example"))
                    .child(Spacer::new(8.0))
                    .child(
                        Row::new()
                            .child(Text::new("Quote No.:").bold().flex(1.0))
                            .child(Text::new("QUO-DEMO-0001")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Customer No.:").bold().flex(1.0))
                            .child(Text::new("C-0002")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Quote Date:").bold().flex(1.0))
                            .child(Text::new("02/02/2026")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Valid Until:").bold().flex(1.0))
                            .child(Text::new("03/04/2026")),
                    ),
            ),
    );
    doc.add(Spacer::new(14.0 * MM));

    doc.add(Text::new("Quote").heading3());
    doc.add(Spacer::new(10.0));

    doc.add(
        Table::new()
            .columns([
                TableColumn::fixed(32.0),
                TableColumn::flex(1.0),
                TableColumn::fixed(45.0).align(Align::End),
                TableColumn::fixed(40.0),
                TableColumn::fixed(80.0).align(Align::End),
                TableColumn::fixed(80.0).align(Align::End),
            ])
            .header(["No.", "Description", "Qty", "Unit", "Unit Price", "Total"])
            .rows(items.iter().enumerate().map(|(i, item)| {
                let line_total = item.qty as i64 * item.unit_price_cents;
                vec![
                    Element::from(Text::new((i + 1).to_string())),
                    Element::from(
                        Column::new()
                            .gap(2.0)
                            .child(Text::new(item.title).bold())
                            .child(Text::new(item.description).size(9.0))
                            .child(
                                Text::new(item.scope.join(" \u{2022} "))
                                    .size(8.0)
                                    .color(Color::rgb(0x66, 0x66, 0x66)),
                            ),
                    ),
                    Element::from(Text::new(item.qty.to_string()).align(Align::End)),
                    Element::from(Text::new(item.unit)),
                    Element::from(Text::new(format!("EUR {}", amount(item.unit_price_cents))).align(Align::End)),
                    Element::from(Text::new(format!("EUR {}", amount(line_total))).bold().align(Align::End)),
                ]
            })),
    );

    doc.add(Spacer::new(14.0));
    doc.add(
        Column::new().align(Align::End).child(
            Row::new()
                .width(200.0)
                .child(Text::new("Total (net):").bold().flex(1.0))
                .child(Text::new(format!("EUR {}", amount(total))).bold()),
        ),
    );
    doc.add(Spacer::new(14.0));
    doc.add(Text::new("This quote is valid for 30 days from the quote date."));
    doc.add(Spacer::new(10.0));
    doc.add(
        Row::new().gap(4.0).child(Text::new("Payment Terms:").bold()).child(
            Text::new(
                "50% deposit on order confirmation, 50% on project completion. Payable \
                 within 14 days of invoicing, without deduction.",
            )
            .flex(1.0),
        ),
    );
    doc.add(Spacer::new(6.0));
    doc.add(
        Row::new().gap(4.0).child(Text::new("Delivery Terms:").bold()).child(
            Text::new(
                "Completion within 8 weeks of order confirmation and receipt of all \
                 required content and approvals.",
            )
            .flex(1.0),
        ),
    );
    doc.add(Spacer::new(10.0));
    doc.add(
        Text::new("Sample document \u{2014} all names, addresses and amounts are fictional.")
            .size(9.0)
            .color(Color::rgb(0x88, 0x88, 0x88)),
    );
    doc.add(Spacer::new(20.0));
    doc.add(Text::new("Kind regards"));
    doc.add(Spacer::new(20.0));
    doc.add(Text::new("John Doe"));

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_offer.pdf", &bytes).expect("write examples/demo_offer.pdf");
    println!("wrote examples/demo_offer.pdf ({} bytes)", bytes.len());
}
