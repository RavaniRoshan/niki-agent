import gsap from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';

let registered = false;

export function ensureGsapRegistered(): void {
  if (registered || typeof window === 'undefined') return;
  gsap.registerPlugin(ScrollTrigger);
  ScrollTrigger.config({ ignoreMobileResize: true });
  registered = true;
}

export function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined') return true;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

export const MOTION_TIMESTAMPS = {
  heroEyebrow: 0,
  heroHeadline: 0.08,
  heroCommand: 0.2,
  heroLogos: 0.34,
  sectionHeader: 0,
  sectionGrid: 0.12,
  sectionStage: 0.24,
  sectionSatellite: 0.34,
  orbitalPulse: 0,
  orbitalTitle: 0.04,
  orbitalDiagram: 0.08,
  orbitalBullets: 0.13,
  orbitalLinks: 0.22,
} as const;

export { gsap, ScrollTrigger };
