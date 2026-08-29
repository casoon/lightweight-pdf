//! Thin, allocation-light layer directly over PDF object syntax: typed
//! object IDs, dict/stream writing, xref table + trailer. Architecturally
//! modeled after `pdf-writer` (ADR-004) but self-written for full control
//! over every emitted byte, and (still) no required dependencies — the one
//! optional dependency, `miniz_oxide` behind the `compress` feature
//! (ADR-016), is FlateDecode compression, not object-model plumbing.

/// A PDF indirect object reference (generation is always 0 in V1 — we never
/// rewrite an existing file).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ref(pub u32);

impl Ref {
    /// `"N 0 R"` as used inside dictionaries/arrays.
    pub fn write(&self) -> String {
        format!("{} 0 R", self.0)
    }
}

pub struct PdfWriter {
    buf: Vec<u8>,
    /// Byte offset of each object, indexed by `id - 1` (object 0 is the
    /// reserved free-list head and is not stored here).
    offsets: Vec<usize>,
    next_id: u32,
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfWriter {
    pub fn new() -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
        PdfWriter {
            buf,
            offsets: Vec::new(),
            next_id: 1,
        }
    }

    pub fn alloc(&mut self) -> Ref {
        let id = self.next_id;
        self.next_id += 1;
        Ref(id)
    }

    fn record_offset(&mut self, id: Ref) {
        // `id.0` is a `u32` object id; `usize` is at least 32 bits on every
        // platform this crate targets, so this widening conversion never
        // fails.
        let idx = usize::try_from(id.0 - 1).expect("PDF object ids fit in usize for any realistic document, see round 2 rationale");
        if self.offsets.len() <= idx {
            self.offsets.resize(idx + 1, 0);
        }
        self.offsets[idx] = self.buf.len();
    }

    /// Writes a plain indirect object: `id 0 obj\n<body>\nendobj\n`.
    pub fn object(&mut self, id: Ref, body: &str) {
        self.record_offset(id);
        self.buf.extend_from_slice(format!("{} 0 obj\n", id.0).as_bytes());
        self.buf.extend_from_slice(body.as_bytes());
        self.buf.extend_from_slice(b"\nendobj\n");
    }

    /// Writes an indirect stream object, uncompressed. `dict_extra` are
    /// additional dictionary entries (e.g. `/Length1 1234`); `/Length` is
    /// computed and added automatically.
    pub fn stream(&mut self, id: Ref, dict_extra: &str, data: &[u8]) {
        self.record_offset(id);
        self.buf
            .extend_from_slice(format!("{} 0 obj\n<< /Length {} {} >>\nstream\n", id.0, data.len(), dict_extra).as_bytes());
        self.buf.extend_from_slice(data);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    /// `Self::stream`'s DEFLATE-compressed counterpart (ADR-016): adds
    /// `/Filter /FlateDecode` and zlib-wraps `data` (RFC 1950 — what
    /// `/FlateDecode` expects per PDF 32000-1 7.4.4) before writing. Used
    /// for content streams, embedded font programs (`FontFile2`) and
    /// `ToUnicode` CMaps — never for data that's already compressed (e.g.
    /// JPEG `/DCTDecode` samples), where re-deflating near-random bytes
    /// wastes CPU for ~0 size benefit.
    #[cfg(feature = "compress")]
    pub fn compressed_stream(&mut self, id: Ref, dict_extra: &str, data: &[u8]) {
        // Level 6 (zlib's own default): a reasonable ratio/speed balance
        // for a "generate once" library — no hard requirement pushed
        // this any higher, and this crate has no benchmarked need to.
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
        self.stream(id, &format!("/Filter /FlateDecode {dict_extra}"), &compressed);
    }

    /// Without the `compress` feature, `compressed_stream` is exactly
    /// `stream` — the previous, always-uncompressed behavior.
    #[cfg(not(feature = "compress"))]
    pub fn compressed_stream(&mut self, id: Ref, dict_extra: &str, data: &[u8]) {
        self.stream(id, dict_extra, data);
    }

    /// Writes the xref table, trailer and `%%EOF`, consuming the writer.
    /// Crate-internal: only [`crate::doc::PdfDocument::write`] calls this —
    /// `PdfWriter` is `PdfDocument`'s implementation detail, not part of
    /// this crate's public surface.
    pub(crate) fn finish(mut self, root: Ref, info: Option<Ref>) -> Vec<u8> {
        let xref_offset = self.buf.len();
        let count = self.next_id; // includes object 0
        self.buf.extend_from_slice(format!("xref\n0 {count}\n").as_bytes());
        self.buf.extend_from_slice(b"0000000000 65535 f \n");
        for i in 0..(count - 1) {
            // `i` is a `u32` object index; `usize` is at least 32 bits on
            // every platform this crate targets, so this widening
            // conversion never fails.
            let idx = usize::try_from(i).expect("PDF object counts fit in usize for any realistic document, see round 2 rationale");
            let offset = *self.offsets.get(idx).unwrap_or(&0);
            self.buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        let info_str = match info {
            Some(r) => format!(" /Info {}", r.write()),
            None => String::new(),
        };
        // Deterministic /ID: a hash over every object written so far
        // (everything up to, but not including, this xref/trailer), never
        // a random source — `wasm32-unknown-unknown` has none, and two
        // renders of the same `Document` must produce byte-identical PDFs.
        // Both array entries are equal, as is conventional for a document
        // written in a single revision (no prior version to diff against).
        let id = document_id_hex(&self.buf[..xref_offset]);
        self.buf.extend_from_slice(
            format!(
                "trailer\n<< /Size {count} /Root {}{info_str} /ID [<{id}> <{id}>] >>\nstartxref\n{xref_offset}\n%%EOF",
                root.write()
            )
            .as_bytes(),
        );
        self.buf
    }
}

/// 16 bytes (32 hex chars), hashed from `content` with a fixed-seed,
/// deterministic hasher (`DefaultHasher::new()` always starts from the
/// same internal state — unlike `HashMap`'s `RandomState`, it never reads
/// OS randomness). Two calls with the same `content` always agree.
fn document_id_hex(content: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h1 = DefaultHasher::new();
    content.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    1u8.hash(&mut h2);
    content.hash(&mut h2);
    format!("{:016x}{:016x}", h1.finish(), h2.finish())
}

/// Formats an `f32` with a fixed, compact precision suitable for PDF
/// content streams (avoids Rust's default float `Display`, which can emit
/// scientific notation or excessive digits).
pub fn fmt_num(v: f32) -> String {
    let rounded = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{rounded:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s.is_empty() || s == "-" {
        s = "0".to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_a_minimal_valid_structure() {
        let mut w = PdfWriter::new();
        let catalog = w.alloc();
        let pages = w.alloc();
        w.object(pages, "<< /Type /Pages /Kids [] /Count 0 >>");
        w.object(catalog, &format!("<< /Type /Catalog /Pages {} >>", pages.write()));
        let bytes = w.finish(catalog, None);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("trailer"));
        assert!(text.contains("startxref"));
        assert!(text.ends_with("%%EOF"));
    }

    #[test]
    fn fmt_num_is_compact() {
        assert_eq!(fmt_num(12.0), "12");
        assert_eq!(fmt_num(12.5), "12.5");
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(-3.14149), "-3.141");
    }
}
