// A page that renders a 40-row invoice table into a PDF, as the package's own tests do.
export default async function (lw) {
  const pdf = lw.LightweightPdf.withDefaultFonts();
  const rows = Array.from({ length: 40 }, (_, i) => [
    { element: { type: "text", content: `Item ${i + 1}` } },
    { element: { type: "text", content: `${(i + 1) * 12}.00 USD` } },
  ]);
  const document = {
    page_format: "A4",
    children: [
      { type: "text", content: "Invoice #2026-100", style: { size: 20, font: "sans-bold" } },
      {
        type: "table",
        columns: [{ width: { flex: 1 } }, { width: { fixed: 80 }, align: "end" }],
        header: [{ element: { type: "text", content: "Item" } }, { element: { type: "text", content: "Amount" } }],
        rows,
      },
    ],
  };
  const bytes = pdf.render(JSON.stringify({ schema_version: 1, document }));
  if (bytes.length === 0) throw new Error("render returned no bytes");
  pdf.free();
}
