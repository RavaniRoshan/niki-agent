"use strict";

// CI workflow: one Rust job and one aggregate gate.
//
// The workflow used to run a second package job. After the independent Rust
// product became the only build target, these tests pin the small shape that is
// left instead of the machinery that left with it.

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const {
  readWorkflow,
  jobIds,
  jobBlock,
  valueAt,
  listAt,
  literalAt,
} = require('./workflow-text.js');

const ROOT = path.resolve(__dirname, '../../../..');
const CI_WORKFLOW = readWorkflow(ROOT, 'ci.yml');

test('ci runs the rust job plus an aggregate', () => {
  // A missing job means `CI Success` can never be satisfied.
  assert.deepEqual(new Set(jobIds(CI_WORKFLOW)), new Set(['rust', 'ci_success']));
});

test('rust job runs the workspace gate', () => {
  // The authoritative gate is the Rust workspace as a contributor runs it.
  assert.ok(CI_WORKFLOW.includes('cargo test -j 4 --workspace --locked'));
  assert.ok(CI_WORKFLOW.includes('cargo fmt --all -- --check'));
  assert.ok(
    CI_WORKFLOW.includes(
      'cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings',
    ),
  );
});

test('success gate fails on any result but success', () => {
  // The aggregate must not pass on `cancelled`, `skipped`, or `failure`.
  const job = jobBlock(CI_WORKFLOW, 'ci_success');
  assert.deepEqual(listAt(job, 'needs', 4), ['rust']);
  assert.equal(valueAt(job, 'if', 4), 'always()');
  const run = literalAt(job, 'run', 8);
  assert.ok(run.includes('"$RUST_RESULT" != "success"'));
});

test('no job targets a removed package', () => {
  // The workflow names one product; a window into a removed tree would fail.
  const removed = [
    'libs/evals',
    'libs/deepagents',
    'libs/acp',
    'libs/talon',
    'libs/partners',
  ];
  for (const directory of removed) {
    assert.ok(!CI_WORKFLOW.includes(directory), `${directory} is archived, not built`);
  }
});
