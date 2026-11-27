// Check that the branch-name rules stay in sync across their three copies.
//
// The branch naming convention `<github-username>/<scope>/<short-description>` is
// enforced in three places that cannot share code:
//
// - `.githooks/pre-push` — the local pre-push hook (bash)
// - `.github/workflows/branch_name_check.yml` — the advisory CI check (bash in YAML)
// - `.github/workflows/pr_lint.yml` — the PR *title* scope list, which the branch
//   scope list mirrors (plus `docs`, which AGENTS.md lists as a branch scope)
//
// Drift has an asymmetric cost: a scope added to `pr_lint.yml` alone means the
// local hook hard-blocks a branch CI considers valid, and reports the stale scope
// set as authoritative. The CI check is advisory, so it produces no red signal.
// This check makes that drift a commit-time failure instead.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../..',
);

const HOOK = '.githooks/pre-push';
const BRANCH_CHECK = '.github/workflows/branch_name_check.yml';
const PR_LINT = '.github/workflows/pr_lint.yml';

// Scopes valid in a branch name but not in a PR title: AGENTS.md lists `docs` as
// a branch scope, while `pr_lint.yml` carries it as a Conventional Commits
// *type*. Keep this the only permitted delta between the two lists.
const BRANCH_ONLY_SCOPES = new Set(['docs']);

// The three shell variables the two branch-name checks must agree on verbatim.
const SHARED_VARS = ['ALLOWED_RE', 'ALLOWED_PREFIX_RE', 'SCOPES_RE'];

const SCRIPT_NAME = path.basename(fileURLToPath(import.meta.url));

// Return the text of a repo-relative path, or exit with a clear error.
function read(relPath) {
  try {
    return fs.readFileSync(path.join(REPO_ROOT, relPath), 'utf8');
  } catch (error) {
    console.error(`error: could not read ${relPath}: ${error.message}`);
    process.exit(1);
  }
}

// Return the single-quoted value assigned to `variable` in a shell snippet.
//
// Requires exactly one assignment so a second, shadowing copy is a failure
// rather than a silently ignored one.
function shellAssignment(text, file, variable) {
  const matches = [
    ...text.matchAll(new RegExp(`^\\s*${variable}='([^']*)'\\s*$`, 'gm')),
  ];
  if (matches.length !== 1) {
    console.error(
      `error: expected exactly one single-quoted \`${variable}=...\` assignment in ` +
        `${file}, found ${matches.length}. If the assignment was reformatted, ` +
        `update ${SCRIPT_NAME} to match.`,
    );
    process.exit(1);
  }
  return matches[0][1];
}

// Return the alternatives from a `(a|b|c)` scope group.
function scopesFromRe(pattern) {
  if (!(pattern.startsWith('(') && pattern.endsWith(')'))) {
    console.error(`error: SCOPES_RE is not a parenthesized group: '${pattern}'`);
    process.exit(1);
  }
  return pattern.slice(1, -1).split('|');
}

// Return the `scopes:` block entries from `pr_lint.yml`.
//
// Parsed with a regex rather than a YAML loader to keep this check
// dependency-free (it runs under `language: system`).
function prLintScopes() {
  const text = read(PR_LINT);
  const match = /^([ ]*)scopes: \|\n((?:.*\n)*?)(?=\1\S)/m.exec(text);
  if (match === null) {
    console.error(
      `error: could not find a \`scopes: |\` block in ${PR_LINT}. If the ` +
        `block was restructured, update ${SCRIPT_NAME} to match.`,
    );
    process.exit(1);
  }
  const scopes = match[2]
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
  if (scopes.length === 0) {
    console.error(`error: the \`scopes: |\` block in ${PR_LINT} is empty.`);
    process.exit(1);
  }
  return scopes;
}

function sorted(values) {
  return [...values].sort();
}

// Compare the three rule sets and report every mismatch found.
function main() {
  const hookText = read(HOOK);
  const ciText = read(BRANCH_CHECK);
  const errors = [];

  // 1. The two branch-name checks must share all three patterns byte for byte.
  for (const variable of SHARED_VARS) {
    const hookValue = shellAssignment(hookText, HOOK, variable);
    const ciValue = shellAssignment(ciText, BRANCH_CHECK, variable);
    if (hookValue !== ciValue) {
      errors.push(
        `${variable} differs between the local hook and the CI check:\n` +
          `  ${HOOK}: ${hookValue}\n` +
          `  ${BRANCH_CHECK}: ${ciValue}`,
      );
    }
  }

  // 2. The branch scope list must be the PR-title scope list plus `docs`.
  const branchScopes = new Set(
    scopesFromRe(shellAssignment(hookText, HOOK, 'SCOPES_RE')),
  );
  const expected = new Set([...prLintScopes(), ...BRANCH_ONLY_SCOPES]);
  const actual = branchScopes;

  const missing = sorted(
    [...expected].filter((scope) => !actual.has(scope)),
  );
  if (missing.length > 0) {
    const branchOnly = sorted(BRANCH_ONLY_SCOPES)
      .map((scope) => `'${scope}'`)
      .join(', ');
    errors.push(
      `scopes in ${PR_LINT} (plus [${branchOnly}]) but ` +
        `missing from SCOPES_RE: ${missing.join(', ')}`,
    );
  }
  const extra = sorted([...actual].filter((scope) => !expected.has(scope)));
  if (extra.length > 0) {
    errors.push(
      `scopes in SCOPES_RE but not in ${PR_LINT}: ${extra.join(', ')}. Add ` +
        `them to pr_lint.yml, or to BRANCH_ONLY_SCOPES if they are ` +
        `deliberately branch-only.`,
    );
  }

  if (errors.length > 0) {
    let message = 'Branch-name rules are out of sync:\n\n';
    for (const error of errors) {
      message += `  - ${error}\n\n`;
    }
    process.stderr.write(message);
    return 1;
  }
  return 0;
}

process.exitCode = main();
