// Flag PRs whose package title scopes do not cover touched package dirs.
//
// The PR labeler config already defines both sides of this relationship:
// `scopeToLabel` maps conventional-commit scopes to package labels, and
// `fileRules` maps package directories to the same labels. This helper reads that
// config directly so the CI gate cannot drift from the labeler.
//
// On successful analysis the script reports offenders on stdout and exits 0; the
// workflow that calls it decides whether to fail, bypass, or comment. If the
// labeler config cannot be read or validated, the script exits 2 so CI fails
// closed.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../..',
);

export const DEFAULT_CONFIG = path.join(
  REPO_ROOT,
  '.github',
  'scripts',
  'labeling',
  'pr-labeler-config.json',
);
export const DEFAULT_RELEASE_CONFIG = path.join(
  REPO_ROOT,
  'release-please-config.json',
);

const TITLE_RE = /^[a-z]+(?:\(([^)]*)\))?!?:\s/;
const RELEASE_TITLE_RE = /^release\([^)]*\):\s/;

const PACKAGE_PREFIXES = new Set(['libs', 'rust', 'install']);

// Distinguishes config read/validation failures (which `main` turns into exit
// code 2) from programming errors, which should propagate like a traceback.
class ConfigError extends Error {}

function isPlainObject(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function joinPackagePath(root, entry) {
  return path.isAbsolute(entry) ? entry : path.normalize(path.join(root, entry));
}

const releaseFilesCache = new Map();

// Return release-please managed artifact paths.
//
// Cached per path so a single run does not re-read and re-parse the config once
// per changed file. The config is immutable within a process, and thrown
// `ConfigError`s are not cached, so a malformed config keeps failing closed on
// every call.
export function releaseFiles(releaseConfigPath = DEFAULT_RELEASE_CONFIG) {
  const key = path.resolve(releaseConfigPath);
  if (releaseFilesCache.has(key)) {
    return releaseFilesCache.get(key);
  }

  let config;
  try {
    config = JSON.parse(fs.readFileSync(releaseConfigPath, 'utf8'));
  } catch (error) {
    throw new ConfigError(
      `could not read release-please config ${releaseConfigPath}: ${error.message}`,
    );
  }

  const packages = config?.packages;
  if (!isPlainObject(packages)) {
    throw new ConfigError("release-please config has no 'packages' map");
  }

  const files = new Set(['.release-please-manifest.json']);
  for (const [root, pkg] of Object.entries(packages)) {
    if (!isPlainObject(pkg)) {
      throw new ConfigError(
        `release-please package entry is malformed: '${root}': ${JSON.stringify(pkg)}`,
      );
    }
    const changelog = pkg['changelog-path'];
    if (typeof changelog === 'string') {
      files.add(joinPackagePath(root, changelog));
    }
    const extraFiles = Object.hasOwn(pkg, 'extra-files') ? pkg['extra-files'] : [];
    if (!Array.isArray(extraFiles)) {
      throw new ConfigError(
        `release-please package '${root}' has malformed 'extra-files'`,
      );
    }
    for (const extra of extraFiles) {
      if (typeof extra !== 'string') {
        throw new ConfigError(
          `release-please package '${root}' has non-string extra file`,
        );
      }
      files.add(joinPackagePath(root, extra));
    }
  }
  releaseFilesCache.set(key, files);
  return files;
}

// Return whether `file` is a file release-please may update in a release PR.
//
// Covers the version-of-record manifest and the configured per-package
// changelog and extra files.
export function isReleaseFile(
  file,
  { releaseConfigPath = DEFAULT_RELEASE_CONFIG } = {},
) {
  return releaseFiles(releaseConfigPath).has(file);
}

// Return whether `title` is a release PR title.
export function isReleaseTitle(title) {
  return RELEASE_TITLE_RE.test(title);
}

// Return whether the PR looks like a release-please artifact update.
//
// The title is author-controlled (PR event payload) and the release component
// is not validated against real package names, so a non-release PR can adopt a
// `release(...)` title. Safety therefore rests entirely on the file allowlist:
// the bypass only applies when *every* changed file matches `isReleaseFile`, so
// a single source file re-arms the scope gate.
export function isReleasePrChange(
  title,
  changed,
  { releaseConfigPath = DEFAULT_RELEASE_CONFIG } = {},
) {
  if (!(changed.length > 0 && isReleaseTitle(title))) {
    return false;
  }
  // Read (and thereby validate) the release config up front so a malformed
  // `release-please-config.json` fails closed even when the file list looks
  // like a release artifact. `releaseFiles` is cached, so the per-file checks
  // below reuse this result.
  releaseFiles(releaseConfigPath);
  return changed.every((file) => isReleaseFile(file, { releaseConfigPath }));
}

// Return conventional-commit scopes parsed from `title`.
//
// Empty when the title is not conventional-commit shaped or has no scope.
export function parseTitleScopes(title) {
  const match = TITLE_RE.exec(title);
  if (!match || !match[1]) {
    return [];
  }
  return match[1]
    .split(',')
    .map((scope) => scope.trim())
    .filter(Boolean);
}

// Return package directory rules from the PR labeler config.
function packageRules(config) {
  const rules = config?.fileRules;
  if (!Array.isArray(rules) || rules.length === 0) {
    throw new ConfigError("pr-labeler config has no non-empty 'fileRules' list");
  }

  const packageRuleList = [];
  for (const rule of rules) {
    if (!isPlainObject(rule)) {
      throw new ConfigError(
        `pr-labeler fileRules entry is not an object: ${JSON.stringify(rule)}`,
      );
    }
    const label = rule.label;
    const prefix = rule.prefix;
    // Rules without a 'prefix' (suffix/exact/pattern rules) are not package
    // directory rules and are legitimately skipped.
    if (prefix === undefined || prefix === null) {
      continue;
    }
    // A rule that *has* a 'prefix' but with a malformed type is config
    // corruption, not a non-package rule. Fail closed rather than silently
    // dropping it: a single dropped package rule would let a real
    // scope/file mismatch pass unnoticed (the gate's worst outcome).
    if (typeof label !== 'string' || typeof prefix !== 'string') {
      throw new ConfigError(
        `pr-labeler fileRules entry has non-string label/prefix: ${JSON.stringify(rule)}`,
      );
    }
    // Package code lives under `libs/`, `rust/` and `install/`; any other
    // prefix (`.github/`, `docs/`) is a legitimate non-package rule.
    if (!PACKAGE_PREFIXES.has(prefix.split('/', 1)[0])) {
      continue;
    }
    packageRuleList.push({
      label,
      prefix: `${prefix.replace(/\/+$/, '')}/`,
    });
  }

  if (packageRuleList.length === 0) {
    throw new ConfigError('pr-labeler config has no package directory fileRules');
  }
  return packageRuleList.sort((a, b) => (a.prefix < b.prefix ? -1 : a.prefix > b.prefix ? 1 : 0));
}

// Return scope names whose label points at a package label.
function scopePackages(config, packageLabels) {
  const scopeToLabel = config?.scopeToLabel;
  if (!isPlainObject(scopeToLabel) || Object.keys(scopeToLabel).length === 0) {
    throw new ConfigError("pr-labeler config has no non-empty 'scopeToLabel' map");
  }
  return new Set(
    Object.entries(scopeToLabel)
      .filter(([, label]) => typeof label === 'string' && packageLabels.has(label))
      .map(([scope]) => scope),
  );
}

// Return package labels declared by the PR title scopes.
export function declaredPackages(title, config) {
  const rules = packageRules(config);
  const packageLabels = new Set(rules.map((rule) => rule.label));
  const scopes = scopePackages(config, packageLabels);
  const scopeToLabel = config.scopeToLabel;
  const declared = new Set();
  for (const scope of parseTitleScopes(title)) {
    if (scopes.has(scope)) {
      declared.add(scopeToLabel[scope]);
    }
  }
  return declared;
}

// Return package labels and dirs touched by changed files.
export function changedPackages(changed, config) {
  const packages = new Map();
  for (const rule of packageRules(config)) {
    const prefix = rule.prefix;
    const packageDir = prefix.replace(/\/+$/, '');
    for (const file of changed) {
      if (file === packageDir || file.startsWith(prefix)) {
        if (!packages.has(rule.label)) {
          packages.set(rule.label, new Set());
        }
        packages.get(rule.label).add(prefix);
        break;
      }
    }
  }

  const result = {};
  for (const label of [...packages.keys()].sort()) {
    result[label] = [...packages.get(label)].sort();
  }
  return result;
}

// Return touched package dirs not covered by package scopes in `title`.
//
// Empty when the title declares no package scopes, when no package dirs are
// touched (for release PRs, after release-managed artifacts are filtered out),
// or when every touched package is covered by a declared scope.
export function findOffenders(
  title,
  changed,
  config,
  { releaseConfigPath = DEFAULT_RELEASE_CONFIG } = {},
) {
  // Computed before the release filtering so a malformed labeler config still
  // fails closed (throws) on release PRs rather than passing unchecked.
  const declared = declaredPackages(title, config);
  let files = changed;
  if (isReleaseTitle(title)) {
    // Validate the release config even when no package offenders remain. Then
    // ignore release-managed artifacts for package-dir matching: release PRs
    // often touch generated files in packages unrelated to the release
    // component, but ordinary source edits must still be checked.
    releaseFiles(releaseConfigPath);
    files = files.filter(
      (file) => !isReleaseFile(file, { releaseConfigPath }),
    );
  }

  if (files.length === 0 || declared.size === 0) {
    return [];
  }

  return Object.entries(changedPackages(files, config))
    .filter(([pkg]) => !declared.has(pkg))
    .map(([pkg, dirs]) => ({ package: pkg, dirs }));
}

// Print offending packages as a JSON array to stdout.
//
// Returns `0` after successful analysis. Offenders are reported on stdout and
// the workflow makes the blocking decision. Returns `2` when the config cannot
// be read or validated, so CI fails closed.
export function main(
  title,
  changed,
  configPath = DEFAULT_CONFIG,
  releaseConfigPath = DEFAULT_RELEASE_CONFIG,
) {
  let offenders;
  let bypassed;
  try {
    // Wrap the labeler-config read so its errors name the labeler file. The
    // outer handler also catches config errors raised while reading the
    // release config — by findOffenders (via `releaseFiles`) and by the
    // isReleasePrChange call below — whose messages name
    // release-please-config.json themselves, so the generic prefix below does
    // not mis-attribute one config's failure to the other.
    let config;
    try {
      config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    } catch (error) {
      throw new ConfigError(
        `could not read PR labeler config ${configPath}: ${error.message}`,
      );
    }
    offenders = findOffenders(title, changed, config, { releaseConfigPath });
    bypassed = isReleasePrChange(title, changed, { releaseConfigPath });
  } catch (error) {
    if (!(error instanceof ConfigError)) {
      throw error;
    }
    process.stderr.write(
      `::error::PR scope/file check failed to read or validate config: ${error.message}\n`,
    );
    return 2;
  }

  // Surface the release bypass so "gate stood down" is distinguishable from
  // "genuinely clean" in the Checks UI, mirroring the workflow's
  // detector-absent ::warning::. To stderr so it never corrupts the JSON
  // offenders the workflow captures from stdout. ::notice:: (not ::warning::)
  // because this is a designed, expected bypass.
  if (bypassed) {
    process.stderr.write(
      '::notice::Release-shaped title; scope/file gate bypassed because ' +
        'every changed file matched the release-please artifact allowlist.\n',
    );
  }

  if (offenders.length > 0) {
    const summary = offenders
      .map((offender) => `${offender.package} (${offender.dirs.join(', ')})`)
      .join(', ');
    process.stderr.write(
      `PR title scope does not cover touched package dirs: ${summary}\n`,
    );
  }
  process.stdout.write(`${JSON.stringify(offenders)}\n`);
  return 0;
}

const isMain =
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;

if (isMain) {
  if (process.argv.length < 3) {
    process.stderr.write(
      'usage: check-pr-scope-files.mjs <pr-title>  (changed files on stdin)\n',
    );
    process.exit(2);
  }
  const prTitle = process.argv[2];
  const changedFiles = fs
    .readFileSync(0, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
  process.exit(main(prTitle, changedFiles));
}
