"use strict";

// Indentation-based readers for the workflow YAML these tests assert on.
//
// The repository has no package.json and the helper tests install nothing, so
// this deliberately supports only the small YAML shape the workflows use:
// block mappings, `key: value` scalars, flow lists, and literal `|` blocks.
// Anything richer should be asserted through the workflow's own behaviour.

const fs = require('node:fs');
const path = require('node:path');

function readWorkflow(root, name) {
  return fs.readFileSync(path.join(root, '.github', 'workflows', name), 'utf8');
}

function indentOf(line) {
  return line.length - line.trimStart().length;
}

function parseScalar(raw) {
  let value = raw.trim();
  // YAML drops an inline comment from an unquoted scalar.
  if (!value.startsWith('"') && !value.startsWith("'")) {
    value = value.replace(/\s+#.*$/, '');
  }
  if (value === 'true') return true;
  if (value === 'false') return false;
  if (value === 'null' || value === '~') return null;
  if (/^-?\d+$/.test(value)) return Number(value);
  if (value.startsWith('"') && value.endsWith('"')) return value.slice(1, -1);
  if (value.startsWith("'") && value.endsWith("'")) return value.slice(1, -1);
  return value;
}

function parseList(raw) {
  const value = raw.trim();
  if (!value.startsWith('[') || !value.endsWith(']')) return undefined;
  return value
    .slice(1, -1)
    .split(',')
    .map((item) => parseScalar(item))
    .filter((item) => item !== '');
}

function findKey(text, key, indent) {
  const lines = text.split('\n');
  const prefix = `${' '.repeat(indent)}${key}:`;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line === prefix || line.startsWith(`${prefix} `)) return index;
  }
  return -1;
}

function childLines(text, index) {
  const lines = text.split('\n');
  const base = indentOf(lines[index]);
  const children = [];
  for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
    const line = lines[cursor];
    if (line.trim() === '') {
      children.push(line);
      continue;
    }
    if (indentOf(line) <= base) break;
    children.push(line);
  }
  return children;
}

function entry(text, key, indent) {
  const index = findKey(text, key, indent);
  if (index === -1) return undefined;
  const line = text.split('\n')[index];
  return {
    value: line.slice(indent + key.length + 1).trim(),
    children: childLines(text, index),
  };
}

function valueAt(text, key, indent) {
  const found = entry(text, key, indent);
  return found === undefined ? undefined : parseScalar(found.value);
}

function listAt(text, key, indent) {
  const found = entry(text, key, indent);
  return found === undefined ? undefined : parseList(found.value);
}

function parseMap(lines, indent) {
  const result = {};
  for (const line of lines) {
    if (line.trim() === '' || line.trimStart().startsWith('#')) continue;
    if (indentOf(line) !== indent) continue;
    const rest = line.slice(indent);
    const split = rest.indexOf(':');
    if (split === -1) continue;
    const key = rest.slice(0, split).trim();
    if (!key) continue;
    result[key] = parseScalar(rest.slice(split + 1));
  }
  return result;
}

function mapAt(text, key, indent) {
  const found = entry(text, key, indent);
  return found === undefined ? undefined : parseMap(found.children, indent + 2);
}

function literalAt(text, key, indent) {
  const found = entry(text, key, indent);
  if (found === undefined || found.value !== '|') {
    throw new Error(`expected a \`${key}: |\` block at indent ${indent}`);
  }
  const content = found.children.filter((line) => line.trim() !== '');
  const common = Math.min(...content.map(indentOf));
  return content.map((line) => line.slice(common)).join('\n');
}

function jobIds(text) {
  const lines = text.split('\n');
  const start = lines.indexOf('jobs:');
  if (start === -1) throw new Error('expected a top-level `jobs:` block');
  const ids = [];
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === '' || line.trimStart().startsWith('#')) continue;
    if (indentOf(line) === 0) break;
    const match = /^ {2}([A-Za-z0-9_-]+):\s*$/.exec(line);
    if (match) ids.push(match[1]);
  }
  return ids;
}

function jobBlock(text, id) {
  const lines = text.split('\n');
  const start = lines.indexOf(`  ${id}:`);
  if (start === -1) throw new Error(`no job named ${id}`);
  let end = start + 1;
  for (; end < lines.length; end += 1) {
    const line = lines[end];
    if (line.trim() === '') continue;
    if (indentOf(line) <= 2) break;
  }
  return lines.slice(start, end).join('\n');
}

function stepsOf(jobText) {
  const lines = jobText.split('\n');
  const start = lines.indexOf('    steps:');
  if (start === -1) throw new Error('no `steps:` list in job');
  const steps = [];
  let current = null;
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (/^ {6}-(\s|$)/.test(line)) {
      if (current) steps.push(current);
      current = [line];
      continue;
    }
    if (!current) continue;
    if (line.trim() !== '' && indentOf(line) <= 4) {
      steps.push(current);
      current = null;
    } else {
      current.push(line);
    }
  }
  if (current) steps.push(current);
  return steps.map((step) => step.join('\n'));
}

function stepNamed(jobText, name) {
  const matches = stepsOf(jobText).filter((step) => {
    const match = /^ {6}- name:\s*(.*)$/.exec(step.split('\n')[0]);
    return match !== null && parseScalar(match[1]) === name;
  });
  if (matches.length !== 1) {
    throw new Error(`expected one step named ${name}, found ${matches.length}`);
  }
  return matches[0];
}

module.exports = {
  readWorkflow,
  jobIds,
  jobBlock,
  stepsOf,
  stepNamed,
  valueAt,
  listAt,
  mapAt,
  literalAt,
};
