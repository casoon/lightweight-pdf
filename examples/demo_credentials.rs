//! Example: a confidential credentials hand-off document — a cover page
//! (placeholder logo image, metadata rows + a bordered "CONFIDENTIAL"
//! warning box, header suppressed via `.header_visible_from(2)`), then a
//! content page with a "CONFIDENTIAL · <doc-id>" header band, a labeled
//! key/value settings box, a ports table, bordered per-credential boxes,
//! and a closing security-notes box with a bulleted `List`.
//!
//! All names, hosts, IPs and "passwords" below are fictional demo data —
//! this file exists purely to demonstrate layout, not to reproduce any
//! real credentials.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_credentials`

use lightweight_pdf::*;

/// Dummy logo: white "LOGO" lettering on a silver-gray rectangle, baseline
/// JPEG (560x160px, 3.5:1) so it embeds without the optional `png` feature.
const LOGO_JPEG: &[u8] = include_bytes!("assets/logo.jpg");

const ACCENT: Color = Color(0xE0, 0x50, 0x40);
const GRAY_BG: Color = Color(0xF5, 0xF5, 0xF5);
const GRAY_TEXT: Color = Color(0x66, 0x66, 0x66);
const BORDER_GRAY: Color = Color(0xCC, 0xCC, 0xCC);

struct Credential {
    label: &'static str,
    username: &'static str,
    password: &'static str,
    kind: &'static str,
}

fn credential_box(cred: &Credential) -> Element {
    Column::new()
        .gap(4.0)
        .padding(10.0)
        .border(Border {
            width: 1.0,
            color: BORDER_GRAY,
        })
        .child(Text::new(cred.label).bold())
        .child(
            Row::new()
                .child(Text::new("Username:").bold().width(110.0))
                .child(Text::new(cred.username)),
        )
        .child(
            Row::new()
                .child(Text::new("Password:").bold().width(110.0))
                .child(Text::new(cred.password)),
        )
        .child(Row::new().child(Text::new("Type:").bold().width(110.0)).child(Text::new(cred.kind)))
        .into()
}

fn main() {
    let credentials = [
        Credential {
            label: "SSH access for server administration (demo)",
            username: "admin",
            password: "Demo!2026#Secure",
            kind: "ssh",
        },
        Credential {
            label: "Deployment user (restricted rights, demo)",
            username: "deploy",
            password: "Deploy$Key!2026",
            kind: "ssh",
        },
    ];

    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(20.0 * 72.0 / 25.4))
        .header(Header::new(20.0, |_ctx| {
            Row::new()
                .child(Text::new("CONFIDENTIAL").bold().color(ACCENT).flex(1.0))
                .child(Text::new("SMP-ACC-DEMO-0001"))
                .into()
        }))
        .header_visible_from(2)
        .footer(Footer::new(24.0, |ctx| {
            Column::new()
                .gap(4.0)
                .child(Line::new())
                .child(
                    Row::new()
                        .child(Text::new("SAMPLE STUDIO").size(8.0).flex(1.0))
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
    doc.add(Spacer::new(50.0));
    doc.add(Text::new("Demo Server Credentials").heading1().align(Align::Center));
    doc.add(Spacer::new(20.0));
    doc.add(
        Row::new()
            .gap(4.0)
            .align(Align::Center)
            .child(Text::new("CREDENTIALS").bold().color(ACCENT))
            .child(Text::new("\u{b7} SMP-ACC-DEMO-0001")),
    );
    doc.add(Spacer::new(30.0));
    doc.add(
        Column::new()
            .align(Align::Center)
            .child(Column::new().gap(4.0).width(260.0).children(vec![
                Element::from(
                    Row::new()
                        .child(Text::new("Project").width(80.0).color(GRAY_TEXT))
                        .child(Text::new("Demo & Testing Environment")),
                ),
                Element::from(
                    Row::new()
                        .child(Text::new("Client").width(80.0).color(GRAY_TEXT))
                        .child(Text::new("Sample Studio Ltd.")),
                ),
                Element::from(
                    Row::new()
                        .child(Text::new("Date").width(80.0).color(GRAY_TEXT))
                        .child(Text::new("2026-02-02")),
                ),
            ])),
    );
    doc.add(Spacer::new(40.0));
    doc.add(
        Column::new()
            .padding(14.0)
            .border(Border { width: 1.5, color: ACCENT })
            .child(Text::new("CONFIDENTIAL").bold().color(ACCENT).align(Align::Center))
            .child(Spacer::new(6.0))
            .child(
                Text::new(
                    "This document contains confidential credentials. Please store it \
                     securely and do not share it with unauthorized third parties.",
                )
                .align(Align::Center),
            ),
    );
    doc.add(Element::PageBreak);

    // --- content page ------------------------------------------------------
    doc.add(Text::new("Demo Web Server").heading2().color(ACCENT));
    doc.add(Text::new("Nginx web server for the demo environment (demo data)"));
    doc.add(Spacer::new(4.0));
    doc.add(
        Row::new()
            .gap(4.0)
            .child(Text::new("URL:").bold())
            .child(Text::new("https://demo.sample-design.example")),
    );
    doc.add(Spacer::new(14.0));

    doc.add(Text::new("Technical Settings").heading3());
    doc.add(Spacer::new(6.0));
    doc.add(Column::new().gap(2.0).padding(10.0).background(GRAY_BG).children(vec![
        Element::from(
            Row::new()
                .child(Text::new("IP Address:").bold().width(110.0))
                .child(Text::new("192.168.1.100")),
        ),
        Element::from(Row::new().child(Text::new("SSH Port:").bold().width(110.0)).child(Text::new("22"))),
        Element::from(Row::new().child(Text::new("OS:").bold().width(110.0)).child(Text::new("Ubuntu 24.04 LTS"))),
    ]));
    doc.add(Spacer::new(14.0));

    doc.add(Text::new("Ports & Protocols").heading3());
    doc.add(Spacer::new(6.0));
    doc.add(
        Table::new()
            .columns([TableColumn::fixed(60.0), TableColumn::fixed(80.0), TableColumn::flex(1.0)])
            .header(["Port", "Protocol", "Description"])
            .rows(vec![
                vec!["80", "TCP", "HTTP (redirect to HTTPS)"],
                vec!["443", "TCP", "HTTPS"],
                vec!["22", "TCP", "SSH"],
            ]),
    );
    doc.add(Spacer::new(14.0));

    doc.add(Text::new("Credentials").heading3());
    doc.add(Spacer::new(6.0));
    doc.add(Column::new().gap(10.0).children(credentials.iter().map(credential_box)));
    doc.add(Spacer::new(20.0));

    doc.add(
        Column::new()
            .gap(4.0)
            .padding(12.0)
            .border(Border {
                width: 1.0,
                color: Color::rgb(0xE6, 0x9A, 0x2E),
            })
            .child(Text::new("Security Notes").bold().align(Align::Center))
            .child(Spacer::new(4.0))
            .child(
                List::new()
                    .bullet(Text::new("Never send passwords by email or unencrypted."))
                    .bullet(Text::new("Store credentials securely in a password manager."))
                    .bullet(Text::new("Rotate passwords regularly (at least every 90 days).")),
            ),
    );

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_credentials.pdf", &bytes).expect("write examples/demo_credentials.pdf");
    println!("wrote examples/demo_credentials.pdf ({} bytes)", bytes.len());
}
