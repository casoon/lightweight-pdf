//! Example: an invoice — sender/recipient block (DIN-5008-style window-
//! envelope layout), an "Inquiries to" contact box paired with invoice
//! metadata, a position table with an indented detail line under each
//! item, a right-aligned Subtotal/VAT/Total summary, and a four-column
//! footer (company / contact / owner+VAT-ID / bank details).
//!
//! All names, addresses, amounts and bank data below are fictional demo
//! data — this file exists purely to demonstrate layout, not to reproduce
//! any real document.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_invoice`

use lightweight_pdf::*;

/// PDF points per millimeter (72pt / 25.4mm).
const MM: f32 = 72.0 / 25.4;

struct LineItem {
    description: &'static str,
    detail: &'static str,
    qty: u32,
    unit: &'static str,
    vat_percent: u32,
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
            description: "Content Onboarding",
            detail: "Creation of sample pages from supplied content (demo data)",
            qty: 5,
            unit: "hrs",
            vat_percent: 19,
            unit_price_cents: 5_000,
        },
        LineItem {
            description: "Maintenance & Support",
            detail: "Updates and security patches / on-site visits (demo data)",
            qty: 4,
            unit: "hrs",
            vat_percent: 19,
            unit_price_cents: 5_000,
        },
    ];

    let net_total: i64 = items.iter().map(|i| i.qty as i64 * i.unit_price_cents).sum();
    let vat_total: i64 = items
        .iter()
        .map(|i| i.qty as i64 * i.unit_price_cents * i.vat_percent as i64 / 100)
        .sum();
    let gross_total = net_total + vat_total;

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

    // --- letterhead: wordmark, right-aligned ---------------------------
    doc.add(
        Column::new()
            .align(Align::End)
            .child(Text::new("SAMPLE STUDIO").bold().size(18.0).color(Color::rgb(0x33, 0x33, 0x33))),
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
                    .child(Text::new("Sample Trading Ltd."))
                    .child(Text::new("Sample Trading Ltd."))
                    .child(Text::new("42 Example Road"))
                    .child(Text::new("54321 Exampleville")),
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
                            .child(Text::new("Invoice No.:").bold().flex(1.0))
                            .child(Text::new("INV-DEMO-0001")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Customer No.:").bold().flex(1.0))
                            .child(Text::new("C-0001")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Invoice Date:").bold().flex(1.0))
                            .child(Text::new("02/02/2026")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Service Period:").bold().flex(1.0))
                            .child(Text::new("Jan 2026")),
                    )
                    .child(
                        Row::new()
                            .child(Text::new("Due Date:").bold().flex(1.0))
                            .child(Text::new("02/16/2026")),
                    ),
            ),
    );
    doc.add(Spacer::new(14.0 * MM));

    doc.add(Text::new("Invoice").heading3());
    doc.add(Spacer::new(6.0));
    doc.add(Text::new("Project: SMP-001 Website Maintenance (Demo)"));
    doc.add(Spacer::new(10.0));

    doc.add(
        Table::new()
            .columns([
                TableColumn::fixed(32.0),
                TableColumn::flex(1.0),
                TableColumn::fixed(45.0).align(Align::End),
                TableColumn::fixed(35.0),
                TableColumn::fixed(40.0).align(Align::End),
                TableColumn::fixed(85.0).align(Align::End),
                TableColumn::fixed(65.0).align(Align::End),
            ])
            .header(["No.", "Description", "Qty", "Unit", "VAT", "Unit Price", "Total"])
            .rows(items.iter().enumerate().map(|(i, item)| {
                let total = item.qty as i64 * item.unit_price_cents;
                vec![
                    Element::from(Text::new((i + 1).to_string())),
                    Element::from(
                        Column::new()
                            .gap(2.0)
                            .child(Text::new(item.description))
                            .child(Text::new(item.detail).size(8.0).color(Color::rgb(0x66, 0x66, 0x66))),
                    ),
                    Element::from(Text::new(item.qty.to_string()).align(Align::End)),
                    Element::from(Text::new(item.unit)),
                    Element::from(Text::new(format!("{}%", item.vat_percent)).align(Align::End)),
                    Element::from(Text::new(amount(item.unit_price_cents)).align(Align::End)),
                    Element::from(Text::new(amount(total)).bold().align(Align::End)),
                ]
            })),
    );

    doc.add(Spacer::new(14.0));

    // --- summary block (Subtotal/VAT/Total), right-aligned -------------
    doc.add(
        Column::new()
            .align(Align::End)
            .child(Column::new().gap(2.0).width(200.0).children(vec![
                Element::from(
                    Row::new()
                        .child(Text::new("Subtotal:").flex(1.0))
                        .child(Text::new(format!("EUR {}", amount(net_total)))),
                ),
                Element::from(
                    Row::new()
                        .child(Text::new("VAT (19%):").flex(1.0))
                        .child(Text::new(format!("EUR {}", amount(vat_total)))),
                ),
                Element::from(Line::new()),
                Element::from(
                    Row::new()
                        .child(Text::new("Total:").bold().flex(1.0))
                        .child(Text::new(format!("EUR {}", amount(gross_total))).bold()),
                ),
            ])),
    );

    doc.add(Spacer::new(20.0));
    doc.add(Text::new("Payable without deduction by 02/16/2026."));
    doc.add(Spacer::new(10.0));
    doc.add(Text::new("Delivered goods remain our property until paid in full."));
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
    std::fs::write("examples/demo_invoice.pdf", &bytes).expect("write examples/demo_invoice.pdf");
    println!("wrote examples/demo_invoice.pdf ({} bytes)", bytes.len());
}
