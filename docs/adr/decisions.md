---
title: Decision Records (ADR-001 – ADR-019)
sidebarLabel: All Decisions
description: Full text of all 19 architecture decision records for lightweight-pdf.
order: 2
---

# Entscheidungsprotokoll (ADR-artig)

Grundsatzentscheidungen werden hier festgehalten, damit sie nicht mitten in
der Implementierung stillschweigend neu getroffen oder vergessen werden.
Jede Entscheidung: Status, Kontext, Entscheidung, Konsequenz.

## ADR-001 — Lizenz: MIT

**Status:** fixiert (Vorgabe).

Der gesamte Crate-Code (alle Workspace-Member) steht unter MIT. `Cargo.toml`
jedes Members: `license = "MIT"`. `LICENSE`-Datei im Repo-Root (Standard-MIT-
Text, Copyright-Zeile mit Jahr + Name/Org). Gebündelte Default-Fonts stehen
unter SIL OFL 1.1 (fremde Lizenz, siehe ADR-006) — das ist verträglich
(OFL erlaubt Embedding in Dokumenten und Weiterverbreitung als Teil eines
größeren Werks unter dessen eigener Lizenz), muss aber im Repo sauber
dokumentiert werden: `assets/fonts/LICENSES/OFL-*.txt` je Font, Hinweis im
Haupt-`README.md`.

## ADR-002 — Workspace-Layout

**Status:** übernommen aus Konzept, um eine Facade-Crate ergänzt.

Vier Kern-Crates wie im Konzept (`slimdoc-core`, `slimdoc-layout`,
`slimdoc-pdf`, `slimdoc-fonts`), plus:

- **`slimdoc`** — dünne Facade-Crate, re-exportiert die öffentliche
  Builder-API aus `slimdoc-core` unter einem Namen, den man tatsächlich in
  `Cargo.toml` von Anwendungscode einträgt (`slimdoc = "0.1"`). Das ist der
  Punkt, an dem `render()` öffentlich wird.
- **Wasm-Build als Cargo-Feature `wasm` in `slimdoc`**, keine eigene fünfte
  Crate. V1 ist Rust-first; sie enthält keine unvollständige JS-Builder-API
  und damit auch keine voreilige `wasm-bindgen`-Abhängigkeit. Die Facade
  liefert ein `cdylib`-Messartefakt; ein externer JS/TS-Einstiegspunkt braucht
  später ein eigenes, versioniertes Eingabeformat und einen ADR (ADR-009).

Layer-Abhängigkeitsrichtung bleibt strikt: `slimdoc-pdf` und `slimdoc-fonts`
kennen `slimdoc-layout`/`slimdoc-core` nicht; `slimdoc-layout` kennt
`slimdoc-core`s Elementtypen, aber nicht `slimdoc-pdf`; `slimdoc-core` kennt
niemanden. Diese Richtung wird durch die Crate-Grenzen erzwungen (nicht nur
Konvention) — das war der Punkt des Vier-Crate-Schnitts im Konzept.

## ADR-003 — Zwei begrenzte Dependency-Ausnahmen: `ttf-parser` und `png`

