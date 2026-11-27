"use strict";

// Structural contracts for the new-Markdown-file pull request gate.
//
// The gate's *behavior* is tested in
// `.github/scripts/tests/checks/markdown_file_check.test.js`, which runs the real
// `run()` against a fake octokit. This module covers only what lives in YAML and
// therefore cannot be executed: the trigger list, the permissions, the trusted
// checkout, and the thinness of the inline script itself.

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const {
  readWorkflow,
  jobBlock,
  stepsOf,
  valueAt,
  listAt,
  mapAt,
  literalAt,
} = require('./workflow-text.js');

const ROOT = path.resolve(__dirname, '../../../..');
const WORKFLOW_NAME = 'markdown_file_check.yml';
const DETECTOR = '.github/scripts/checks/markdown_file_check.js';
const GITHUB_SCRIPT_PIN = 'actions/github-script@3a2844b7e9c422d3c10d287c895573f7108da1b3';
const CHECKOUT_PIN = 'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1';

const WORKFLOW = readWorkflow(ROOT, WORKFLOW_NAME);

function steps() {
  return stepsOf(jobBlock(WORKFLOW, 'markdown-file-check'));
}

function script() {
  return literalAt(steps()[1], 'script', 10);
}

test('markdown gate reruns for title file and label changes', () => {
  // Without `labeled`/`unlabeled` the bypass label needs a push to take effect.
  assert.deepEqual(
    new Set(listAt(WORKFLOW, 'types', 4)),
    new Set(['opened', 'edited', 'synchronize', 'reopened', 'labeled', 'unlabeled']),
  );
  assert.deepEqual(mapAt(WORKFLOW, 'permissions', 0), {
    contents: 'read',
    'pull-requests': 'write',
  });
});

test('markdown gate runs only base revision code', () => {
  // The detector must come from base, never from the PR's own head.
  //
  // This job holds `pull-requests: write` under `pull_request_target`. A head
  // checkout would both run PR-authored code against that token and let a PR
  // edit the detector to pass itself.
  const [checkout, check] = steps();

  assert.equal(valueAt(checkout, 'uses', 8), CHECKOUT_PIN);
  assert.equal(valueAt(checkout, 'ref', 10), '${{ github.event.pull_request.base.sha }}');
  assert.equal(valueAt(checkout, 'persist-credentials', 10), false);
  assert.equal(literalAt(checkout, 'sparse-checkout', 10).trim(), DETECTOR);
  // Cone mode is for directory prefixes; declare the single-file intent.
  assert.equal(valueAt(checkout, 'sparse-checkout-cone-mode', 10), false);
  assert.equal(valueAt(check, 'uses', 8), GITHUB_SCRIPT_PIN);
});

test('markdown gate delegates to the tested module', () => {
  // Logic in the YAML string cannot be executed by any test, so keep it out.
  //
  // Anything that moves back into the `script:` block silently loses its
  // coverage: an inverted condition there still ships green.
  const body = script();

  assert.ok(body.includes('require(detector)'));
  assert.ok(body.includes(DETECTOR));
  assert.ok(body.includes('await run({ github, context, core })'));

  // Allow the existence guard and the require, nothing resembling a decision.
  for (const forbidden of ['setFailed', 'listFiles', 'listLabelsOnIssue', 'createComment']) {
    assert.ok(
      !body.includes(forbidden),
      `${forbidden} belongs in ${DETECTOR}, where it can be tested`,
    );
  }
});

test('markdown gate tolerates a base without the detector', () => {
  // Every PR open when this merges has a base.sha predating the detector.
  //
  // Without the guard those runs die on an unhandled module-resolution error:
  // an opaque red X clearable only by rebasing.
  const body = script();

  assert.ok(body.includes('fs.existsSync(detector)'));
  // Warning, not notice: a detector renamed on base without updating this
  // path silently disarms the gate, which must surface in the Checks UI.
  assert.ok(body.includes('core.warning('));
});
