import { defineConfig } from 'vitepress'

// Navigation and sidebar are written by hand: the site is small enough that a
// generated tree would hide the order the reader needs. Local search indexes
// the built pages; the only third-party load is Vercel's same-origin insights
// script, and no font is fetched.
const HOST = 'https://niki-docs.vercel.app'
const HOST_SITE = 'https://niki-site.vercel.app'

/** Canonical URL for a page, with the trailing slash only on the root. */
function canonical(relativePath: string): string {
  const page = relativePath.replace(/(index)?\.md$/, '').replace(/\/$/, '')
  return page === '' ? `${HOST}/` : `${HOST}/${page}`
}

export default defineConfig({
  lang: 'en-US',
  title: 'Niki Agent',
  titleTemplate: ':title — Niki Agent',
  description:
    'Documentation for Niki Agent, a Rust terminal coding agent that runs on your machine with the model you already use.',
  srcDir: './docs',
  cleanUrls: true,
  appearance: true,
  lastUpdated: false,
  sitemap: { hostname: HOST },
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/mark.svg' }],
    ['meta', { name: 'theme-color', content: '#101014' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'Niki Agent' }],
    ['meta', { property: 'og:locale', content: 'en_US' }],
    ['meta', { property: 'og:image', content: `${HOST_SITE}/og.png` }],
    ['meta', { property: 'og:image:width', content: '1200' }],
    ['meta', { property: 'og:image:height', content: '630' }],
    [
      'meta',
      {
        property: 'og:image:alt',
        content: 'Niki Agent: a quiet terminal coding agent in one native Rust binary.',
      },
    ],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:image', content: `${HOST_SITE}/og.png` }],
    ['link', { rel: 'apple-touch-icon', href: '/mark.svg' }],
    [
      'script',
      {},
      "window.va=window.va||function(){(window.vaq=window.vaq||[]).push(arguments)};",
    ],
    ['script', { defer: '', src: '/_vercel/insights/script.js' }],
  ],
  transformHead({ pageData }) {
    return [['link', { rel: 'canonical', href: canonical(pageData.relativePath) }]]
  },
  markdown: {
    theme: { light: 'github-light', dark: 'github-dark' },
  },
  themeConfig: {
    logo: '/mark.svg',
    siteTitle: 'Niki Agent',
    nav: [
      { text: 'Getting started', link: '/getting-started/', activeMatch: '^/getting-started/' },
      { text: 'Providers', link: '/providers/', activeMatch: '^/providers/' },
      { text: 'Extensions', link: '/extensions/', activeMatch: '^/extensions/' },
      { text: 'Security', link: '/security', activeMatch: '^/security' },
      { text: 'Troubleshooting', link: '/troubleshooting' },
      { text: 'Download', link: 'https://niki-site.vercel.app/download' },
    ],
    sidebar: [
      {
        text: 'Getting started',
        items: [
          { text: 'Overview', link: '/getting-started/' },
          { text: 'Install', link: '/getting-started/install' },
          { text: 'Your first turn', link: '/getting-started/first-turn' },
        ],
      },
      {
        text: 'Using Niki',
        items: [
          { text: 'Providers', link: '/providers/' },
          { text: 'Permissions', link: '/permissions' },
          { text: 'Sessions', link: '/sessions' },
          { text: 'Headless and scripted runs', link: '/headless' },
          { text: 'Keyboard reference', link: '/keyboard' },
        ],
      },
      {
        text: 'Extending Niki',
        items: [
          { text: 'Extensions', link: '/extensions/' },
          { text: 'Hooks', link: '/extensions/hooks' },
          { text: 'Skills', link: '/extensions/skills' },
        ],
      },
      {
        text: 'Understanding Niki',
        items: [
          { text: 'Interface', link: '/ui' },
          { text: 'Architecture', link: '/architecture' },
          { text: 'Independence', link: '/independence' },
          { text: 'Security', link: '/security' },
        ],
      },
      {
        text: 'Project',
        items: [
          { text: 'Beta builds', link: '/beta' },
          { text: 'Changelog', link: '/changelog' },
          { text: 'Download', link: 'https://niki-site.vercel.app/download' },
          { text: 'Website', link: 'https://niki-site.vercel.app' },
        ],
      },
      {
        text: 'Help',
        items: [{ text: 'Troubleshooting', link: '/troubleshooting' }],
      },
    ],
    search: {
      provider: 'local',
      options: {
        detailedView: true,
      },
    },
    outline: { level: [2, 3], label: 'On this page' },
    docFooter: { prev: 'Previous', next: 'Next' },
    darkModeSwitchLabel: 'Appearance',
    lightModeSwitchTitle: 'Switch to light mode',
    darkModeSwitchTitle: 'Switch to dark mode',
    sidebarMenuLabel: 'Menu',
    returnToTopLabel: 'Return to top',
    externalLinkIcon: true,
    footer: {
      message: 'Released under the MIT licence. Third-party notices are kept in NOTICE.',
      copyright: 'Niki Agent',
    },
  },
})
