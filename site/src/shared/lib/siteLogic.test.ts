import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildProviderInstallCommand,
  clampCarouselIndex,
  computeActiveChapterIndex,
  computeOrbitalScrollState,
  computeParallaxOffset,
  getNextCarouselIndex,
  getOrbitalStagePositions,
  getStageScrollProgress,
  isLightNavbarZone,
  isNavbarAttached,
  resolveColorMode,
  validateDemoForm,
  validateEmail,
} from './siteLogic';

test('validateEmail rejects empty or malformed addresses and accepts valid emails', () => {
  assert.equal(validateEmail('').valid, false);
  assert.equal(validateEmail('not-an-email').valid, false);
  assert.equal(validateEmail('team@niki-agent.dev').valid, true);
});

test('validateDemoForm reports required field errors and passes on complete valid input', () => {
  const invalid = validateDemoForm({ name: 'A', email: 'bad', company: '', useCase: '' });
  assert.equal(invalid.valid, false);
  assert.ok(invalid.errors.name);
  assert.ok(invalid.errors.email);
  assert.ok(invalid.errors.company);

  const valid = validateDemoForm({
    name: 'Roshan Ravani',
    email: 'roshan@niki-agent.dev',
    company: 'Niki Labs',
    useCase: 'Calm terminal coding',
  });
  assert.equal(valid.valid, true);
});

test('carousel navigation wraps cleanly in both directions and clamps', () => {
  assert.equal(getNextCarouselIndex(0, -1, 5), 4);
  assert.equal(getNextCarouselIndex(4, 1, 5), 0);
  assert.equal(clampCarouselIndex(-2, 5), 0);
  assert.equal(clampCarouselIndex(9, 5), 4);
});

test('orbital stage positions center the active stage at 0 degrees', () => {
  const positions = getOrbitalStagePositions(2, 6, 28);
  const active = positions.find((p) => p.index === 2);
  assert.ok(active);
  assert.equal(active.isActive, true);
  assert.equal(active.angleDeg, 0);
});

test('isLightNavbarZone detects overlap with light surface band', () => {
  assert.equal(isLightNavbarZone(82, 40, 900), true);
  assert.equal(isLightNavbarZone(82, 120, 900), false);
  assert.equal(isLightNavbarZone(82, -500, 60), false);
});

test('isNavbarAttached stays inlaid at top and attaches on scroll or open menu', () => {
  assert.equal(isNavbarAttached(0, false), false);
  assert.equal(isNavbarAttached(24, false), false);
  assert.equal(isNavbarAttached(25, false), true);
  assert.equal(isNavbarAttached(0, true), true);
});

test('resolveColorMode defaults to dark and respects explicit preferences', () => {
  assert.equal(resolveColorMode(null, false), 'dark');
  assert.equal(resolveColorMode(undefined, false), 'dark');
  assert.equal(resolveColorMode('light', true), 'light');
  assert.equal(resolveColorMode('dark', false), 'dark');
  assert.equal(resolveColorMode('system', true), 'dark');
  assert.equal(resolveColorMode('system', false), 'light');
});

test('computeParallaxOffset clamps progress and scales by depth symmetrically', () => {
  assert.equal(computeParallaxOffset(0.5, 1, 40), 0);
  assert.equal(computeParallaxOffset(0, 1, 40), 40);
  assert.equal(computeParallaxOffset(1, 1, 40), -40);
  assert.equal(computeParallaxOffset(1.5, 0.5, 40), -20);
});

test('computeActiveChapterIndex maps scroll progress across chapters cleanly', () => {
  assert.equal(computeActiveChapterIndex(0, 4), 0);
  assert.equal(computeActiveChapterIndex(0.3, 4), 1);
  assert.equal(computeActiveChapterIndex(0.6, 4), 2);
  assert.equal(computeActiveChapterIndex(1, 4), 3);
});

test('buildProviderInstallCommand returns the installer line or a real in-session /install command', () => {
  assert.ok(buildProviderInstallCommand('install').startsWith('curl -fsSL'));
  assert.equal(buildProviderInstallCommand('anthropic'), 'export ANTHROPIC_API_KEY=... && niki');
  assert.equal(buildProviderInstallCommand('google'), 'export GOOGLE_API_KEY=... && niki');
  assert.equal(buildProviderInstallCommand('ollama'), '/install ollama');
});

test('computeOrbitalScrollState sequences all 6 dial stages smoothly across scroll progress', () => {
  const s0 = computeOrbitalScrollState(0, 6, 28);
  assert.equal(s0.activeIndex, 0);
  assert.equal(s0.ringRotationDeg, 0);

  const s2 = computeOrbitalScrollState(0.4, 6, 28);
  assert.equal(s2.activeIndex, 2);
  assert.equal(s2.ringRotationDeg, -56);

  const s5 = computeOrbitalScrollState(1, 6, 28);
  assert.equal(s5.activeIndex, 5);
  assert.equal(s5.ringRotationDeg, -140);

  assert.equal(getStageScrollProgress(0, 6), 0);
  assert.equal(getStageScrollProgress(5, 6), 1);
  assert.equal(getStageScrollProgress(2, 6), 0.4);
});
