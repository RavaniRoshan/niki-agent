// Per-route HTML for the single-page app.
//
// Vercel serves a static file before it applies the SPA rewrite, so writing
// `dist/download/index.html` and `dist/changelog/index.html` gives each route
// its own title, description, canonical and social card without a framework.
// The head is rebuilt from the markers `index.html` already carries.

import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');
const DIST = path.join(ROOT, 'dist');
const ORIGIN = 'https://niki-site.vercel.app';
const OG_IMAGE = `${ORIGIN}/og.png`;
// `rust/Cargo.toml` is authoritative, but Vercel builds this project with only
// `site/` on disk, so the committed constant is the fallback.
const VERSION = (() => {
  const manifest = path.join(ROOT, '..', 'rust', 'Cargo.toml');
  if (fs.existsSync(manifest)) {
    const match = fs.readFileSync(manifest, 'utf8').match(/^version = "([^"]+)"/m);
    if (match) return match[1];
  }
  const constant = fs
    .readFileSync(path.join(ROOT, 'src', 'shared', 'version.ts'), 'utf8')
    .match(/VERSION = '([^']+)'/);
  if (!constant) throw new Error('prerender cannot find a version');
  return constant[1];
})();

/** Title, description, and path for each route the app answers. */
const ROUTES = [
  {
    path: '/',
    title: 'Niki Agent — a terminal coding agent that stays quiet',
    description:
      'Niki Agent reads your repository, edits files, and runs commands with an approval gate in front of every change. One native binary, eleven providers, sessions on your disk, no account, no telemetry.',
    changefreq: 'weekly',
    priority: '1.0',
  },
  {
    path: '/download',
    title: 'Download Niki Agent — one binary, no runtime',
    description:
      'Download Niki Agent for Linux, macOS, and Windows. One archive per platform, a one-line installer, and a SHA-256 checksum for every build.',
    changefreq: 'weekly',
    priority: '0.9',
  },
  {
    path: '/changelog',
    title: 'Niki Agent changelog — every release, in order',
    description:
      'Every Niki Agent release, newest first, with what changed in each one. Generated from the repository changelog.',
    changefreq: 'weekly',
    priority: '0.7',
  },
];

const escape = (value) =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');

function head(route) {
  const url = route.path === '/' ? `${ORIGIN}/` : `${ORIGIN}${route.path}`;
  const title = escape(route.title);
  const description = escape(route.description);
  const lines = [
    `<title>${title}</title>`,
    `<meta name="description" content="${description}" />`,
    `<link rel="canonical" href="${url}" />`,
    `<meta name="robots" content="index, follow, max-image-preview:large" />`,
    `<meta property="og:title" content="${title}" />`,
    `<meta property="og:description" content="${description}" />`,
    `<meta property="og:type" content="website" />`,
    `<meta property="og:site_name" content="Niki Agent" />`,
    `<meta property="og:locale" content="en_US" />`,
    `<meta property="og:url" content="${url}" />`,
    `<meta property="og:image" content="${OG_IMAGE}" />`,
    `<meta property="og:image:width" content="1200" />`,
    `<meta property="og:image:height" content="630" />`,
    `<meta property="og:image:alt" content="Niki Agent: a quiet terminal coding agent in one native Rust binary." />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
    `<meta name="twitter:title" content="${title}" />`,
    `<meta name="twitter:description" content="${description}" />`,
    `<meta name="twitter:image" content="${OG_IMAGE}" />`,
    `<link rel="icon" type="image/svg+xml" href="/mark.svg" />`,
    `<link rel="apple-touch-icon" href="/mark.svg" />`,
    `<meta name="theme-color" content="#0f1014" />`,
  ];
  if (route.path === '/') {
    lines.push(
      `<script type="application/ld+json">${JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        name: 'Niki Agent',
        applicationCategory: 'DeveloperApplication',
        operatingSystem: 'Linux, macOS, Windows',
        description:
          'A terminal coding agent in one native Rust binary, with an approval gate in front of every file write and shell command.',
        url: `${ORIGIN}/`,
        downloadUrl: `${ORIGIN}/download`,
        softwareVersion: VERSION,
        license: 'https://opensource.org/license/mit',
        offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
        sameAs: ['https://github.com/RavaniRoshan/niki-agent'],
      })}</script>`,
    );
  }
  return lines.map((line) => `    ${line}`).join('\n');
}

const shell = fs.readFileSync(path.join(DIST, 'index.html'), 'utf8');
const START = '<!-- seo:start -->';
const END = '<!-- seo:end -->';
if (!shell.includes(START) || !shell.includes(END)) {
  throw new Error('dist/index.html lost its seo markers; prerender cannot build routes');
}

const before = shell.slice(0, shell.indexOf(START) + START.length);
const after = shell.slice(shell.indexOf(END));

for (const route of ROUTES) {
  const html = `${before}\n${head(route)}\n    ${after}`;
  const target =
    route.path === '/'
      ? path.join(DIST, 'index.html')
      : path.join(DIST, route.path.slice(1), 'index.html');
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, html);
}

const sitemap = [
  '<?xml version="1.0" encoding="UTF-8"?>',
  '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
  ...ROUTES.map(
    (route) =>
      `  <url>\n    <loc>${route.path === '/' ? `${ORIGIN}/` : `${ORIGIN}${route.path}`}</loc>\n    <changefreq>${route.changefreq}</changefreq>\n    <priority>${route.priority}</priority>\n  </url>`,
  ),
  '</urlset>',
  '',
].join('\n');
fs.writeFileSync(path.join(DIST, 'sitemap.xml'), sitemap);

console.log(`[prerender] wrote ${ROUTES.length} route heads and sitemap.xml`);
