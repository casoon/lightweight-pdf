import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { url } from '@casoon/pages-theme/lib/url.ts';
import type { ShowcaseExample } from '@casoon/pages-theme/showcase';
import './styles/showcase.css';

// Every PDF on this site is rendered from this repository by scripts/render-site-showcase.sh –
// the demo programs in examples/ and the lwpdf CLI – which the Pages workflow runs right before
// the site build. The inputs shown next to each PDF are loaded from the same files.
const PDF_DIR = resolve(process.cwd(), 'public/pdf');
if (!existsSync(PDF_DIR)) {
  throw new Error(
    'site/public/pdf/ is missing: run scripts/render-site-showcase.sh from the repository root first.',
  );
}

const sources = import.meta.glob<string>(['../../examples/*.rs', '../../examples/*.json'], {
  query: '?raw',
  import: 'default',
  eager: true,
});

function source(file: string): string {
  const code = sources[`../../${file}`];
  if (code === undefined) throw new Error(`Unknown source: ${file}`);
  return code;
}

export interface PdfPage {
  src: string;
  width: number;
  height: number;
}

export interface PdfFile {
  href: string;
  kib: string;
  pages: PdfPage[];
}

/** A rendered PDF in public/pdf/ plus its page previews (pdftoppm names them <name>-<n>.png). */
export function pdfFile(name: string): PdfFile {
  const pageNumber = (file: string) => Number(file.slice(name.length + 1, -'.png'.length));
  const pages = readdirSync(PDF_DIR)
    .filter((file) => file.startsWith(`${name}-`) && file.endsWith('.png') && pageNumber(file) > 0)
    .sort((a, b) => pageNumber(a) - pageNumber(b))
    .map((file) => {
      const png = readFileSync(resolve(PDF_DIR, file));
      // PNG IHDR: width and height are the first two fields after the 16-byte header.
      return { src: url(`pdf/${file}`), width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
    });
  if (pages.length === 0) throw new Error(`No page previews for ${name}.pdf in ${PDF_DIR}`);
  const bytes = statSync(resolve(PDF_DIR, `${name}.pdf`)).size;
  return { href: url(`pdf/${name}.pdf`), kib: (bytes / 1024).toFixed(1), pages };
}

/** Terminal session of the lwpdf CLI, captured by the render script. */
export const cliLog = readFileSync(resolve(PDF_DIR, 'lwpdf-invoice-template.txt'), 'utf8').trimEnd();

function pdfOutput(file: PdfFile, title: string): string {
  const total = file.pages.length;
  const context = `<span class="visually-hidden">: ${title}</span>`;
  const images = file.pages
    .map(
      (page, i) =>
        `<img class="pdf-page" src="${page.src}" width="${page.width}" height="${page.height}" alt="Page ${i + 1} of ${total} of the rendered PDF" loading="lazy" decoding="async">`,
    )
    .join('');
  return (
    `<div class="pdf-output"><p class="pdf-meta">${total} ${total === 1 ? 'page' : 'pages'} · ${file.kib} KiB · ` +
    `<a href="${file.href}">Open PDF${context}</a> · <a href="${file.href}" download>Download${context}</a></p>${images}</div>`
  );
}

const catalogue = [
  {
    slug: 'invoice',
    pdf: 'demo_invoice',
    title: 'Invoice',
    description:
      'Logo image, window-envelope address block, contact box with invoice metadata, a position table with a detail line per item, a right-aligned VAT summary and a four-column footer.',
    file: 'examples/demo_invoice.rs',
    tags: ['Rust', 'Table', 'Image', 'Footer'],
  },
  {
    slug: 'quote',
    pdf: 'demo_offer',
    title: 'Quote',
    description:
      'The invoice letterhead with lump-sum positions, bullet-separated scope details under each item and labelled payment and delivery terms instead of a tax summary.',
    file: 'examples/demo_offer.rs',
    tags: ['Rust', 'Table', 'List'],
  },
  {
    slug: 'report',
    pdf: 'demo_report',
    title: 'Audit report',
    description:
      'Cover page, table of contents, running header and footer with page numbers, a scorecard table and one subsection per top issue.',
    file: 'examples/demo_report.rs',
    tags: ['Rust', 'Table of contents', 'Header', 'Footer'],
  },
  {
    slug: 'documentation',
    pdf: 'demo_documentation',
    title: 'API documentation',
    description:
      'The same cover, contents and header/footer pattern with endpoint sections, a monospace-styled request and response block and an error-code table.',
    file: 'examples/demo_documentation.rs',
    tags: ['Rust', 'Table of contents', 'Links'],
  },
  {
    slug: 'cli-template',
    pdf: 'lwpdf-invoice-template',
    title: 'Template and data with the CLI',
    description:
      'No Rust code: lwpdf render examples/invoice-template.json --data examples/invoice-data.json resolves the {{placeholders}} and the $each row repetition against the data file.',
    file: 'examples/invoice-template.json',
    tags: ['CLI', 'JSON', 'Template'],
  },
  {
    slug: 'pdf-ua',
    pdf: 'demo_pdf_ua',
    title: 'Tagged PDF / PDF/UA-1',
    description:
      'Document::pdf_ua() writes a structure tree, marked content and /Lang, marks the watermark and footer as artifacts, and requires alt text for the image. Also PDF/A-3b.',
    file: 'examples/demo_pdf_ua.rs',
    tags: ['Rust', 'tagged-pdf', 'Accessibility'],
  },
  {
    slug: 'zugferd',
    pdf: 'demo_zugferd',
    title: 'ZUGFeRD / Factur-X invoice',
    description:
      'A PDF/A-3b file with an EN 16931 invoice XML from the ZUGFeRD reference corpus embedded as factur-x.xml. Open the attachments panel of your PDF reader to see it.',
    file: 'examples/demo_zugferd.rs',
    tags: ['Rust', 'zugferd', 'PDF/A-3b'],
  },
];

function input(file: string): { code: string; lang: string } {
  if (file === 'examples/invoice-template.json') {
    return {
      lang: 'jsonc',
      code: `// examples/invoice-template.json\n${source(file)}\n// examples/invoice-data.json\n${source('examples/invoice-data.json')}`,
    };
  }
  return { lang: 'rust', code: source(file) };
}

export const examples: ShowcaseExample[] = catalogue.map(({ slug, pdf, title, description, file, tags }) => ({
  slug,
  title,
  description,
  file,
  tags,
  input: input(file),
  output: { html: pdfOutput(pdfFile(pdf), title), kind: 'panel' },
}));
