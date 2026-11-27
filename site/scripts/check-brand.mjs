// Ban lineage branding: no page copy or asset may name an upstream project.
import {readFileSync, readdirSync, statSync} from 'node:fs';
import {join, relative, extname} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const TARGETS = ['src', 'index.html', 'DESIGN.md'];
const TEXT_EXTENSIONS = new Set([
  '.ts', '.tsx', '.js', '.jsx', '.mjs', '.css', '.json', '.md', '.html', '.svg', '.txt',
]);

const BANNED = (() => {
  // The guard ships in the published tree, so the terms it blocks are stored
  // base64-encoded: spelling them out here would leak exactly what it prevents.
  const decode = (value) => Buffer.from(value, 'base64').toString('utf8');
  const rule = (term, source) => ({ name: decode(term), pattern: new RegExp(decode(source), 'gi') });
  return [
    rule('RGVlcCBBZ2VudHM=', 'ZGVlcFtcc18tXT9hZ2VudHM='),
    rule('ZGNvZGU=', 'XGJkY29kZVxi'),
    rule('TGFuZ0dyYXBo', 'bGFuZ2dyYXBo'),
    rule('TGFuZ0NoYWlu', 'bGFuZ2NoYWlu'),
  ];
})();

function walk(path) {
  const stats = statSync(path);
  if (stats.isFile()) return [path];
  return readdirSync(path).flatMap((entry) => walk(join(path, entry)));
}

const files = TARGETS.flatMap((target) => walk(join(ROOT, target))).filter((file) =>
  TEXT_EXTENSIONS.has(extname(file).toLowerCase())
);

const hits = [];
for (const file of files) {
  const lines = readFileSync(file, 'utf8').split('\n');
  lines.forEach((line, index) => {
    for (const {name, pattern} of BANNED) {
      const matches = line.match(pattern);
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
