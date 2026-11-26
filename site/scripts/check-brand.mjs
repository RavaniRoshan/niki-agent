// Ban lineage branding. Exception: `langchain*` is allowed only inside a `class_path` value
// (e.g. `class_path = "langchain_openai:ChatOpenAI"`), a literal config.toml value the product requires.
import {readFileSync, readdirSync, statSync} from 'node:fs';
import {join, relative, extname} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const TARGETS = ['src', 'index.html', 'DESIGN.md'];
const TEXT_EXTENSIONS = new Set([
  '.ts', '.tsx', '.js', '.jsx', '.mjs', '.css', '.json', '.md', '.html', '.svg', '.txt',
]);

const BANNED = [
  {name: 'Deep Agents', pattern: /deep[\s_-]?agents/gi},
  {name: 'dcode', pattern: /\bdcode\b/gi},
  {name: 'LangGraph', pattern: /langgraph/gi},
  {name: 'LangChain', pattern: /langchain/gi},
];

const CLASS_PATH_SPAN = /class_path\s*[:=]\s*["'][^"']*["']/g;
const CLASS_PATH_VALUE = /langchain_[a-z0-9_]+:[A-Za-z0-9_]+/g;

function walk(path) {
  const stats = statSync(path);
  if (stats.isFile()) return [path];
  return readdirSync(path).flatMap((entry) => walk(join(path, entry)));
}

function maskClassPaths(line) {
  return line
    .replace(CLASS_PATH_SPAN, (span) => span.replace(/[^\s]/g, ' '))
    .replace(CLASS_PATH_VALUE, (span) => span.replace(/[^\s]/g, ' '));
}

const files = TARGETS.flatMap((target) => walk(join(ROOT, target))).filter((file) =>
  TEXT_EXTENSIONS.has(extname(file).toLowerCase())
);

const hits = [];
for (const file of files) {
  const lines = readFileSync(file, 'utf8').split('\n');
  lines.forEach((line, index) => {
    const inspected = maskClassPaths(line);
    for (const {name, pattern} of BANNED) {
      const matches = inspected.match(pattern);
      if (matches) {
        hits.push({
          location: `${relative(ROOT, file)}:${index + 1}`,
          name,
          count: matches.length,
          line: line.trim(),
        });
      }
    }
  });
}

if (hits.length === 0) {
  console.log(`check:brand: no banned product names in ${TARGETS.join(', ')} (${files.length} files).`);
  process.exit(0);
}

for (const hit of hits) {
  console.log(`${hit.location}: ${hit.name} — ${hit.line}`);
}
console.log(
  `\ncheck:brand: ${hits.length} banned occurrence(s) in ${new Set(hits.map((h) => h.location.split(':')[0])).size} files.`
);
process.exit(1);
