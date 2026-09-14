// @ts-check
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

// Project page: https://casoon.github.io/lightweight-pdf/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/lightweight-pdf',
  base: '/lightweight-pdf/',
  integrations: [
    casoonPages({
      name: 'lightweight-pdf',
      description:
        'Document-oriented PDF generation in pure Rust for invoices, quotes and reports – natively or as wasm32 in a Cloudflare Worker.',
      repo: 'casoon/lightweight-pdf',
      version: '0.3.0',
      license: 'MIT',
      branch: 'master',
      packages: [
        { label: 'crates.io', href: 'https://crates.io/crates/lightweight-pdf' },
        { label: 'docs.rs', href: 'https://docs.rs/lightweight-pdf' },
      ],
      docsGroups: {
        'getting-started': 'Getting started',
        guides: 'Guides',
        reference: 'Reference',
        adr: 'Architecture Decisions (ADRs)',
      },
    }),
  ],
});
