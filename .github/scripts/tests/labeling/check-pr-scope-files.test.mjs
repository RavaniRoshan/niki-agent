import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test, { after } from 'node:test';

import {
  DEFAULT_CONFIG,
  changedPackages,
  declaredPackages,
  findOffenders,
  isReleaseFile,
  isReleasePrChange,
  isReleaseTitle,
  main,
  parseTitleScopes,
  releaseFiles,
} from '../../labeling/check-pr-scope-files.mjs';

const CONFIG = {
  scopeToLabel: {
    ci: 'ci',
    docs: 'documentation',
    example: 'example',
    infra: 'infra',
    'example-alias': 'example',
    quickjs: 'example',
    rust: 'rust',
  },
  fileRules: [
    { label: 'example', prefix: 'libs/example/' },
    { label: 'rust', prefix: 'rust/' },
    { label: 'infra', prefix: 'install/' },
    { label: 'github_actions', prefix: '.github/workflows/' },
    { label: 'dependencies', suffix: 'pyproject.toml' },
  ],
};

const tmpDirs = [];

function tmpDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pr-scope-check-'));
  tmpDirs.push(dir);
  return dir;
}

after(() => {
  for (const dir of tmpDirs) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

function writeReleaseConfig(dir, payload) {
  const file = path.join(dir, 'release-please-config.json');
  fs.writeFileSync(file, JSON.stringify(payload), 'utf8');
  return file;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

// Capture what `main` writes to stdout and stderr, mirroring pytest's capsys.
function runMain(...args) {
  const stdout = [];
  const stderr = [];
  const originalStdout = process.stdout.write;
  const originalStderr = process.stderr.write;
  process.stdout.write = (chunk) => {
    stdout.push(String(chunk));
    return true;
  };
  process.stderr.write = (chunk) => {
    stderr.push(String(chunk));
    return true;
  };
  let rc;
  try {
    rc = main(...args);
  } finally {
    process.stdout.write = originalStdout;
    process.stderr.write = originalStderr;
  }
  return { rc, stdout: stdout.join(''), stderr: stderr.join('') };
}

test('matching scope/files pass', () => {
  const changed = ['libs/example/app.py'];
  assert.deepEqual(findOffenders('fix(example): repair startup', changed, CONFIG), []);
});

test('rust scope with example files blocks', () => {
  const changed = ['libs/example/app.py'];
  assert.deepEqual(findOffenders('fix(rust): repair startup', changed, CONFIG), [
    { package: 'example', dirs: ['libs/example/'] },
  ]);
});

test('multi-scope title covers multiple package dirs', () => {
  const changed = [
    'rust/crates/niki-proto/src/lib.rs',
    'libs/example/app.py',
  ];
  assert.deepEqual(findOffenders('feat(rust,example): share option', changed, CONFIG), []);
});

test('multi-scope title blocks uncovered package dir', () => {
  const changed = [
    'rust/crates/niki-proto/src/lib.rs',
    'libs/example/app.py',
    'install/install.sh',
  ];
  assert.deepEqual(findOffenders('feat(rust,example): share option', changed, CONFIG), [
    { package: 'infra', dirs: ['install/'] },
  ]);
});

test('unscoped title type and non-package paths pass', () => {
  assert.deepEqual(
    findOffenders(
      'ci(docs): tune workflow',
      ['.github/workflows/ci.yml', 'README.md', 'pyproject.toml'],
      CONFIG,
    ),
    [],
  );
  assert.deepEqual(
    findOffenders('ci(docs): tune package job', ['libs/example/app.py'], CONFIG),
    [],
  );
});

test('non-package path with package scope passes', () => {
  assert.deepEqual(findOffenders('fix(rust): repair action', ['action.yml'], CONFIG), []);
});

test('package scope aliases resolve to same package label', () => {
  assert.deepEqual(
    [...declaredPackages('fix(example-alias): repair startup', CONFIG)].sort(),
    ['example'],
  );
  assert.deepEqual(
    findOffenders('fix(example-alias): repair startup', ['libs/example/app.py'], CONFIG),
    [],
  );
});

test('parse title scopes variants', () => {
  assert.deepEqual(parseTitleScopes('feat(rust, example): x'), ['rust', 'example']);
  assert.deepEqual(parseTitleScopes('fix(example-alias)!: x'), ['example-alias']);
  assert.deepEqual(parseTitleScopes('fix: x'), []);
  assert.deepEqual(parseTitleScopes('not conventional'), []);
});

test('changed packages returns touched package dirs only', () => {
  assert.deepEqual(
    changedPackages(['libs/example/app.py', '.github/workflows/ci.yml'], CONFIG),
    { example: ['libs/example/'] },
  );
});

test('changed packages matches bare package dir', () => {
  assert.deepEqual(changedPackages(['libs/example'], CONFIG), {
    example: ['libs/example/'],
  });
});

test('changed packages ignores prefix collision', () => {
  assert.deepEqual(changedPackages(['libs/examplex/app.py'], CONFIG), {});
});

test('breaking change title still detects offender', () => {
  assert.deepEqual(findOffenders('feat(rust)!: drop option', ['libs/example/app.py'], CONFIG), [
    { package: 'example', dirs: ['libs/example/'] },
  ]);
});

test('release title with release files bypasses scope/file check', () => {
  // Release PRs can touch generated/version files across package dirs.
  //
  // The changed files belong to a package the title scope does not cover, so
  // absent the bypass they are genuine offenders. The assertion can therefore
  // only pass via the release bypass, not ordinary scope coverage — deleting
  // the filtering in `findOffenders` makes this test fail.
  const release = writeReleaseConfig(tmpDir(), {
    packages: {
      'libs/example': {
        'changelog-path': 'CHANGELOG.md',
        'extra-files': ['_version.py'],
      },
    },
  });
  const changed = ['libs/example/CHANGELOG.md', 'libs/example/_version.py'];

  assert.ok(isReleaseTitle('release(rust): 1.1.2'));
  assert.ok(
    isReleasePrChange('release(rust): 1.1.2', changed, { releaseConfigPath: release }),
  );
  assert.deepEqual(
    findOffenders('release(rust): 1.1.2', changed, CONFIG, { releaseConfigPath: release }),
    [],
  );
});

test('release title with source change still blocks mismatch', () => {
  assert.deepEqual(
    findOffenders('release(rust): anything', ['libs/example/app.py'], CONFIG),
    [{ package: 'example', dirs: ['libs/example/'] }],
  );
});

test('release pr change requires release files only', () => {
  const release = writeReleaseConfig(tmpDir(), {
    packages: {
      'libs/example': {
        'changelog-path': 'CHANGELOG.md',
        'extra-files': ['_version.py'],
      },
    },
  });
  const isRelease = (file) => isReleaseFile(file, { releaseConfigPath: release });

  assert.ok(isRelease('.release-please-manifest.json'));
  assert.ok(isRelease('libs/example/CHANGELOG.md'));
  assert.ok(isRelease('libs/example/_version.py'));
  assert.ok(!isRelease('libs/example/app.py'));
  assert.ok(!isRelease('libs/example/docs/CHANGELOG.md'));
  assert.ok(!isRelease('libs/example/nested/_version.py'));
  // The manifest match is anchored to the repo root, not matched by name.
  assert.ok(!isRelease('libs/example/.release-please-manifest.json'));
  assert.ok(
    !isReleasePrChange(
      'release(example-alias): anything',
      ['libs/example/app.py'],
      { releaseConfigPath: release },
    ),
  );
  assert.ok(
    !isReleasePrChange('release(example-alias): anything', [], {
      releaseConfigPath: release,
    }),
  );
});

test('unmanaged package artifacts do not trigger release bypass', () => {
  const config = readJson(DEFAULT_CONFIG);
  const changed = ['libs/unreleased/pyproject.toml'];

  assert.ok(!isReleaseFile('libs/unreleased/pyproject.toml'));
  assert.ok(!isReleaseFile('libs/unreleased/anything/_version.py'));
  assert.ok(!isReleasePrChange('release(example-alias): 0.1.22', changed));
  assert.deepEqual(findOffenders('release(example-alias): 0.1.22', changed, config), []);
});

test('release files validates structure', () => {
  const dir = tmpDir();
  const empty = writeReleaseConfig(dir, { packages: {} });
  assert.deepEqual([...releaseFiles(empty)].sort(), ['.release-please-manifest.json']);

  const cases = [
    [{ packages: 'x' }, /no 'packages' map/],
    [{ packages: { rust: 'notadict' } }, /malformed/],
    [{ packages: { rust: { 'extra-files': 'x' } } }, /malformed 'extra-files'/],
    [{ packages: { rust: { 'extra-files': [1] } } }, /non-string extra file/],
  ];
  cases.forEach(([payload, pattern], index) => {
    const file = path.join(dir, `release-${index}.json`);
    fs.writeFileSync(file, JSON.stringify(payload), 'utf8');
    assert.throws(() => releaseFiles(file), pattern);
  });

  const missing = path.join(dir, 'nope.json');
  assert.throws(() => releaseFiles(missing), /could not read release-please config/);
});

test('release pr change fails closed on malformed release config', () => {
  const bad = writeReleaseConfig(tmpDir(), { packages: 'not-a-map' });
  assert.throws(
    () =>
      isReleasePrChange('release(example-alias): 0.1.22', ['rust/src/lib.rs'], {
        releaseConfigPath: bad,
      }),
    /no 'packages' map/,
  );
});

test('main malformed release config fails closed and attributes correctly', () => {
  const dir = tmpDir();
  const labeler = path.join(dir, 'pr-labeler-config.json');
  fs.writeFileSync(labeler, JSON.stringify(CONFIG), 'utf8');
  const release = writeReleaseConfig(dir, { packages: 'not-a-map' });

  const { rc, stdout, stderr } = runMain(
    'release(example-alias): 0.1.22',
    ['rust/src/lib.rs'],
    labeler,
    release,
  );

  assert.equal(rc, 2);
  assert.equal(stdout, '');
  assert.match(stderr, /release-please config/);
  assert.doesNotMatch(stderr, /PR labeler config/);
});

test('release title with mixed files does not bypass', () => {
  const changed = ['libs/example/CHANGELOG.md', 'libs/example/app.py'];
  assert.ok(!isReleasePrChange('release(rust): 0.1.22', changed));
  assert.deepEqual(findOffenders('release(rust): 0.1.22', changed, CONFIG), [
    { package: 'example', dirs: ['libs/example/'] },
  ]);
});

test('release bypass does not validate component', () => {
  // The bypass keys off title shape + files, not a real component name.
  //
  // Pins that an unrecognized component still bypasses when every file is a
  // release artifact, so a future tightening (validating the component against
  // real package names) is a conscious change rather than a silent one.
  const release = writeReleaseConfig(tmpDir(), {
    packages: { 'libs/example': { 'changelog-path': 'CHANGELOG.md' } },
  });
  assert.deepEqual(
    findOffenders(
      'release(not-a-real-scope): 9.9.9',
      ['libs/example/CHANGELOG.md'],
      CONFIG,
      { releaseConfigPath: release },
    ),
    [],
  );
});

test('is release title boundaries', () => {
  assert.ok(isReleaseTitle('release(example): 1.0.0'));
  assert.ok(isReleaseTitle('release(): 1.0.0')); // empty scope still matches
  assert.ok(!isReleaseTitle('release: 1.0.0')); // scope required
  assert.ok(!isReleaseTitle('release(scope)!: 1.0.0')); // breaking marker excluded
  assert.ok(!isReleaseTitle('  release(example): 1.0.0')); // anchored; no leading ws
  assert.ok(!isReleaseTitle('Release(example): 1.0.0')); // case-sensitive
});

test('nested module resolves to the rust package', () => {
  const config = readJson(DEFAULT_CONFIG);
  assert.deepEqual(
    findOffenders(
      'fix(infra): repair startup',
      ['rust/crates/niki-agent/src/vendor/quickjs/middleware.rs'],
      config,
    ),
    [{ package: 'package:rust', dirs: ['rust/'] }],
  );
});

test('package scopes resolve with real config', () => {
  const config = readJson(DEFAULT_CONFIG);
  assert.deepEqual([...declaredPackages('fix(rust): x', config)].sort(), ['package:rust']);
  assert.deepEqual([...declaredPackages('fix(infra): x', config)].sort(), ['package:infra']);
  assert.deepEqual([...declaredPackages('fix(code): x', config)], []);
  assert.deepEqual([...declaredPackages('fix(ci): x', config)], []);
});

test('non dict file rule raises', () => {
  // A non-object `fileRules` entry is config corruption, not a skip.
  const config = { scopeToLabel: CONFIG.scopeToLabel, fileRules: ['libs/example/'] };
  assert.throws(
    () => findOffenders('fix(rust): x', ['libs/example/file.py'], config),
    /not an object/,
  );
});

test('non string prefix file rule raises', () => {
  const config = {
    scopeToLabel: CONFIG.scopeToLabel,
    fileRules: [{ label: 'example', prefix: 123 }],
  };
  assert.throws(
    () => findOffenders('fix(rust): x', ['libs/example/file.py'], config),
    /non-string label\/prefix/,
  );
});

test('main partially malformed file rules returns 2', () => {
  // A single malformed package rule fails closed instead of being dropped.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(
    configPath,
    JSON.stringify({
      scopeToLabel: CONFIG.scopeToLabel,
      fileRules: [
        { label: 'example', prefix: 123 },
        { label: 'rust', prefix: 'rust/' },
      ],
    }),
    'utf8',
  );
  const { rc, stderr } = runMain(
    'fix(rust): x',
    ['libs/example/file.py'],
    configPath,
  );
  assert.equal(rc, 2);
  assert.match(stderr, /::error::/);
});

test('main release bypass validates labeler config', () => {
  // Release artifact bypasses still fail closed on malformed labeler config.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(
    configPath,
    JSON.stringify({
      scopeToLabel: CONFIG.scopeToLabel,
      fileRules: [{ label: 'example', prefix: 123 }],
    }),
    'utf8',
  );
  const { rc, stdout, stderr } = runMain(
    'release(example-alias): 0.1.22',
    ['libs/example/file.py'],
    configPath,
  );

  assert.equal(rc, 2);
  assert.equal(stdout, '');
  assert.match(stderr, /fileRules/);
});

test('main stdout is json array', () => {
  // The workflow can strictly parse stdout as JSON.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(configPath, JSON.stringify(CONFIG), 'utf8');

  const { rc, stdout, stderr } = runMain(
    'fix(rust): repair startup',
    ['libs/example/app.py'],
    configPath,
  );

  assert.equal(rc, 0);
  assert.deepEqual(JSON.parse(stdout), [
    { package: 'example', dirs: ['libs/example/'] },
  ]);
  assert.match(stderr, /PR title scope does not cover/);
});

test('main clean stdout is empty json array', () => {
  // A clean PR prints exactly `[]` and nothing on stderr.
  //
  // The calling workflow consumes raw stdout: its fail-closed JSON parse blocks
  // on empty or non-array output, and its detector-absent branch hard-codes the
  // same `[]` sentinel. Pin the exact wire contract here so a future change to
  // the clean-path output can't silently desync the gate from the script.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(configPath, JSON.stringify(CONFIG), 'utf8');

  const { rc, stdout, stderr } = runMain(
    'fix(rust): repair startup',
    ['rust/crates/niki-proto/src/lib.rs'],
    configPath,
  );

  assert.equal(rc, 0);
  assert.equal(stdout.trim(), '[]');
  assert.equal(stderr, '');
});

test('main release bypass emits notice', () => {
  // A fired release bypass surfaces a `::notice::` so it is not silent.
  //
  // The gate standing down must be distinguishable from a genuinely clean PR in
  // the Checks UI. The notice goes to stderr so it never corrupts the JSON
  // offenders the workflow captures from stdout.
  const dir = tmpDir();
  const configPath = path.join(dir, 'pr-labeler-config.json');
  fs.writeFileSync(configPath, JSON.stringify(CONFIG), 'utf8');
  const release = writeReleaseConfig(dir, {
    packages: { 'libs/example': { 'changelog-path': 'CHANGELOG.md' } },
  });

  const { rc, stdout, stderr } = runMain(
    'release(rust): 0.1.22',
    ['libs/example/CHANGELOG.md'],
    configPath,
    release,
  );

  assert.equal(rc, 0);
  assert.equal(stdout.trim(), '[]');
  assert.match(stderr, /::notice::/);
});

test('main missing config returns 2', () => {
  // A missing config fails closed instead of silently passing.
  const { rc, stderr } = runMain(
    'fix(rust): x',
    ['libs/example/file.py'],
    path.join(tmpDir(), 'nope.json'),
  );
  assert.equal(rc, 2);
  assert.match(stderr, /::error::/);
});

test('main malformed config returns 2', () => {
  // Invalid JSON fails closed.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(configPath, '{not json', 'utf8');
  const { rc, stderr } = runMain('fix(rust): x', ['libs/example/file.py'], configPath);
  assert.equal(rc, 2);
  assert.match(stderr, /::error::/);
});

test('main empty file rules returns 2', () => {
  // Config drift that removes package file rules fails closed.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(
    configPath,
    JSON.stringify({ scopeToLabel: CONFIG.scopeToLabel, fileRules: [] }),
    'utf8',
  );
  const { rc, stderr } = runMain('fix(rust): x', ['libs/example/file.py'], configPath);
  assert.equal(rc, 2);
  assert.match(stderr, /fileRules/);
});

test('main missing scope map returns 2', () => {
  // Config drift that removes `scopeToLabel` fails closed.
  const configPath = path.join(tmpDir(), 'pr-labeler-config.json');
  fs.writeFileSync(
    configPath,
    JSON.stringify({ scopeToLabel: {}, fileRules: CONFIG.fileRules }),
    'utf8',
  );
  const { rc, stderr } = runMain('fix(rust): x', ['libs/example/file.py'], configPath);
  assert.equal(rc, 2);
  assert.match(stderr, /scopeToLabel/);
});

test('real config has package scope and dir mappings', () => {
  // The committed PR labeler config exposes the maps this check reads.
  const config = readJson(DEFAULT_CONFIG);
  assert.deepEqual([...declaredPackages('fix(rust): x', config)].sort(), ['package:rust']);
  assert.deepEqual([...declaredPackages('fix(code): x', config)], []);
  assert.deepEqual(changedPackages(['rust/crates/niki-proto/src/lib.rs'], config), {
    'package:rust': ['rust/'],
  });
  assert.deepEqual(changedPackages(['install/install.sh'], config), {
    'package:infra': ['install/'],
  });
});
