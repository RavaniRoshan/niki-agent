import test from 'node:test';
import assert from 'node:assert/strict';
import {
  chamfer,
  clamp01,
  coverage,
  dotRadius,
  fitWord,
  formatCount,
  hash,
  meridianPath,
  nextTick,
  project,
  safeHref,
  scramble,
  smoothstep,
  typeFrame,
} from './ticket-stub-footer';

test('clamp01 and smoothstep stay strictly bounded in [0, 1]', () => {
  assert.equal(clamp01(-5), 0);
  assert.equal(clamp01(0.4), 0.4);
  assert.equal(clamp01(4.2), 1);

  assert.equal(smoothstep(0.2, 0.8, 0.1), 0);
  assert.ok(Math.abs(smoothstep(0.2, 0.8, 0.5) - 0.5) < 1e-12);
  assert.equal(smoothstep(0.2, 0.8, 0.9), 1);
  assert.equal(smoothstep(0.5, 0.5, 0.4), 0);
  assert.equal(smoothstep(0.5, 0.5, 0.5), 1);
});

test('formatCount formats positive integers with thousands separators and guards invalid input', () => {
  assert.equal(formatCount(12745012), '12,745,012');
  assert.equal(formatCount(255480.9), '255,480');
  assert.equal(formatCount(-10), '0');
  assert.equal(formatCount(Number.NaN), '0');
});

test('chamfer builds a valid 8-vertex CSS clip-path polygon', () => {
  const poly = chamfer(9, 9, 9, 9);
  assert.ok(poly.startsWith('polygon(') && poly.endsWith(')'));
  assert.equal(poly.split(',').length, 8);
});

test('typeFrame loops smoothly through type, hold, erase, and gap phases', () => {
  const timing = { type: 10, hold: 100, erase: 10, gap: 50 };
  const phrases = ['niki', 'calm'];
  assert.equal(typeFrame(phrases, 5, timing).phase, 'type');
  assert.equal(typeFrame(phrases, 50, timing).phase, 'hold');
  assert.equal(typeFrame(phrases, 50, timing).text, 'niki');
  assert.equal(typeFrame(phrases, 150, timing).phase, 'erase');
  assert.equal(typeFrame(phrases, 200, timing).phase, 'gap');
  assert.equal(typeFrame([], 10, timing).text, '');
});

test('hash is deterministic and scramble preserves length and whitespace', () => {
  assert.equal(hash(42), hash(42));
  assert.ok(hash(42) >= 0 && hash(42) < 1);

  const src = 'Niki Agent stays still at rest.';
  const mid = scramble(src, 0.5, 3);
  assert.equal(mid.length, src.length);
  assert.equal(scramble(src, 1, 3), src);
});

test('coverage, dotRadius, project, meridianPath, fitWord, and nextTick return valid geometry', () => {
  const tone = {
    fade: 0.44,
    depth: 0.84,
    lensX: 100,
    lensY: 50,
    lensR: 60,
    lens: 0,
    reveal: 1,
    wave: 0,
    time: 0,
    rings: [],
    ringW: 20,
  };
  const topInk = coverage(50, 10, 500, 120, tone);
  const bottomInk = coverage(50, 118, 500, 120, tone);
  assert.ok(topInk > bottomInk);
  assert.ok(dotRadius(1, 6) > dotRadius(0.2, 6));

  const [x, y, z] = project(0, 0, 0, 0);
  assert.equal(x, 0);
  assert.equal(y, 0);
  assert.equal(z, 1);

  const path = meridianPath(0.4, 0.34, -1.36, 150, 120, 100, 32);
  assert.ok(path.startsWith('M'));

  const fit = fitWord(400, 72, 1000, 200, 10);
  assert.ok(fit.size > 0);

  const tick = nextTick(1.8, () => 0.5);
  assert.ok(tick.delay >= 450 && tick.add >= 1);
  assert.equal(nextTick(0, () => 0.5).add, 0);
});

test('safeHref permits safe relative anchors and http(s) URLs while blocking unsafe protocols', () => {
  assert.equal(safeHref('#hero'), '#hero');
  assert.equal(safeHref('/docs'), '/docs');
  assert.equal(safeHref('https://github.com/RavaniRoshan/niki-agent'), 'https://github.com/RavaniRoshan/niki-agent');
  assert.equal(safeHref('javascript:alert(1)'), '#');
  assert.equal(safeHref('data:text/html,<script>alert(1)</script>'), '#');
  assert.equal(safeHref('vbscript:msgbox(1)'), '#');
  assert.equal(safeHref(undefined), '#');
});
