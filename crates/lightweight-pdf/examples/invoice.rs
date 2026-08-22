//! Example: a German business invoice — DIN-5008-style window-envelope
//! address block, a striped multi-page position table (header repeats
//! automatically across pages), a right-aligned Netto/USt/Brutto summary
//! block, and a footer with bank details/VAT ID + page numbers.
//! (Phase 6 DoD, `plan/phases/phase-6-business-polish.md`.)
//!
//! Run: `cargo run -p lightweight-pdf --example invoice`

use lightweight_pdf::*;

#[path = "common/mod.rs"]
mod common;

/// PDF points per millimeter (72pt / 25.4mm).
const MM: f32 = 72.0 / 25.4;

struct LineItem {
    description: String,
    qty: u32,
    unit_price_cents: i64,
}

fn main() {
    let mut items = vec![
        LineItem {
            description: "Beratungsleistung Projekt Alpha".to_string(),
            qty: 8,
            unit_price_cents: 12_000,
        },
        LineItem {
            description: "Lizenz Software-Paket (jährlich)".to_string(),
            qty: 1,
            unit_price_cents: 49_900,
        },
        LineItem {
            description: "Individuelle Anpassung / Customizing".to_string(),
            qty: 3,
            unit_price_cents: 15_000,
        },
    ];
    // A few filler positions so the table is guaranteed to span more than
    // one page, demonstrating the header-repeat-on-split behavior.
    for i in 1..=25 {
        items.push(LineItem {
            description: format!("Zusatzposition {i:02}"),
            qty: 1,
            unit_price_cents: 990,
        });
    }

    let net_total: i64 = items.iter().map(|i| i.qty as i64 * i.unit_price_cents).sum();
    let vat_rate = 19;
    let vat_total = net_total * vat_rate / 100;
    let gross_total = net_total + vat_total;

    let top_margin = 15.0 * MM;
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(20.0 * MM, top_margin))
        .footer(Footer::new(30.0, |ctx| {
            Column::new()
                .gap(2.0)
                .child(Line::new())
                .child(
                    Row::new()
                        .gap(20.0)
                        .child(Text::new("Musterbank · IBAN DE12 3456 7890 1234 5678 90 · BIC MUSTDEFF").size(8.0))
                        .child(Text::new("USt-IdNr. DE123456789").size(8.0).flex(1.0)),
                )
                .child(Text::new(format!("Seite {} von {}", ctx.page, ctx.total_pages)).size(8.0))
                .into()
        }));

    // --- DIN 5008 Form A window-envelope address block ----------------
    // Window starts ~45mm from the top, ~20mm from the left
    // (`plan/02-elementcatalog-and-features.md`); ~85x40mm matches a
    // typical C6/5-long window envelope opening. Position/size are a
    // documented convention for this recipe, not parsed from any norm
    // document — a caller with different envelope stock adjusts these.
    doc.add(Spacer::new(45.0 * MM - top_margin));
    doc.add(
        Column::new()
            .gap(2.0)
            .width(85.0 * MM)
            .height(40.0 * MM)
            .child(Text::new("Muster GmbH · Musterstraße 1 · 12345 Musterstadt").size(7.0))
            .child(Spacer::new(8.0))
            .child(Text::new("Empfänger GmbH"))
            .child(Text::new("Frau Erika Mustermann"))
            .child(Text::new("Beispielweg 42"))
            .child(Text::new("54321 Beispielstadt")),
    );
    doc.add(Spacer::new(10.0 * MM));

    doc.add(Text::new("Rechnung").heading1());
    doc.add(Text::new(
        "Rechnungsnummer: RE-2026-0142    Rechnungsdatum: 20.08.2026    Leistungsdatum: 20.08.2026",
    ));
    doc.add(Spacer::new(10.0));

    doc.add(
        Table::new()
            .columns([
                TableColumn::flex(1.0),
                TableColumn::fixed(50.0).align(Align::End),
                TableColumn::fixed(65.0).align(Align::End),
                TableColumn::fixed(70.0).align(Align::End),
            ])
            .header(["Beschreibung", "Menge", "Einzelpreis", "Gesamt"])
            .striped(Color::rgb(0xF5, 0xF5, 0xF5))
            .rows(items.iter().map(|item| {
                let total = item.qty as i64 * item.unit_price_cents;
                vec![
                    Element::from(item.description.as_str()),
                    Element::from(item.qty.to_string()),
                    Element::from(format_currency_de(item.unit_price_cents)),
                    Element::from(format_currency_de(total)),
                ]
            })),
    );

    doc.add(Spacer::new(14.0));

    // --- summary block (Netto/USt/Brutto), right-aligned --------------
    // Recipe (`plan/02-elementcatalog-and-features.md`): an outer, full-
    // width auto Column with `.align(Align::End)` positions the fixed-
    // width (200pt) inner summary Column at the right edge; within it,
    // each label gets `.flex(1.0)` to push its value to that block's own
    // right edge. No special "summary block" element needed.
    doc.add(
        Column::new()
            .align(Align::End)
            .child(Column::new().gap(2.0).width(200.0).children(vec![
                Element::from(
                    Row::new()
                        .child(Text::new("Nettosumme").flex(1.0))
                        .child(Text::new(format_currency_de(net_total))),
                ),
                Element::from(
                    Row::new()
                        .child(Text::new(format!("zzgl. {vat_rate}% USt.")).flex(1.0))
                        .child(Text::new(format_currency_de(vat_total))),
                ),
                Element::from(Line::new()),
                Element::from(
                    Row::new()
                        .child(Text::new("Gesamtbetrag").bold().flex(1.0))
                        .child(Text::new(format_currency_de(gross_total)).bold()),
                ),
            ])),
    );

    common::write_pdf(&doc, "invoice.pdf");
}
