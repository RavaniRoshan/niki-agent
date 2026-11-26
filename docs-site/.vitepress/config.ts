import { defineConfig } from 'vitepress'

// Navigation and sidebar are written by hand: the site is small enough that a
// generated tree would hide the order the reader needs. Local search indexes
// the built pages; nothing here loads a font, a script, or an analytics tag
// from another origin.
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
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/mark.svg' }],
    ['meta', { name: 'theme-color', content: '#101014' }],
  ],
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
          { text: 'Architecture', link: '/architecture' },
          { text: 'Independence', link: '/independence' },
          { text: 'Security', link: '/security' },
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