**Status:** entschieden, begründet durch Recherche (siehe unten).
`ttf-parser` wurde später wegen RUSTSEC-2026-0192 durch `skrifa` ersetzt
(ADR-015) — die hier begründete Grund-Entscheidung ("eine schmale,
reine-Rust Parsing-Dependency statt Eigenbau eines TrueType-Parsers")
gilt unverändert, nur die konkrete Crate hat gewechselt.

Die Vorgabe "100 % Rust, keine oder nur minimale, exakt begründete
Abhängigkeiten" wird nur für zwei kleine, reine Rust-Bibliotheken geöffnet:

- [`ttf-parser`](https://github.com/harfbuzz/ttf-parser), ausschließlich in
  `slimdoc-fonts`, mit `default-features = false`. Seine Tabellen (`glyf`,
  `loca`, `cmap`, `hmtx`) sind keine Cargo-Features; variable Fonts und CFF
  gehören nicht zum V1-Scope (ADR-012).
- die Crate [`png`](https://crates.io/crates/png), feature-reduziert und
  ausschließlich hinter dem optionalen `png`-Feature. Transparente PNGs brauchen Deflate-Decoding,
  Filter-Rekonstruktion und Alpha-Trennung; ein eigener Parser wäre weder
  kleiner noch robuster (ADR-013).

Begründung:

- Ein eigener TrueType/OpenType-Parser ist laut Konzept Kapitel 5/10 das
  größte Zeitplan-Risiko im ganzen Projekt — genau der Teil, an dem generische
  Bibliotheken bereits gelöste Arbeit mitbringen.
- `ttf-parser` ist `no_std`-fähig, zero-copy, keine Pflicht-Dependencies,
  keine Heap-Allokationen, in `resvg`/`fontdb`/`cosmic-text`/Typst selbst im
  produktiven Einsatz — also ein gut auditiertes, schmales Stück Code statt
  eines schweren Frameworks.
- Es deckt **nur Parsing** ab (für V1: `glyf`, cmap-Lookup, hmtx-Metriken).
  Subsetting (neue, kleinere Font-Datei aus ausgewählten Glyphen bauen) liefert
  es nicht — das bleibt eigener Code in `slimdoc-fonts` (Phase 4). Damit bleibt
  der Anteil an "wirklich eigenem" Code dort, wo er Kontrolle über Encoding
  und PDF-Font-Dictionaries gibt, und nur der undankbare Teil (binäres
  Tabellenformat robust parsen) wird ausgelagert.

**Explizit nicht übernommen:** `pdf-writer` (eigener Writer, siehe ADR-004),
`taffy` (eigenes Layout, siehe ADR-004), `fontdue` (Rasterizer — irrelevant,
PDF bekommt Vektor-Outlines, kein Bitmap-Rendering), `printpdf`/`genpdf`/
`lopdf` (zu viel Maschinerie/Fremdentscheidungen für das Größenziel).

## ADR-004 — Kein `taffy`, kein `pdf-writer`: Vorbild ja, Dependency nein

**Status:** entschieden.

- `slimdoc-pdf` wird als eigener, minimaler Objekt-/Xref-/Stream-Writer
  gebaut, architektonisch am Vorbild von `pdf-writer` orientiert (typisierte
  Objekt-IDs, Builder direkt über PDF-Syntax, keine Zwischenrepräsentation).
  `pdf-writer` selbst wird nicht eingebunden — Ziel ist Kontrolle über jedes
  geschriebene Byte plus echte Zero-Dependency an dieser Stelle (Konzept
  Kapitel 6 nennt das explizit als den Teil, der am wenigsten Risiko trägt
  und am mechanischsten ist).
- `slimdoc-layout`s Row/Column/gap/align-Modell übernimmt **Namensvokabular**
  von `taffy::Style` (`gap`, `align_items`, `flex_direction`-Analogie), aber
  nicht die Crate selbst. Begründung: das tatsächliche Layout-Problem hier
  ist klein und bekannt (Zeilen/Spalten mit fixem/flexiblem Anteil, kein
  generisches CSS-Grid, kein `calc()`), im Gegensatz zum TrueType-Parsing ist
  das kein unbekanntes Binärformat, sondern ein Algorithmus, den man selbst
  gut beherrschen kann — Risiko/Aufwand-Verhältnis spricht für Eigenbau.

## ADR-005 — Keine Makro-DSL, kein Parser: Builder-Pattern bleibt

**Status:** entschieden, Ergonomie-Leitlinien siehe `03-builder-api-design.md`.

Es wird **keine** eigene beschreibende Sprache (Typst-artig) und **kein**
proc-macro-DSL (`rsx!`/`maud`-artig) gebaut. Gründe:

- Widerspricht Konzept Kapitel 0 direkt ("keine separate Template-Sprache").
- Ein Parser + Interpreter für eine eigene Sprache ist erheblicher,
  eigenständiger Implementierungsaufwand mit eigenem Fehlerklassen-Set
  (Parse-Fehler, Laufzeitfehler in der Sprache selbst) — Aufwand, der an
  keiner Stelle für Rechnungen/Berichte gebraucht wird.
- Ein proc-macro-DSL zieht `syn`/`quote`/`proc-macro2` als Build-Dependency
  (Compile-Zeit-Kosten, nicht Laufzeit-Binärgröße — aber dennoch eine
  Abhängigkeit, die für simple Methodenketten nicht gerechtfertigt ist), und
  bringt keinen Vorteil, den `impl Into<Element>` + `impl IntoIterator` nicht
  auch mit reinem Rust liefern (siehe Recherche in `03-builder-api-design.md`).

Stattdessen: Builder-Methoden, die sich in ihrer *Lesbarkeit* an
elm-ui/SwiftUI/Typst orientieren, aber rein aus Methodenverkettung und
Trait-Konversionen bestehen — kein zusätzliches Sprachkonstrukt.

## ADR-006 — Default-Fonts: Source Sans 3 + Source Serif 4 (SIL OFL 1.1)

**Status:** entschieden, siehe `04-fonts.md` für Details.

Primärer Default: **Source Sans 3** (Text/Tabellen/Fließtext), optionaler
zweiter Default für Überschriften/Editorial-Look: **Source Serif 4**. Beide
SIL OFL 1.1, beide als statische TTF-Weights (Regular/Bold mind.) verfügbar,
`glyf`-Outlines (kein CFF nötig, passt zu `ttf-parser`s einfachstem Pfad),
Ziffern per Default tabellarisch (wichtig für Rechnungstabellen ohne
GSUB/`tnum`-Unterstützung), volle Latin-Extended-A-Abdeckung (deutsche
Umlaute, ß, €).

## ADR-007 — Überlauf-/Überlagerungs-Schutz ist Kernanforderung ab Phase 1

**Status:** entschieden, explizit angefordert. Details siehe
`05-overflow-and-robustness.md`.

Text-/Seitenumbrüche und Überlauf-Verhalten werden nicht als Detail einzelner
Phasen behandelt, sondern als Querschnittsanforderung von Phase 1 an
verbindlich mitgebaut:

- Auto-Size ist der Default für alle Container (Höhe/Breite ergibt sich aus
  dem Inhalt nach Umbruch); feste Größen sind Opt-in und verlangen dann eine
  explizite `overflow`-Policy (`Clip`, bei einzeiligem Text `Ellipsis`).
- Ein einzelnes Wort/Token, das breiter ist als der verfügbare Platz, wird
  hart auf Zeichenebene umgebrochen statt über den Rand hinauszulaufen.
- Der Render-Pass setzt für Header, Body, Footer und Container jeweils vor
  dem Zeichnen einen PDF-Clipping-Pfad; ein äußerer Clip begrenzt immer die
  physische Seite. Das ist ein
  Sicherheitsnetz, das Überlagerungen auch bei unvorhergesehenen
  Layout-Randfällen strukturell verhindert (fehlender Inhalt statt
  überlappender Inhalt).
- Tabellenzeilen wachsen standardmäßig mit ihrem höchsten Zellinhalt, statt
  Inhalt zu quetschen oder in Nachbarzeilen laufen zu lassen.
- Eine zusätzliche, rein additive Methode `render_with_diagnostics()`
  liefert `LayoutWarning`s (z. B. `TextClipped`, `ForcedPageBreak`) für
  Entwicklungs-/Testzwecke, ohne die bestehende `render()`-Signatur zu
  verändern. Die Warnungen werden zusammen mit den fertigen PDF-Bytes
  zurückgegeben, nie getrennt davon (Lehre aus InDesigns "Overset Text"-
  Warnung, die den PDF-Export nicht übersteht — siehe
  `05-overflow-and-robustness.md`, Grundprinzip 8).
- **Verschärfung über Container-Grenzen hinaus: die Seite selbst ist eine
  harte Grenze.** Body, Header und Footer haben getrennte Boxen; die
  physische Seite ist die äußerste Clip-Grenze. Nur die Body-Inhaltsbox wird
  rekursiv als Constraint durch den Dokumentkörper gereicht. Recherche zu TeX, CSS Paged Media/
  WeasyPrint und Typst zeigt, dass keines dieser Referenzsysteme das hart
  garantiert (alle drei lassen sichtbaren Überlauf über die Seite hinaus zu,
  Typst aktuell nicht mal mit Warnung) — `slimdoc` ist hier bewusst
  strenger, weil "Inhalt darf die Seite nie verlassen" explizit gefordert
  ist (Grundprinzip 6, `05-overflow-and-robustness.md`).
- **Witwen-/Waisenregelung + Heading-mit-Folgeinhalt-Regel** (Schwellwert
  `N = 2` Zeilen, einfache lokale Variante ohne TeX-artige globale
  Kostenoptimierung): ein Absatz mit zu wenigen Zeilen wird beim
  Seitenumbruch nicht aufgeteilt, sondern als Ganzes verschoben; eine
  Überschrift ohne folgenden Inhalt auf derselben Seite wird mitverschoben
  (Grundprinzip 9).

Begründung: das ist laut Nutzer ein wiederkehrender Schmerzpunkt bei
PDF-Generierung in der Praxis (Text überschreibt Nachbarelemente, Inhalt
passt nicht in Tabellenzellen/Seiten, Inhalt verlässt sichtbar die Seite) —
nachträgliches Reparieren wäre deutlich teurer als es von Anfang an im
`Layoutable`-Vertrag und im Render-Pass zu verankern. Betrifft konkret
Phase 1 (Grundmechanismus, Seiten-Inhaltsbox), Phase 2 (atomare Elemente >
Seitenhöhe, Witwen/Waisen), Phase 3 (Zeilenhöhe), Phase 5 (Bild-Fit), Phase 6
(Rotation/Wasserzeichen, Heading-Regel).

## ADR-008 — Größenbudget

**Status:** Zielkorridor festgelegt, wird ab Phase 0 laufend gemessen.

- **Ambitioniert:** < 300 KB nach `wasm-opt -Oz` für die Generator-Messung
  (PDF-Writer + Layout + Font-Metrik-Code, **ohne** Default-Fonts und PNG).
- **Realistischer Planungskorridor:** 300–500 KB. Referenzpunkte aus der
  Recherche: ein Rust-"Hello World" liegt nach `-Oz`+LTO bei ~17 KB; ein
  vergleichbares textverarbeitendes Wasm-Modul (Cloudflare HTML Rewriter)
  liegt bei 447 KB (345 KB gzip); "stateless business logic" Wasm-Services
  liegen typischerweise bei 200–800 KB. 300 KB als hartes Muss vor jeder
  Messung zu behaupten wäre unehrlich — es ist das Ziel, kein Fakt.
- Gemessen wird ab dem ersten lauffähigen Spike (Phase 0), nicht erst am
  Ende. Gemessen wird nur das in Phase 0a festgelegte `cdylib` mit aktivem
  `wasm-size-probe`; zusätzlich werden die realistischen Standard- und
  Integrationskonfigurationen erfasst. Kein CI-Hard-Fail auf eine feste Zahl
  in Phase 0/1 (noch keine belastbare Baseline) — ab Phase 2 wird ein
  CI-Warnschwellwert eingeführt, sobald eine Baseline existiert.

## ADR-009 — V1-Wasm-Grenze: Rust-first, keine implizite JS-API

**Status:** durch ADR-017 abgelöst (Issue #22) — die hier verlangte
Vorbedingung (separat versioniertes Eingabeformat) ist mit Issue #17
erfüllt.

Ein Rust-Worker baut den Dokumentbaum und rendert ihn direkt. `wasm` dient
V1 dem Build und der Größenmessung; eine JavaScript-/TypeScript-API wird erst
mit einem separat versionierten Eingabeformat geplant. Das verhindert, dass
eine nicht brauchbare `render(Document)`-Bindung oder ein unversioniertes
JSON nebenbei zur öffentlichen API wird.

## ADR-010 — Font-Bridge liegt an der Facade

**Status:** entschieden.

`slimdoc-layout` abstrahiert Metriken über `FontResolver`; `slimdoc-fonts`
kennt nur Fontdaten/Parsing, und `slimdoc` verbindet beides. `FontData` besitzt
seine Bytes und parst eine `ttf_parser::Face` nur kurzzeitig. Ein erster
metriktauglicher `glyf`-Pfad ist Teil des Spike, Subsetting folgt erst später.

## ADR-011 — Harte Seitenbereiche statt nachträglichem Seiten-Clip

**Status:** entschieden.

Header, Body und Footer haben feste, getrennte Bänder; der äußere Clip gilt
der physischen Seite. Jeder Clip wird vor seinem Inhalt gesetzt und danach
mit `Q` beendet. `Overflow::Visible` entfällt in V1. Die festen Bänder machen
die Seitenzahl zwischen Pass 1 und Pass 2 invariant; Überhöhe wird gewarnt
und geclippt, nie durch eine offene Neu-Pagination "gelöst".

## ADR-012 — Schrift-Scope und PDF-Textverträglichkeit

**Status:** entschieden.

V1 unterstützt statische TrueType-`glyf`-Fonts. Variable und CFF/OTF-Fonts
werden klar abgelehnt. Subsets schließen zusammengesetzte Glyphen ein,
erzeugen korrekte Tabellen-Prüfsummen und werden als Type-0/CIDFont mit
`Identity-H`, `CIDToGIDMap` und `ToUnicode` geschrieben. Sichtbarkeit,
Kopierbarkeit und Suche sind gleichwertige Abnahmekriterien.

## ADR-013 — PNG über geprüften Decoder, mit klaren Eingabegrenzen

**Status:** entschieden.

Transparente PNGs werden über eine begrenzte, reine-Rust Decoder-Dependency
verarbeitet, nicht über einen Eigenbau. V1 akzeptiert nicht-interlaced 8-Bit
RGB/RGBA sowie alle PNG-Filter, erzeugt für Alpha eine `SMask` und begrenzt
Pixelzahl/Dekompressionsgröße. JPEG beschränkt sich auf Baseline RGB/Gray;
alles andere wird mit einem fachlichen Fehler abgewiesen. Das optionale
`png`-Feature wird getrennt gemessen.

## ADR-014 — Phase-0-Größenmessung: Baseline liegt über dem Zielkorridor, `wasm-size-probe` koppelt an `default-fonts`

**Status:** entschieden (Spike-Ergebnis, gemäß Phase-0-Risiko-Abschnitt hier
explizit dokumentiert statt stillschweigend akzeptiert).

**Messung (Generator-Konfiguration, `--no-default-features --features
wasm,wasm-size-probe,default-fonts`, `wasm-opt -Oz --all-features`):**

| Wert | Größe |
|---|---|
| roh | 1.003.555 Byte |
| `wasm-opt -Oz` | 931.300 Byte |
| gzip -9 | 468.747 Byte |

Standard-Konfiguration (zusätzlich `png`-Feature) ist aktuell **identisch**,
weil das `png`-Feature noch keinen Code enthält (Bild-Einbettung ist
Phase-5-Scope, noch nicht implementiert) — kein Messfehler, sondern Stand
vor Phase 5.

**Abweichung 1 — Baseline über Korridor:** 931 KB liegt deutlich über dem
ambitionierten Ziel (< 300 KB) und auch über dem realistischen
Planungskorridor (300–500 KB) aus ADR-008. Ursache: die Generator-Messung
bettet aktuell zwei vollständige, ungekürzte Source-Sans-3-Gewichte
(~430 KB + ~428 KB TTF) ein — genau das von Phase 0 explizit verlangte
"kein Subsetting"-Verhalten. Der eigentliche Generator-Code (Writer +
Layout + Font-Metrik-Parsing) macht davon nur einen kleinen Teil aus. Diese
Überschreitung ist ein **erwartetes, im Plan selbst vorgesehenes
Spike-Ergebnis** (Kapitel "Risiko/Abbruchkriterium" in
`phases/phase-0-spike.md`) und kein Abbruchkriterium — die eigentliche
Kontrolle über die Größe kommt erst mit Font-Subsetting (Phase 4). Wird ab
Phase 4 erneut gemessen und mit dieser Baseline verglichen.

**Abweichung 2 — `wasm-size-probe` verlangt `default-fonts`:** Die
Drei-Konfigurations-Trennung aus `00a-contracts-and-artifacts.md` Punkt 2
sieht die Generator-Messung explizit *ohne* Default-Fonts vor, um
Schreiber-/Layout-/Font-Parsing-Code isoliert von den Font-Assets zu
messen. In der Umsetzung braucht `render()` aber zwingend eine Font-Quelle,
und V1 hat noch keine Custom-Font-API (die kommt erst mit Phase 4/
Subsetting) — ohne `default-fonts` gibt es schlicht nichts, das
`wasm-size-probe` rendern könnte. `slimdoc/src/lib.rs` erzwingt die
Kopplung deshalb explizit über `compile_error!`
(`wasm-size-probe` ohne `default-fonts` schlägt beim Bauen fehl), statt sie
stillschweigend inkonsistent zu lassen. Eine echte Trennung bräuchte einen
zweiten, nur für die Messung bestimmten Mini-Testfont unabhängig vom
`default-fonts`-Feature — als TODO für Phase 4 vorgemerkt, wenn die
Font-Schicht ohnehin überarbeitet wird.

**Konsequenz:** kein CI-Hard-Fail (ADR-008 sieht ohnehin erst ab Phase 2
einen Warnschwellwert vor, sobald eine belastbare Baseline existiert) —
diese Messung *ist* die Baseline, mit der Phase-4-Ergebnisse verglichen
werden.

**Nachtrag nach Phase 4:** Generator-Messung liegt jetzt bei 984.385 Byte
nach `wasm-opt -Oz` (+5,7 % ggü. der hier dokumentierten Baseline). Das
bestätigt die oben schon erwartete Richtung: Subsetting-Code (Composite-
Glyph-Closure, eigener `cmap`-Format-4-Encoder, Prüfsummenlogik) kommt zur
Wasm-Binary hinzu, während die eingebetteten Fontbytes selbst unverändert
bleiben (die *volle* Font muss weiterhin im Wasm liegen, damit zur Laufzeit
pro Dokument daraus subsettet werden kann) — Subsetting verkleinert die
**PDF-Ausgabe** drastisch (siehe `progress.md`), nicht die Wasm-Binary.
Der Mini-Testfont-TODO für eine isolierte Generator-Messung bleibt offen,
ist aber nicht mehr an Phase 4 gebunden (keine feste Folgephase).

## ADR-015 — `ttf-parser` unmaintained (RUSTSEC-2026-0192): auf `skrifa` migriert

**Status:** umgesetzt. Ursprünglich als "beobachten, nicht sofort wechseln"
entschieden (siehe Bewertung unten); nach echtem Quellcode-Abgleich von
`skrifa`/`read-fonts` (0.46.1) stellte sich der Umbau als deutlich
mechanischer heraus als hier ursprünglich vermutet, siehe Nachtrag.

**Befund:** `cargo audit` meldet `ttf-parser 0.24.1` als `unmaintained`
(informational, keine Sicherheitslücke) — der Autor hat die Pflege laut
verlinktem Issue eingestellt. Empfohlene Alternative der Advisory:
[`skrifa`](https://crates.io/crates/skrifa) (Google-Fonts-"oxidize"-Projekt).

**Bewertung:**

- Kein CVE, keine bekannte Sicherheitslücke — reine Maintainer-Warnung.
  `ttf-parser` bleibt weiterhin ein schmaler, auditierter, reiner Parser
  ohne Laufzeit-Netzwerk-/Dateisystemzugriff; das Risiko einer stillen
  Kompromittierung ist gering, das Risiko ist "bekommt keine Fixes mehr für
  neue Font-Edge-Cases", nicht "aktiv gefährlich".
- `lightweight-pdf-fonts` nutzt laut ADR-003 nur einen kleinen Ausschnitt
  (`glyf`, `loca`, `cmap`, `hmtx`) mit `default-features = false` — die
  Angriffsfläche ist bereits eng begrenzt.
- Ein Wechsel zu `skrifa` ist keine mechanische Ersetzung: `skrifa` hat eine
  andere API-Form (u. a. `FreeTypeOutline`-Callback-Traversal statt
  `ttf_parser::Face::outline_glyph`), berührt also `MetricsAdapter`
  (`crates/lightweight-pdf/src/fonts.rs`) und die Subsetting-Logik in
  `lightweight-pdf-fonts` direkt — ein eigener, nicht-trivialer Umbau, kein
  Drop-in.

**Entscheidung:** kein Fork, kein sofortiger Wechsel zu `skrifa`.
Stattdessen:

1. `cargo audit` bleibt in CI als reportender (nicht blockierender) Schritt
   aktiv (siehe `.github/workflows/ci.yml`), damit ein Eskalieren zu einer
   echten Sicherheitslücke sofort auffällt.
2. Diese ADR ist der dokumentierte Risiko-Nachweis, den das
   Projektbewertungs-To-do verlangt — kein stillschweigendes Ignorieren der
   Warnung.
3. Ein Wechsel zu `skrifa` wird erneut geprüft, sobald ohnehin an
   `lightweight-pdf-fonts`/Subsetting gearbeitet wird (z. B. eine
   Custom-Font-API, siehe `fonts.rs`-Kommentar zu Phase 4+), nicht als
   isolierte Abhängigkeits-Migration.

**Nachtrag — Migration durchgeführt:** Auf Nutzeranfrage wurde `skrifa`
(0.46.1) real gezogen und der tatsächliche Quellcode gegen jede
`ttf-parser`-Nutzungsstelle abgeglichen, statt aus dem Gedächtnis zu
schätzen. Ergebnis: Der oben vermutete "eigene, nicht-triviale Umbau" war
im Kern **mechanisch**, weil `subset.rs`s eigentliche Subsetting-Logik
(Composite-Glyph-Closure, `cmap`-Format-4-Encoder, sfnt-Assembly,
Checksum) schon vorher nur rohe Tabellen-Bytes via
`ttf_parser::Face::raw_face().table(Tag)` konsumierte — `read_fonts`s
`TableProvider::data_for_tag(Tag)` liefert exakt dasselbe (rohe Bytes),
sodass ~95 % von `subset.rs` unverändert blieben. Einzige echte Reibung:
`GlyphId` ist bei `skrifa`/`font-types` ein `u32`-Newtype (vorher `u16`
bei `ttf-parser`) — überall `.to_u32() as u16` an der Grenze zu unserem
u16-GID-Raum nötig. `is_bold`/`is_italic` wird jetzt über
`skrifa::attribute::Attributes` (Gewicht als `f32`, Stil als Enum inkl.
`Oblique`) bestimmt statt über `ttf-parser`s boolesche `is_bold()`/
`is_italic()` — bewusst weiter gefasst (`Oblique` zählt jetzt auch als
kursiv), aber dafür gab es vorher keine Tests mit konkreten Erwartungen
("blindes Terrain" laut ursprünglicher Bewertung oben).

**Verifikation:** Alle 14 `lightweight-pdf-fonts`-Tests grün nach dem
Umbau. Wichtiger noch: `sha256sum` von acht generierten Beispiel-PDFs
(`invoice`, `report`, alle sechs `demo_*`, inkl. `--features png`) vor und
nach der Migration ist **byte-identisch** — die Umstellung ändert weder
Glyph-Auswahl noch Breiten noch die erzeugten PDF-Bytes. Ganzer
Workspace-Testlauf (`--all-features`), `clippy` (Default/`png`/
`--no-default-features --lib`), Wasm-Build und die Crate-Boundary-Checks
bleiben grün. `cargo audit` meldet danach keine Advisories mehr.

**Wasm-Größe (Kontext ADR-008/014):** Generator-Konfiguration nach
`wasm-opt -Oz`: 965.732 Byte (vorher zuletzt dokumentiert: 984.385 Byte,
−1,9 %). Standard-Konfiguration (inkl. `png`): 1.056.590 Byte (vorher
1.086.858 Byte, −2,7 %). `skrifa` zieht zwar mehr Sub-Crates
(`read-fonts`, `font-types`, `bytemuck`) als der reine `ttf-parser`, aber
nach Dead-Code-Elimination durch `wasm-opt` bleibt für unseren schmalen
Nutzungsausschnitt (glyf-only, kein CFF/Bitmap/Color/Variable-Font-Code)
netto weniger übrig — kein Size-Malus, entgegen der naiven Erwartung "mehr
Abhängigkeiten = größer".

## ADR-016 — FlateDecode via `miniz_oxide`, nicht Eigenbau (Issue #2)

**Status:** entschieden.

Content-Streams, Font-Programme (`FontFile2`) und rohe Bild-Samples wurden
bislang unkomprimiert geschrieben ("No compression in V1"). Zwei Optionen
standen zur Wahl:

1. `miniz_oxide` hinter einem Cargo-Feature `compress` (default an).
2. Ein eigener Deflate-Encoder mit Fixed-Huffman-Codes (~300 Zeilen, keine
   Dependency), der laut Schätzung ~60–70 % der Ersparnis von Option 1
   erreicht.

**Entscheidung: Option 1.** Begründung, abweichend vom sonst in ADR-003/004
etablierten Muster ("Eigenbau, wenn das Problem klein und beherrschbar
ist"):

- Fixed-Huffman allein (ohne LZ77-Rückverweise) komprimiert PDF-
  Content-Streams/Font-Binärdaten kaum — die eigentliche Ersparnis kommt
  aus LZ77-Backreferences, nicht aus der Entropiecodierung. Ein
  korrekter LZ77+Huffman-Encoder ist kein "kleines, bekanntes Problem"
  wie das Row/Column-Layout (ADR-004) oder das TrueType-Parsing bereits
  gelöst mitbringt (ADR-003) — es ist ein Bitstream-Format mit vielen
  Fallstricken (Bit-Packing-Reihenfolge, Code-Längen-Tabellen,
  Backreference-Kodierung), bei dem ein Fehler **jede** erzeugte PDF-Datei
  korrumpieren kann.
- `miniz_oxide` ist reines Rust, `no_std`-fähig (mit `alloc`), zieht genau
  eine Sub-Dependency (`adler2`, die Prüfsummen-Implementierung) und ist
  breit im produktiven Einsatz (u. a. als Rust-Ersatz für `zlib`/`miniz` in
  vielen Crates) — ein gut auditiertes, schmales Stück Code, nicht ein
  schweres Framework (dieselbe Güteklasse wie `ttf-parser`/`skrifa` in
  ADR-003).
- Kollidiert mit ADR-003 ("keine oder nur minimale Abhängigkeiten"), aber
  bewusst als dritte, eng begründete Ausnahme behandelt — hier ist
  Korrektheit für einen Codepfad, der **jede** Ausgabedatei durchläuft,
  wichtiger als Dependency-Null.

**Umsetzung:** `lightweight-pdf-writer::PdfWriter::compressed_stream()`
(zlib-Wrapper, RFC 1950 — das ist, was `/FlateDecode` laut PDF-Spec 7.4.4
erwartet) für Content-Streams, `FontFile2` und `ToUnicode`-CMaps.
Bild-Samples nur, wenn sie noch nicht komprimiert sind
(`ImageDataFilter::None`, d. h. rohe PNG-Pixel) — JPEG (`DctDecode`) bleibt
unverändert, erneutes Deflaten von bereits komprimierten/quasi-zufälligen
Bytes bringt nichts und kostet nur CPU. Feature `compress`, Default an,
in Writer und Facade gleichermaßen; ohne das Feature bleibt der bisherige
unkomprimierte Pfad erhalten (z. B. für `--no-default-features`).

## ADR-017 — wasm-bindgen-Bindings + npm-Paket (Issue #22, löst ADR-009 ab)

**Status:** entschieden.

**Kontext:** ADR-009 hat eine JS-API bewusst vertagt, bis ein separat
versioniertes Eingabeformat existiert — mit Issue #17 (serde-Layer,
`{"schema_version": 1, ...}`) ist das jetzt der Fall. Zielgruppe
"JS/TS-Entwickler ohne Rust-Kenntnisse, oft in einer Edge-Runtime" ist
laut Issue-Text der größte Reichweiten-Hebel des Projekts.

**Entscheidung:**
- Bindings hinter dem bestehenden `wasm`-Feature (nicht neu), das jetzt
  `serde` impliziert. Rust-seitige Oberfläche bewusst minimal: ein
  Dokument/Template geht als JSON-*String* rein (`Document::from_json`
  validiert ohnehin schon vollständig — ein zweiter, weniger strenger
  Objekt-Walk an der JS-Grenze wäre nur eine zusätzliche Fehlerquelle),
  Fonts werden als rohe Bytes registriert. `LightweightPdf` (wasm-bindgen-
  Klasse) hält den `FontRegistry`-State; `render`/`renderWithDiagnostics`/
  `renderTemplate` als Methoden. `RenderResult.bytes` ist ein echtes
  `Uint8Array` (wasm-bindgens eingebautes `Vec<u8>`-Mapping, keine
  JSON-Zahlen-Array), `.warnings` ein strukturiertes Array über
  `serde-wasm-bindgen` (eigener `WasmWarning`-Typ in der Facade, damit
  `lightweight-pdf-layout::LayoutWarning` selbst kein `serde` braucht).
- TypeScript-Typen für das Dokumentformat werden aus dem `schemars`-
  generierten JSON-Schema abgeleitet (`lwpdf schema` → `json-schema-to-
  typescript`), nicht handgepflegt — neue `schemars`-Ableitung parallel
  zu den bestehenden `serde`-Ableitungen in `lightweight-pdf-core`
  (gleiches Muster, `schemars` impliziert `serde`). `FontKey`/`Image`
  (eigene, nicht abgeleitete `Serialize`/`Deserialize`, siehe ADR zu
  Issue #17) bekommen dafür einen eigenen, passenden `JsonSchema`-Impl.
- npm-Paket unter `bindings/js/` (`@casoon/lightweight-pdf`), Build-Kette
  `wasm-pack build --target web` → `wasm-opt` → `lwpdf schema` →
  `json2ts` → `tsc`. Dünner Hand-Wrapper (`src/index.ts`, `render()`)
  bietet die im Issue verlangte `render(document: Document):
  Promise<Uint8Array>`-API; erkennt Node zur Laufzeit und liest die
  `.wasm`-Datei selbst (Knotens `fetch` unterstützt kein `file://`,
  der `--target web`-Default-Ladepfad geht sonst leer aus) — Browser/
  Bundler/Edge-Runtimes bekommen weiterhin den Standard-Ladepfad bzw.
  können ihre eigenen wasm-Bytes übergeben (z. B. ein künftiges
  Cloudflare-Worker-Template, Issue #23).
- Build+Veröffentlichung des npm-Pakets nur als taggetriggerter
  Release-Job, nicht im PR-Lauf (der PR-Pfad prüft weiterhin nur, dass
  `--target wasm32-unknown-unknown --features wasm,...` kompiliert —
  das deckt der bestehende `wasm-size`-Job schon ab).

**Nebenbefund beim Implementieren (kein Rust-Bug, aber real):**
`wasm-pack`s eigener `wasm-opt`-Post-Processing-Schritt schlägt mit dem
aktuellen rustc validierungsseitig fehl ("memory.copy operations require
bulk memory operations"/"i32.trunc_sat_f32_u" ohne
`--enable-nontrapping-float-to-int") — rustc emittiert inzwischen
standardmäßig Bulk-Memory- und Non-Trapping-Float-to-Int-Operationen für
`wasm32-unknown-unknown`, `wasm-pack`s interner `wasm-opt`-Aufruf
kennt aber nur die alten Default-Flags. Behoben über
`[package.metadata.wasm-pack.profile.release] wasm-opt = false` +
eigener `wasm-opt -Oz --enable-bulk-memory
--enable-nontrapping-float-to-int`-Schritt in `bindings/js`s Build-Skript
— exakt dieselben zwei Flags, mit denen auch der bestehende `wasm-size`-
CI-Job jetzt misst (der vorher `--all-features` nutzte, was zwar eine
Zahl lieferte, aber ein Binary erzeugte, das echte JS-Engines gar nicht
laden können — "unknown import kind"; mit einem echten `node`-Ladetest
verifiziert, dass die neuen Flags ein tatsächlich lauffähiges Modul
ergeben).

## ADR-018 — ZUGFeRD/Factur-X: nur Einbettung, keine XML-Erzeugung (Issue #26)

**Status:** entschieden.

**Kontext:** Issue #26 stellt die Grundsatzfrage explizit: erzeugt dieses
Crate die Rechnungs-XML selbst, oder nimmt es sie nur als Bytes entgegen?
Empfehlung des Issues: nur einbetten.

**Entscheidung: nur Einbettung.** `Document::zugferd_xml(bytes)` nimmt
fertige, vom Aufrufer bereitgestellte EN-16931-`CrossIndustryInvoice`-
XML-Bytes entgegen und bettet sie unverändert ein — dieses Crate parst,
validiert oder erzeugt diese XML nicht.

Begründung:
- EN-16931/ZUGFeRD-XML-Erzeugung ist ein eigenständiges, großes
  Geschäftsdomänen-Problem (Rechnungspositionen, Steuerkategorien,
  Zahlungsbedingungen, Partei-/Adressdaten in einem eigenen komplexen
  Schema) — komplett orthogonal zu PDF-Layout, dem eigentlichen Zweck
  dieses Crates.
- Ein Aufrufer, der bereits ein Buchhaltungs-/ERP-System hat, hat die
  Rechnungsdaten strukturiert bereits vorliegen — die natürliche
  Erzeugungsstelle für die XML ist dort, nicht in einer PDF-Bibliothek.
- Hält die Abhängigkeitsgrenze sauber: `lightweight-pdf-writer` bettet
  Bytes ein (PDF-Mechanik: Filespec, `/AF`, XMP-Extension-Schema), ohne
  je ein XML-Schema/Schematron-Regelwerk zu kennen.
- Falsch-positive Konformitätsbehauptungen vermeiden: eine
  XML-Erzeugung, die selbst nicht korrekt ist, würde den Eindruck einer
  vollständigen ZUGFeRD-Lösung erwecken, obwohl nur die PDF-Seite
  geprüft ist.

**Umsetzung:** `Document::zugferd_xml(bytes)` (impliziert `.pdf_a3b()` —
ZUGFeRD *ist* PDF/A-3 plus eingebettete Rechnung, kein unabhängiges
Opt-in) auf `lightweight-pdf-core::Document`, immer vorhanden. Die
eigentliche Einbettung (Kosten: XMP-Extension-Block + Filespec-Objekte)
hinter einem `zugferd`-Feature auf `lightweight-pdf-writer`/
`lightweight-pdf` (impliziert `pdf-a`). Ohne das Feature liefert
`render()` `RenderError::ZugferdFeatureDisabled`.

Geschrieben: `/Type /EmbeddedFile /Subtype /text#2Fxml` (Flate-
komprimiert wie jeder andere Stream), ein `/Type /Filespec` mit
`/AFRelationship /Alternative` (wörtlich wie im Issue verlangt),
referenziert sowohl über `/AF` (PDF/A-3-eigener Mechanismus) als auch
`/Names/EmbeddedFiles` (älterer, universeller Anhänge-Namensbaum, den
die meisten PDF-Viewer für ihr Anhänge-Panel nutzen — beide zeigen auf
dasselbe Objekt). XMP-Erweiterung exakt nach PDFlibs eigenem
Referenz-Sample übernommen (`fx:*`-Namespace `urn:factur-x:pdfa:
CrossIndustryDocument:invoice:1p0#`, `pdfaExtension`/`pdfaSchema`/
`pdfaProperty`-Schema-Beschreibung, von PDF/A-3 für jeden
Custom-Namespace verlangt) statt aus der Spec-Beschreibung
rekonstruiert — dieser Block ist genau die Art Konstrukt, bei der ein
funktionierendes Referenzbeispiel einer Neuherleitung vorzuziehen ist.

Profil-Scope wie im Issue vorgegeben auf EN-16931 (Comfort) begrenzt —
`fx:ConformanceLevel` fest auf `EN 16931`, kein Profil-Parameter.

**Verifikation:** `examples/demo_zugferd.rs` bettet eine echte,
EN-16931-konforme Beispielrechnung aus dem ZUGFeRD-Referenzkorpus ein
(`test-fixtures/zugferd/en16931-sample.xml`, Apache-2.0, aus dem Mustang
Project). Geprüft mit zwei unabhängigen, echten Werkzeugen: veraPDF
(PDF/A-3b-Container: PASS) und der Mustang-CLI (die ZUGFeRD-
Referenzimplementierung, `--action validate`; prüft PDF *und* die
eingebettete XML gegen die EN-16931-Schematron-Regeln): PDF `valid`, XML
`valid`, Gesamt `valid`. Lokal verifiziert, nicht in CI verankert — im
Gegensatz zu #25 verlangt #26 selbst keine CI-Prüfung, und Mustang-CLI
(~59 MB Jar) wäre ein deutlich teurerer CI-Zusatz als veraPDF für einen
Codepfad, dessen PDF-Seite `verapdf --flavour 3b` (der bestehende
`pdf-a-conformance`-Job) strukturell bereits mitprüft, sobald `zugferd`
zu dessen `--all-features`-Kombination gehört (was es über die Facade
tut) — der neue Code kompiliert und lintet also bereits bei jedem Lauf.

## ADR-019 — Tagged PDF/PDF-UA: `RenderNode::Tagged`-Wrapper statt Feldänderung, `pdf_ua()` impliziert `pdf_a3b()` (Issue #27)

**Status:** entschieden.

**Kontext:** Issue #27 verlangt einen echten Strukturbaum
(`/StructTreeRoot`, `/StructElem` je Überschrift/Absatz/Tabelle/Liste/
Bild), `BDC`/`EMC`-markierte Inhalte mit MCIDs, Artefakt-Markierung für
Wasserzeichen/Kopf-/Fußzeile, eine `Image::alt`-API mit Warnung bei
fehlendem Alt-Text, und echte veraPDF-PDF/UA-Bestätigung. Das ist die
architektonisch invasivste Änderung der Session — sie berührt
`lightweight-pdf-layout` (Layout), `lightweight-pdf-writer` (Schreiben)
und die Facade (Rendern) gleichermaßen.

**Entscheidung 1: `RenderNode` bekommt einen neuen `Tagged { role,
inner }`-Varianten-Wrapper statt eines neuen Felds auf jeder bestehenden
Variante.** `RenderNode::Group` hätte ein `struct_role: Option<...>`-Feld
bekommen können, aber das hätte jede bestehende Konstruktionsstelle in
`table.rs`/`list.rs`/`row.rs`/`column.rs`/etc. angefasst (~15-20 Stellen).
Ein Wrapper-Variant berührt nur die (wenigen) *exhaustiven* Matches über
`RenderNode` — von denen sich beim Umsetzen vier als bereits vorhandene,
Group-only-rekursive Walker herausstellten, die durch die neue,
UNBEDINGTE Tagged-Wrapping jedes `Text`/`Image`/`Table`/`List`/
`TableOfContents`-Elements (auch ohne `pdf_ua()`!) sonst BLIND geworden
wären: `collect_chars_in_node`/`collect_anchors_in_node`/
`collect_headings_in_node` (Facade, `render/text.rs`) und
`collect_anchor_pages_in_node` (Layout, `toc.rs`) — ohne den Fix hätte
das Font-Subsetting für JEDES Dokument aufgehört, Zeichen zu finden. Real
entdeckt beim ersten `cargo test`-Lauf nach der Dispatch-Änderung, nicht
vorher erkannt.

**Entscheidung 2: Tagging passiert unbedingt am `Element::layout`-
Dispatch (`layoutable/mod.rs`), nicht nur wenn `pdf_ua()` gesetzt ist.**
`Tagged`-Wrapping kostet zur Laufzeit nichts, wenn niemand es konsumiert
— `tree::render_tagged` prüft `ctx.struct_tree.is_none()` und rendert
`inner` dann komplett transparent (kein BDC/EMC, keine Strukturbaum-
Arbeit). Ein bedingtes Wrapping hätte `LayoutCtx` um ein `pdf_ua: bool`
erweitern und durch jede Layout-Funktion durchreichen müssen — für einen
Fall, der ohnehin schon kostenlos ist, sobald der Konsument (die Facade)
selbst entscheidet, ob er die Tags nutzt.

**Entscheidung 3: `Document::pdf_ua()` impliziert `.pdf_a3b()`.** PDF/UA
verlangt PDF/A technisch nicht, aber beide brauchen dieselbe XMP-/
OutputIntent-Maschinerie, und ein kombiniertes PDF/A+PDF/UA-Dokument
(archivierbar *und* barrierefrei) ist real das, was die meisten
Produzenten dieser Dokumentklasse wollen. `tagged-pdf` (Cargo-Feature)
impliziert entsprechend `pdf-a`. PDF/UA *ohne* PDF/A ist damit bewusst
nicht unterstützt.

**Entscheidung 4: Grouping- vs. Leaf-Rollen teilen sich denselben
`enter`/`exit`-Aufruf.** `Table`/`TR`/`TH`/`TD`/`List`/`LI`/`Lbl`/
`LBody`/`TOC`/`Document` sind reine Gruppierung (kein eigener MCID, nur
verschachtelte `StructElem`s); `H1`-`H6`/`P`/`Figure` sind Blätter (genau
ein `BDC`/`EMC`-Aufruf, ein `ContentRef`-Kind). Beide Formen laufen durch
denselben `StructTreeBuilder::enter()`/`exit()`-Aufruf — der einzige
Unterschied ist, was DAZWISCHEN passiert (weiter rekursieren vs. eine
MCID markieren). Das vereinheitlicht den Code in `tree::render_tagged`
auf einen einzigen Pfad statt zwei parallele.

**Bewusste Vereinfachung:** eine über eine Seitenumbruch-Grenze
gesplittete Tabelle/Liste erzeugt PRO SEITE ein eigenes `/Table`/`/List`-
`StructElem` (Geschwister im Elternknoten) statt eines einzigen
logischen Elements, dessen `/K`-Einträge über mehrere Seiten verteilt
sind. Strukturell gültiges Tagged PDF (nichts in der Spec verlangt EIN
Element über Seiten hinweg), aber nicht maximal elegant — dafür deutlich
einfacher umzusetzen, ohne die Pagination-Logik selbst anzufassen.

**Verifikation, drei über echte veraPDF-Läufe gefundene, nicht aus der
Spec-Prosa ableitbare Lücken:**
1. `/Artifact BDC` (ohne Properties-Operand) ist fehlerhaftes BDC-Syntax
   — `BDC` verlangt zwingend zwei Operanden; ohne eigene Properties ist
   `BMC` (ein Operand) der richtige Operator. Fund: "Undefined property
   /Artifact in a content stream".
2. `/ViewerPreferences << /DisplayDocTitle true >>` fehlte im Catalog
   (ISO 14289-1 7.1).
3. `TH`-Zellen brauchen `/A << /O /Table /Scope /Column >>` — ohne das
   kann ein Reader die Spalten-Zuordnung nicht algorithmisch herleiten,
   selbst bei einer trivial einfachen Ein-Header-Zeile-Tabelle. Fund:
   "TD does not contain Headers attribute, and Headers for this table
   cell cannot be determined algorithmically".
4. `pdfuaid:*`-XMP-Properties brauchen — genau wie ZUGFeRDs `fx:*` in
   Issue #26 — eine eigene `pdfaExtension`/`pdfaSchema`/`pdfaProperty`-
   Schema-Beschreibung, sonst lehnt PDF/A-3bs eigene XMP-Validierung sie
   als "nicht vordefiniert" ab — erst bemerkt, weil `pdf_ua()`
   `pdf_a3b()` impliziert und dieselbe Datei dadurch BEIDE Profile
   gleichzeitig bestehen muss.

**`Image::alt` — kein Platzhaltertext bei fehlendem Alt.** Ohne
`.alt(...)` schreibt dieses Crate `/Alt ()` (leerer String, hält den
Strukturbaum wohlgeformt) und meldet
`LayoutWarningKind::MissingAltText` über `render_with_diagnostics()` —
verifiziert, dass ein leerer String NICHT ausreicht, damit veraPDF
PDF/UA-1 als bestanden meldet ("Figure structure element neither has an
alternate description nor a replacement text"). Bewusst kein erfundener
Platzhaltertext: das wäre für eine Screenreader-Nutzerin irreführender
als eine ehrliche Lücke plus Warnung.

**Verifiziertes Endergebnis** (`examples/demo_pdf_ua.rs`: Überschriften,
Absatz, Tabelle mit Header, gemischte Bullet-/Nummern-Liste, Bild mit
Alt-Text, Wasserzeichen, Seitenzahl-Fußzeile): `verapdf --flavour ua1`
UND `--flavour 3b` bestehen beide, `qpdf --check` fehlerfrei,
`pdftotext`-Extraktion sauber (Wasserzeichen/Fußzeile bewusst NICHT als
"echter" Lesefluss-Inhalt, da `pdftotext` selbst nicht struktur-baum-
bewusst ist — Artefakt-Markierung wirkt für Screenreader/veraPDF, nicht
für einen rohen Text-Dump).
