// Brand gate for the documentation site.
//
// Fails when the site — its Markdown sources, its VitePress config and theme,
// or a built `dist/` — names another agent project or claims an affiliation.
//
//   1. Mirror freshness: every copied page must match its repository source.
//   2. Sources: docs/**/*.md, .vitepress/config.ts, and .vitepress/theme/**.
//   3. Build output: .vitepress/dist/**, when a build exists.
//
// NOTICE, LICENSE, and PROVENANCE files keep their upstream text. They are not
// served by this site, and they are skipped here so attribution is never
// mistaken for branding.
//
// Every hit is reported as `path:line:column — term` with the offending line.

import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { readdir, readFile } from 'node:fs/promises'
import { dirname, extname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const siteDir = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const repoDir = resolve(siteDir, '..')

const RULES = [
  // Internal names from before the rename. Word boundaries keep `hardcoded`
  // from matching `dcode`.
  { term: 'deepagents', pattern: /\bdeepagents\b/gi },
  { term: 'Deep Agents', pattern: /\bdeep\s+agents?\b/gi },
  { term: 'dcode', pattern: /\bdcode\b/gi },
  // The graph runtime and its vendor.
  { term: 'LangGraph', pattern: /\blanggraph\b/gi },
  // Underscored package ids (`langchain_openai`) stay allowed: the underscore
  // is a word character, so this matches the framework name only as a word.
  { term: 'LangChain', pattern: /\blangchain\b/gi },
  { term: 'langchain-ai', pattern: /langchain-ai/gi },
  { term: 'docs.langchain.com', pattern: /docs\.langchain\.com/gi },
  // A claim that Niki Agent is affiliated with, endorsed by, or built by
  // another project. Bare "affiliation" is excluded: the default theme ships a
  // `.affiliation` CSS class for team cards, which claims nothing.
  {
    term: 'claim of affiliation',
    pattern:
      /\b(?:affiliated with|official (?:partner|product|documentation)|in partnership with|partnered with|sponsored by|endorsed by|powered by|from the makers of)\b/gi,
  },
]

const SOURCE_ROOTS = ['docs']
const SOURCE_FILES = ['.vitepress/config.ts']
const THEME_ROOT = '.vitepress/theme'
const DIST_ROOT = '.vitepress/dist'
const TEXT_EXTENSIONS = new Set([
  '.md',
  '.html',
  '.js',
  '.mjs',
  '.css',
  '.json',
  '.map',
  '.txt',
  '.xml',
  '.svg',
])
const SKIP_NAMES = new Set(['NOTICE', 'LICENSE', 'PROVENANCE.md'])
const SKIP_DIRS = new Set(['node_modules', '.git', 'cache'])

async function walk(root) {
  if (!existsSync(root)) return []
  const out = []
  const entries = await readdir(root, { withFileTypes: true })
  for (const entry of entries) {
    if (entry.name.startsWith('.') && entry.name !== '.vitepress') continue
    const path = join(root, entry.name)
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue
      out.push(...(await walk(path)))
    } else if (entry.isFile()) {
      out.push(path)
    }
  }
  return out
}

function isScannable(path) {
  return TEXT_EXTENSIONS.has(extname(path))
}

function scan(text, displayPath, hits) {
  const lines = text.split('\n')
  lines.forEach((line, index) => {
    for (const rule of RULES) {
      rule.pattern.lastIndex = 0
      let match
      while ((match = rule.pattern.exec(line)) !== null) {
        hits.push({
          file: displayPath,
          line: index + 1,
          column: match.index + 1,
          term: rule.term,
          context: line.trim().slice(0, 160),
        })
      }
    }
  })
}

const hits = []

// 1. Mirrored copies must be current.
try {
  execFileSync(process.execPath, [join(siteDir, 'scripts/sync-docs.mjs'), '--check'], {
    stdio: 'inherit',
  })
} catch {
  console.error('\n[brand] mirrored pages are stale (see above)')
  process.exit(1)
}

// 2. Site sources.
const files = []
for (const root of SOURCE_ROOTS) files.push(...(await walk(join(siteDir, root))))
for (const file of SOURCE_FILES) files.push(join(siteDir, file))
files.push(...(await walk(join(siteDir, THEME_ROOT, 'components'))))
files.push(...(await walk(join(siteDir, THEME_ROOT, 'styles'))))
for (const name of ['index.ts', 'custom.css']) {
  const path = join(siteDir, THEME_ROOT, name)
  if (existsSync(path)) files.push(path)
}

for (const path of files) {
  if (!isScannable(path) || SKIP_NAMES.has(path.split('/').pop())) continue
  scan(await readFile(path, 'utf8'), relative(repoDir, path), hits)
}

// 3. Build output, when present.
for (const path of await walk(join(siteDir, DIST_ROOT))) {
  if (!isScannable(path) || SKIP_NAMES.has(path.split('/').pop())) continue
  scan(await readFile(path, 'utf8'), relative(repoDir, path), hits)
}

if (hits.length > 0) {
  for (const hit of hits) {
    console.error(`${hit.file}:${hit.line}:${hit.column} — ${hit.term}`)
    console.error(`  ${hit.context}`)
  }
  console.error(`\n[brand] ${hits.length} hit(s) across ${new Set(hits.map((h) => h.file)).size} file(s)`)
  process.exit(1)
}

const scanned = new Set([...files, ...(await walk(join(siteDir, DIST_ROOT)))]).size
console.log(`[brand] clean: ${RULES.length} rules over ${scanned} file(s)`)
