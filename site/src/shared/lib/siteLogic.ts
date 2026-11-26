import { INSTALL_COMMAND } from '../../entities/niki';

export interface DemoFormInput {
  name: string;
  email: string;
  company: string;
  useCase: string;
}

export interface DemoFormValidation {
  valid: boolean;
  errors: Partial<Record<keyof DemoFormInput, string>>;
}

const EMAIL_REGEX = /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/;

export function validateEmail(email: string): { valid: boolean; error?: string } {
  const trimmed = email.trim();
  if (!trimmed) {
    return { valid: false, error: 'Please enter your email address.' };
  }
  if (!EMAIL_REGEX.test(trimmed)) {
    return { valid: false, error: 'Please enter a valid email address.' };
  }
  return { valid: true };
}

export function validateDemoForm(input: DemoFormInput): DemoFormValidation {
  const errors: Partial<Record<keyof DemoFormInput, string>> = {};

  if (!input.name.trim() || input.name.trim().length < 2) {
    errors.name = 'Please enter your full name.';
  }

  const emailCheck = validateEmail(input.email);
  if (!emailCheck.valid) {
    errors.email = emailCheck.error;
  }

  if (!input.company.trim()) {
    errors.company = 'Please enter your company name.';
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
  };
}

export function getNextCarouselIndex(current: number, delta: number, total: number): number {
  if (total <= 0) return 0;
  return (current + delta + total) % total;
}

export function clampCarouselIndex(index: number, total: number): number {
  if (total <= 0) return 0;
  return Math.max(0, Math.min(total - 1, index));
}

export interface OrbitalStagePosition {
  index: number;
  angleDeg: number;
  isActive: boolean;
  distanceFromActive: number;
}

/**
 * Computes angular positions around the left-anchored orbital semicircle dial.
 * The active stage sits at 0deg (3 o'clock, directly aligned with the connector line).
 */
export function getOrbitalStagePositions(
  activeIndex: number,
  totalStages: number,
  stepDeg = 28
): OrbitalStagePosition[] {
  const positions: OrbitalStagePosition[] = [];
  for (let i = 0; i < totalStages; i++) {
    let diff = i - activeIndex;
    const half = totalStages / 2;
    if (diff > half) diff -= totalStages;
    if (diff < -half) diff += totalStages;

    positions.push({
      index: i,
      angleDeg: diff * stepDeg,
      isActive: i === activeIndex,
      distanceFromActive: Math.abs(diff),
    });
  }
  return positions;
}

/**
 * Determines whether the sticky top navbar is currently overlapping the light theme section.
 */
export function isLightNavbarZone(
  viewportHeaderY: number,
  lightTopRelativeToViewport: number,
  lightBottomRelativeToViewport: number
): boolean {
  return (
    lightTopRelativeToViewport <= viewportHeaderY &&
    lightBottomRelativeToViewport >= viewportHeaderY
  );
}

/**
 * Determines whether the fixed header has transitioned from its initial
 * inlaid hero state into its attached top-docked bar state.
 */
export function isNavbarAttached(
  scrollY: number,
  hasOpenOverlay: boolean,
  threshold = 24
): boolean {
  return hasOpenOverlay || scrollY > threshold;
}

export type ColorModePreference = 'dark' | 'light' | 'system';

/**
 * Resolves the active UI color mode ('dark' | 'light').
 * Defaults to 'dark' when no explicit preference is saved.
 */
export function resolveColorMode(
  preference: string | null | undefined,
  systemPrefersDark: boolean
): 'dark' | 'light' {
  if (preference === 'light') return 'light';
  if (preference === 'dark') return 'dark';
  if (preference === 'system') return systemPrefersDark ? 'dark' : 'light';
  return 'dark';
}

/**
 * Computes a bounded vertical parallax translation (in pixels) for a Stageshow depth layer.
 * progress is normalized to [0, 1]; depth > 0 moves upward as scroll advances, depth < 0 lags behind.
 */
export function computeParallaxOffset(progress: number, depth: number, maxTravelPx = 36): number {
  const clamped = Math.max(0, Math.min(1, progress));
  const centered = (clamped - 0.5) * 2; // [-1, 1]
  const raw = Math.round(-centered * depth * maxTravelPx * 10) / 10;
  return raw === 0 ? 0 : raw;
}

/**
 * Maps normalized scroll progress [0, 1] to a discrete chapter index [0, totalChapters - 1].
 */
export function computeActiveChapterIndex(progress: number, totalChapters: number): number {
  if (totalChapters <= 1) return 0;
  const clamped = Math.max(0, Math.min(0.9999, progress));
  return Math.floor(clamped * totalChapters);
}

/**
 * Builds the in-session integration install command for the Hero action bar.
 * A provider that ships with the app needs no install; everything else is an
 * `/install <extra>` typed inside a running session.
 */
export function buildProviderInstallCommand(providerId: string): string {
  const normalized = providerId.trim().toLowerCase();
  if (!normalized || normalized === 'install') {
    return INSTALL_COMMAND;
  }
  if (normalized === 'anthropic' || normalized === 'openai' || normalized === 'google') {
    return `export ${normalized.toUpperCase()}_API_KEY=... && niki`;
  }
  const extra = normalized.replace(/[^a-z0-9-]/g, '');
  return extra ? `/install ${extra}` : INSTALL_COMMAND;
}

export interface OrbitalScrollState {
  activeIndex: number;
  continuousIndex: number;
  ringRotationDeg: number;
  stageLocalProgress: number;
}

/**
 * Maps normalized vertical scroll progress [0, 1] through the Orbital section
 * onto a smooth 6-stage orbital sequence so every stage rotates cleanly into view.
 */
export function computeOrbitalScrollState(
  progress: number,
  totalStages: number,
  stepDeg = 28
): OrbitalScrollState {
  if (totalStages <= 1) {
    return {
      activeIndex: 0,
      continuousIndex: 0,
      ringRotationDeg: 0,
      stageLocalProgress: 0,
    };
  }

  const clamped = Math.max(0, Math.min(1, progress));
  const continuousIndex = clamped * (totalStages - 1);
  const activeIndex = Math.min(totalStages - 1, Math.round(continuousIndex));
  const rawAngle = Math.round(-continuousIndex * stepDeg * 10) / 10;
  const ringRotationDeg = rawAngle === 0 ? 0 : rawAngle;

  const bandSize = 1 / totalStages;
  const stageStart = activeIndex * bandSize;
  const local = Math.max(0, Math.min(1, (clamped - stageStart) / bandSize));
  const stageLocalProgress = Math.round(local * 100) / 100;

  return {
    activeIndex,
    continuousIndex: Math.round(continuousIndex * 1000) / 1000,
    ringRotationDeg,
    stageLocalProgress,
  };
}

/**
 * Computes the target normalized scroll progress [0, 1] for jumping to a specific orbital stage.
 */
export function getStageScrollProgress(stageIndex: number, totalStages: number): number {
  if (totalStages <= 1) return 0;
  const clamped = Math.max(0, Math.min(totalStages - 1, stageIndex));
  return clamped / (totalStages - 1);
}
