// Mirror repository Markdown into the documentation site.
//
// The repository files stay the single source of truth. This script copies the
// handful of pages the site publishes verbatim, prepends a provenance header
// naming the source, and rewrites the few links that are relative to the
// repository rather than to the site.
//
//   node scripts/sync-docs.mjs            write the copies
//   node scripts/sync-docs.mjs --check    fail if a committed copy is stale
//
// `npm run check:brand` runs the --check pass first, so CI fails when a source
// document changes and its copy was not regenerated. Edit the source file and
// run `npm run sync`; never edit a generated copy.

import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const siteDir = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const repoDir = resolve(siteDir, '..')

/** @type {{source: string, target: string}[]} */
const MIRRORS = [
  { source: 'docs/niki/PROVIDERS.md', target: 'docs/providers/index.md' },
  { source: 'docs/niki/KEYMAP.md', target: 'docs/keyboard.md' },
  { source: 'docs/niki/UI.md', target: 'docs/ui.md' },
  { source: 'docs/niki/ARCHITECTURE.md', target: 'docs/architecture.md' },
  { source: 'docs/niki/INDEPENDENCE.md', target: 'docs/independence.md' },
  { source: 'SECURITY.md', target: 'docs/security.md' },
  { source: 'CHANGELOG.md', target: 'docs/changelog.md' },
]

// Links written for a reader browsing the repository checkout.
const LINK_REWRITES = [
  [
    '](../../NOTICE)',
    '](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE)',
  ],
]

const projectUrl = 'https://github.com/RavaniRoshan/niki-agent'

function render(source, body) {
  const digest = createHash('sha256').update(body).digest('hex')
  let rewritten = body
  for (const [from, to] of LINK_REWRITES) rewritten = rewritten.split(from).join(to)
  if (!rewritten.endsWith('\n')) rewritten += '\n'
  const header = [
    '<!--',
    `  Mirrored from ${source} by docs-site/scripts/sync-docs.mjs.`,
    '  Do not edit this copy: edit the source file and run `npm run sync`.',
    `  source-sha256: ${digest}`,
    '-->',
    '',
  ].join('\n')
  const footer = [
    '',
    '---',
    '',
    `Source: [\`${source}\`](${projectUrl}/blob/niki-agent/${source}) in the Niki Agent repository.`,
    '',
  ].join('\n')
  return header + rewritten + footer
}

async function build() {
  /** @type {{target: string, contents: string}[]} */
  const files = []
  for (const { source, target } of MIRRORS) {
    const body = await readFile(join(repoDir, source), 'utf8')
    files.push({ source, target, contents: render(source, body) })
  }
  return files
}

const checkOnly = process.argv.includes('--check')
const files = await build()
const stale = []

for (const file of files) {
  const targetPath = join(siteDir, file.target)
  if (checkOnly) {
    let current = null
    try {
      current = await readFile(targetPath, 'utf8')
    } catch {
      current = null
    }
    if (current !== file.contents) stale.push(file)
    continue
  }
  await mkdir(dirname(targetPath), { recursive: true })
  await writeFile(targetPath, file.contents, 'utf8')
}

if (checkOnly) {
  if (stale.length === 0) {
    console.log(`[mirror] all ${files.length} copies match their sources`)
    process.exit(0)
  }
  console.error('[mirror] stale copies — the source changed and the copy did not:')
  for (const file of stale) {
    console.error(`  ${relative(siteDir, join(siteDir, file.target))} <- ${file.source}`)
  }
  console.error('[mirror] run `npm run sync` and commit the result')
  process.exit(1)
}

console.log(`[mirror] wrote ${files.length} copies from ${relative(siteDir, repoDir)}/`)
