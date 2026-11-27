"use strict";

// Static contracts for credential scoping in GitHub workflows.

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const {
  readWorkflow,
  jobIds,
  jobBlock,
  stepsOf,
  stepNamed,
  valueAt,
  mapAt,
} = require('./workflow-text.js');

const ROOT = path.resolve(__dirname, '../../../..');
const APP_TOKEN_WORKFLOWS = [
  'close_unchecked_issues.yml',
  'pr_labeler.yml',
  'sync_topic_labels.yml',
  'tag-external-issues.yml',
];

function tokenSteps(workflow) {
  const steps = [];
  for (const id of jobIds(workflow)) {
    for (const step of stepsOf(jobBlock(workflow, id))) {
      const uses = String(valueAt(step, 'uses', 8) ?? '');
      if (uses.startsWith('actions/create-github-app-token@')) steps.push(step);
    }
  }
  return steps;
}

test('github app client id uses repository variable', () => {
  for (const name of APP_TOKEN_WORKFLOWS) {
    const steps = tokenSteps(readWorkflow(ROOT, name));
    assert.ok(steps.length > 0, `No GitHub App token step found in ${name}`);
    for (const step of steps) {
      const withMap = mapAt(step, 'with', 8);
      assert.equal(
        withMap['client-id'],
        '${{ vars.ORG_MEMBERSHIP_APP_CLIENT_ID }}',
      );
      assert.equal(
        withMap['private-key'],
        '${{ secrets.ORG_MEMBERSHIP_APP_PRIVATE_KEY }}',
      );
    }
  }
});

test('issue topic classifier uses dedicated environment', () => {
  const workflow = readWorkflow(ROOT, 'sync_topic_labels.yml');
  const job = jobBlock(workflow, 'classifier-dry-run');

  assert.equal(valueAt(job, 'environment', 4), 'labeling');
  assert.ok(!('LANGSMITH_API_KEY' in (mapAt(workflow, 'env', 0) ?? {})));
  assert.ok(!('LANGSMITH_API_KEY' in (mapAt(job, 'env', 4) ?? {})));

  const step = stepNamed(job, 'Classify issue');
  const env = mapAt(step, 'env', 8);
  assert.equal(env.LANGSMITH_API_KEY, '${{ secrets.LANGSMITH_API_KEY }}');
  assert.equal(env.TOPIC_CLASSIFIER_PROVIDER, 'semif');
});
