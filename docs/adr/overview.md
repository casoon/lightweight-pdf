---
title: Architecture Decisions (ADRs)
sidebarLabel: Overview
description: Index and rationale of architecture decision records in lightweight-pdf.
order: 1
---

Architecture Decision Records (ADRs) capture the key architectural choices, context, trade-offs, and consequences made throughout the evolution of `lightweight-pdf`. 

Documenting these decisions upfront helps maintain focus on core requirements, prevents regressions, clarifies architectural boundaries for contributors, and answers frequent design questions (such as why a custom subsetter was written, why HTML/CSS parsing is out of scope, or how dependency sizes are controlled).

> [!NOTE]
> The ADR texts are preserved in their original, authentic German formulation as maintained during project planning and development.

## Index of Decisions

| ID | Title | Status | Scope |
|:---|:------|:-------|:------|
| **ADR-001** | [Lizenz: MIT](../decisions/#adr-001--lizenz-mit) | Decided | Project license & SIL OFL 1.1 font licensing |
| **ADR-002** | [Workspace-Layout](../decisions/#adr-002--workspace-layout) | Decided | 4 core crates + facade crate separation |
| **ADR-003** | [Zwei begrenzte Dependency-Ausnahmen](../decisions/#adr-003--zwei-begrenzte-dependency-ausnahmen-ttf-parser-und-png) | Decided | Dependency boundaries (skrifa, png) |
| **ADR-004** | [Kein `taffy`, kein `pdf-writer`](../decisions/#adr-004--kein-taffy-kein-pdf-writer-vorbild-ja-dependency-nein) | Decided | Own PDF writer & flex-like layout engine |
| **ADR-005** | [Keine Makro-DSL, kein Parser](../decisions/#adr-005--keine-makro-dsl-kein-parser-builder-pattern-bleibt) | Decided | Pure Rust builder pattern over custom languages |
| **ADR-006** | [Default-Fonts: Source Sans 3 + Source Serif 4](../decisions/#adr-006--default-fonts-source-sans-3--source-serif-4-sil-ofl-11) | Decided | Bundled OFL fonts |
| **ADR-007** | [Überlauf-/Überlagerungs-Schutz](../decisions/#adr-007--berlauf-berlagerungs-schutz-ist-kernanforderung-ab-phase-1) | Decided | Hard robustness principles against overlapping content |
| **ADR-008** | [Größenbudget](../decisions/#adr-008--grenbudget) | Decided | Wasm binary size limits & automated tracking |
| **ADR-009** | [V1-Wasm-Grenze: Rust-first](../decisions/#adr-009--v1-wasm-grenze-rust-first-keine-implizite-js-api) | Superseded | Superseded by ADR-017 |
| **ADR-010** | [Font-Bridge liegt an der Facade](../decisions/#adr-010--font-bridge-liegt-an-der-facade) | Decided | Decoupled font metrics bridge |
| **ADR-011** | [Harte Seitenbereiche statt nachträglichem Seiten-Clip](../decisions/#adr-011--harte-seitenbereiche-statt-nachtrglichem-seiten-clip) | Decided | Page boundary clipping guarantees |
| **ADR-012** | [Schrift-Scope und PDF-Textverträglichkeit](../decisions/#adr-012--schrift-scope-und-pdf-textvertrglichkeit) | Decided | Static TrueType `glyf` outlines & CID mapping |
| **ADR-013** | [PNG über geprüften Decoder](../decisions/#adr-013--png-ber-geprften-decoder-mit-klaren-eingabegrenzen) | Decided | PNG decoding & transparency handling |
| **ADR-014** | [Phase-0-Größenmessung](../decisions/#adr-014--phase-0-grenmessung-baseline-liegt-ber-dem-zielkorridor-wasm-size-probe-koppelt-an-default-fonts) | Decided | Measurement probes & font coupling |
| **ADR-015** | [ttf-parser Migration zu `skrifa`](../decisions/#adr-015--ttf-parser-unmaintained-rustsec-2026-0192-auf-skrifa-migriert) | Decided | Security audit response (RUSTSEC-2026-0192) |
| **ADR-016** | [FlateDecode via `miniz_oxide`](../decisions/#adr-016--flatedecode-via-miniz_oxide-nicht-eigenbau-issue-2) | Decided | Stream compression without native C libraries |
| **ADR-017** | [wasm-bindgen-Bindings & npm-Paket](../decisions/#adr-017--wasm-bindgen-bindings--npm-paket-issue-22-lst-adr-009-ab) | Decided | `@casoon/lightweight-pdf` JS/TS WASM package |
| **ADR-018** | [ZUGFeRD/Factur-X Einbettung](../decisions/#adr-018--zugferdfactur-x-nur-einbettung-keine-xml-erzeugung-issue-26) | Decided | Scope: electronic invoice embedding without XML generator |
| **ADR-019** | [Tagged PDF / PDF-UA-1](../decisions/#adr-019--tagged-pdfpdf-ua-rendernodetagged-wrapper-statt-feldnderung-pdf_ua-impliziert-pdf_a3b-issue-27) | Decided | Accessible PDF/UA structure trees |

👉 Read all full decisions: **[All Decisions (ADR-001 – ADR-019)](../decisions/)**
