import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowUpRight } from 'lucide-react';
import { LIFECYCLE_STAGES } from '../../../entities/niki';
import {
  BTN,
  computeOrbitalScrollState,
  ensureGsapRegistered,
  getStageScrollProgress,
  GridBand,
  gsap,
  MOTION_TIMESTAMPS,
  NikiMark,
  prefersReducedMotion,
  ScrollTrigger,
  SectionEyebrow,
} from '../../../shared';
import { LifecycleDiagram } from './LifecycleDiagrams';

interface FeatureOrbitalSectionProps {
  onSelectDetail: (detailId: string) => void;
}

const TOTAL_STAGES = LIFECYCLE_STAGES.length;
const DIAL_STEP_DEG = 28;

function applyDialNodeTransforms(
  nodeRefs: (HTMLButtonElement | null)[],
  continuousIndex: number,
  activeIdx: number
) {
  const half = TOTAL_STAGES / 2;
  const ringCenterX = -40;
  const ringCenterY = 230;
  const radius = 208;

  for (let idx = 0; idx < TOTAL_STAGES; idx++) {
    const el = nodeRefs[idx];
    if (!el) continue;

    let diff = idx - continuousIndex;
    if (diff > half) diff -= TOTAL_STAGES;
    if (diff < -half) diff += TOTAL_STAGES;

    const angleDeg = diff * DIAL_STEP_DEG;
    const rad = (angleDeg * Math.PI) / 180;
    const x = ringCenterX + Math.cos(rad) * radius;
    const y = ringCenterY + Math.sin(rad) * radius;
    const dist = Math.abs(diff);
    const isActive = idx === activeIdx;

    el.style.transform = `translate3d(${Math.round(x - 32)}px, ${Math.round(y - 16)}px, 0)`;
    el.style.opacity = isActive ? '0' : String(Math.max(0.32, 1 - dist * 0.24));
    el.style.pointerEvents = isActive ? 'none' : 'auto';
  }
}

/**
 * Level 03 Feature Widget: Scroll-Scrubbed 6-Stage Orbital Architecture Dial
 * Framed strictly inside the symmetrical GridBand with hairline-ruled columns,
 * GPU-composited scaleX progress rail, and direct DOM node positioning (zero 60Hz React re-renders).
 */
export function FeatureOrbitalSection({ onSelectDetail }: FeatureOrbitalSectionProps) {
  const sectionRef = useRef<HTMLElement | null>(null);
  const stageBodyRef = useRef<HTMLDivElement | null>(null);
  const dialRingRef = useRef<SVGSVGElement | null>(null);
  const activeHubRef = useRef<HTMLButtonElement | null>(null);
  const connectorBeamRef = useRef<HTMLDivElement | null>(null);
  const progressBarRef = useRef<HTMLDivElement | null>(null);
  const nodeRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const scrollTriggerRef = useRef<ScrollTrigger | null>(null);

  const [activeIndex, setActiveIndex] = useState(0);
  const activeIndexRef = useRef(0);
  const continuousIndexRef = useRef(0);

  const isProgrammaticScrollRef = useRef(false);
  const programmaticTimerRef = useRef<number | null>(null);

  const activeStage = LIFECYCLE_STAGES[activeIndex];

  const syncVisualMetrics = useCallback((progress: number, contIdx: number, nextActive: number) => {
    continuousIndexRef.current = contIdx;
    if (sectionRef.current) {
      sectionRef.current.dataset.scrollProgress = progress.toFixed(2);
    }
    if (progressBarRef.current) {
      const scale = Math.max(0.08, Math.min(1, (contIdx + 0.5) / TOTAL_STAGES));
      progressBarRef.current.style.transform = `scaleX(${scale.toFixed(4)})`;
    }
    applyDialNodeTransforms(nodeRefs.current, contIdx, nextActive);
  }, []);

  // Initialize node transforms on mount
  useEffect(() => {
    syncVisualMetrics(0, 0, 0);
  }, [syncVisualMetrics]);

  // Register GSAP ScrollTrigger and scrub all 6 stages automatically on desktop scroll
  useEffect(() => {
    ensureGsapRegistered();
    const section = sectionRef.current;
    if (!section) return;

    const reduced = prefersReducedMotion();
    const mm = gsap.matchMedia();

    mm.add('(min-width: 1024px)', () => {
      const trigger = ScrollTrigger.create({
        trigger: section,
        start: 'top top+=68',
        end: 'bottom bottom',
        scrub: 0.4,
        onUpdate: (self) => {
          const state = computeOrbitalScrollState(self.progress, TOTAL_STAGES, DIAL_STEP_DEG);
          const nextActive = isProgrammaticScrollRef.current
            ? activeIndexRef.current
            : state.activeIndex;

          syncVisualMetrics(self.progress, state.continuousIndex, nextActive);

          if (!isProgrammaticScrollRef.current && nextActive !== activeIndexRef.current) {
            activeIndexRef.current = nextActive;
            setActiveIndex(nextActive);
          }

          if (!reduced && dialRingRef.current) {
            gsap.to(dialRingRef.current, {
              rotation: state.ringRotationDeg,
              duration: 0.3,
              ease: 'power2.out',
              overwrite: 'auto',
            });
          }
        },
      });

      scrollTriggerRef.current = trigger;
      return () => {
        trigger.kill();
        scrollTriggerRef.current = null;
      };
    });

    return () => {
      mm.revert();
      if (programmaticTimerRef.current !== null) {
        window.clearTimeout(programmaticTimerRef.current);
      }
    };
  }, [syncVisualMetrics]);

  // Timestamped GSAP Core stage-switch choreography
  useEffect(() => {
    const body = stageBodyRef.current;
    if (!body) return;
    if (prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      const tl = gsap.timeline({ defaults: { ease: 'power3.out' } });

      if (activeHubRef.current) {
        tl.fromTo(
          activeHubRef.current,
          { scale: 0.88 },
          { scale: 1, duration: 0.32, ease: 'back.out(2)' },
          MOTION_TIMESTAMPS.orbitalPulse
        );
      }
      if (connectorBeamRef.current) {
        tl.fromTo(
          connectorBeamRef.current,
          { scaleX: 0.25, opacity: 0.4, transformOrigin: 'left center' },
          { scaleX: 1, opacity: 1, duration: 0.34 },
          MOTION_TIMESTAMPS.orbitalPulse
        );
      }

      tl.fromTo(
        '.orbital-stage-title',
        { opacity: 0, y: 14 },
        { opacity: 1, y: 0, duration: 0.34, stagger: 0.04, clearProps: 'transform' },
        MOTION_TIMESTAMPS.orbitalTitle
      );

      tl.fromTo(
        '.orbital-stage-diagram',
        { opacity: 0, x: 18, scale: 0.985 },
        { opacity: 1, x: 0, scale: 1, duration: 0.4, clearProps: 'transform' },
        MOTION_TIMESTAMPS.orbitalDiagram
      );

      tl.fromTo(
        '.orbital-stage-bullet',
        { opacity: 0, y: 10 },
        { opacity: 1, y: 0, duration: 0.3, stagger: 0.05, clearProps: 'transform' },
        MOTION_TIMESTAMPS.orbitalBullets
      );

      tl.fromTo(
        '.orbital-stage-link',
        { opacity: 0, y: 8 },
        { opacity: 1, y: 0, duration: 0.26, stagger: 0.035, clearProps: 'transform' },
        MOTION_TIMESTAMPS.orbitalLinks
      );
    }, body);

    return () => ctx.revert();
  }, [activeIndex]);

  const handleSelectStage = useCallback(
    (idx: number) => {
      const clampedIdx = Math.max(0, Math.min(TOTAL_STAGES - 1, idx));
      activeIndexRef.current = clampedIdx;
      setActiveIndex(clampedIdx);

      const targetProgress = getStageScrollProgress(clampedIdx, TOTAL_STAGES);
      syncVisualMetrics(targetProgress, clampedIdx, clampedIdx);

      if (!prefersReducedMotion() && dialRingRef.current) {
        gsap.to(dialRingRef.current, {
          rotation: -clampedIdx * DIAL_STEP_DEG,
          duration: 0.42,
          ease: 'power2.out',
          overwrite: 'auto',
        });
      }

      const trigger = scrollTriggerRef.current;
      if (trigger && window.innerWidth >= 1024) {
        isProgrammaticScrollRef.current = true;
        if (programmaticTimerRef.current !== null) {
          window.clearTimeout(programmaticTimerRef.current);
        }

        const targetY = trigger.start + targetProgress * (trigger.end - trigger.start);
        window.scrollTo({
          top: Math.round(targetY),
          behavior: prefersReducedMotion() ? 'auto' : 'smooth',
        });

        programmaticTimerRef.current = window.setTimeout(() => {
          isProgrammaticScrollRef.current = false;
          programmaticTimerRef.current = null;
        }, 650);
      }
    },
    [syncVisualMetrics]
  );

  return (
    <GridBand
      id="lifecycle"
      sectionRef={sectionRef}
      dataTestId="orbital-lifecycle-section"
      ariaLabel="Level 3 interactive orbital architecture"
      className="overflow-x-clip lg:min-h-[310vh]"
      dataAttrs={{
        'data-active-stage': activeStage.id,
        'data-active-index': activeIndex,
        'data-scroll-progress': '0.00',
      }}
    >
      {/* Sticky Full-Viewport Architectural Frame */}
      <div className="lg:sticky lg:top-[68px] lg:min-h-[calc(100vh-68px)] flex flex-col justify-between">
        <div>
          {/* Unified Hairline Eyebrow Bar */}
          <SectionEyebrow
            index="03"
            label="Scroll-Driven Terminal Lifecycle"
            rightMeta={`0${activeIndex + 1} / 0${TOTAL_STAGES} · ${activeStage.label.toUpperCase()}`}
          />

          {/* Centered Editorial Heading + Hairline Progress Track */}
          <div className="px-5 sm:px-8 lg:px-10 py-6 lg:py-8 border-b border-grid text-center">
            <h2 className="font-display text-[36px] sm:text-[44px] lg:text-[52px] font-semibold leading-[1.08] tracking-[-0.03em] text-chart-1 dark:text-chart-2 [text-wrap:balance]">
              Engineering the Calm Terminal Experience
            </h2>

            {/* Desktop GPU-Composited scaleX Scrub Progress Track */}
            <div className="mt-5 mx-auto max-w-[600px] hidden lg:block">
              <div className="relative h-[2px] w-full bg-border/80 overflow-hidden">
                <div
                  ref={progressBarRef}
                  className="h-full w-full bg-chart-1 dark:bg-chart-2 origin-left will-change-transform"
                  style={{ transform: 'scaleX(0.0833)' }}
                />
              </div>
              <div className="mt-2.5 grid grid-cols-6 border-t border-l border-grid font-tech text-[11px]">
                {LIFECYCLE_STAGES.map((stage, idx) => {
                  const isCurrent = idx === activeIndex;
                  const isPassed = idx < activeIndex;
                  return (
                    <button
                      key={stage.id}
                      type="button"
                      aria-pressed={isCurrent}
                      onClick={() => handleSelectStage(idx)}
                      className={`cursor-pointer border-r border-b border-grid py-1.5 px-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring ${
                        isCurrent
                          ? 'bg-primary text-primary-foreground font-semibold'
                          : isPassed
                            ? 'bg-card/60 text-chart-1 dark:text-chart-2 hover:bg-muted'
                            : 'bg-background text-muted-foreground hover:text-foreground hover:bg-muted/60'
                      }`}
                    >
                      0{idx + 1} {stage.label}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Mobile / Tablet Compact 3x2 Hairline Stage Grid */}
            <div
              role="tablist"
              aria-label="Architecture stages mobile"
              className="mt-4 grid lg:hidden grid-cols-3 border-t border-l border-grid font-tech text-[11.5px]"
            >
              {LIFECYCLE_STAGES.map((stage, idx) => {
                const selected = idx === activeIndex;
                return (
                  <button
                    key={stage.id}
                    type="button"
                    role="tab"
                    aria-selected={selected}
                    onClick={() => handleSelectStage(idx)}
                    className={`px-2 py-2 border-r border-b border-grid transition-colors cursor-pointer whitespace-nowrap text-center ${
                      selected
                        ? 'bg-primary text-primary-foreground font-semibold'
                        : 'bg-card text-muted-foreground hover:text-foreground'
                    }`}
                  >
                    0{idx + 1} {stage.label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        {/* Hairline-Divided 3-Zone Stage: Orbital Dial | Narrative | Stageshow Product Preview */}
        <div
          ref={stageBodyRef}
          className="relative flex-1 grid grid-cols-1 lg:grid-cols-12 lg:divide-x divide-grid items-center"
        >
          {/* Left Zone: Scroll-Rotated Concentric Orbital Dial */}
          <div
            role="tablist"
            aria-label="Architecture stages"
            className="hidden lg:flex lg:col-span-3 relative h-[460px] items-center overflow-hidden select-none"
          >
            <svg
              ref={dialRingRef}
              viewBox="0 0 420 420"
              aria-hidden="true"
              className="pointer-events-none absolute -left-[250px] top-1/2 -translate-y-1/2 w-[420px] h-[420px] text-border"
            >
              <circle
                cx="210"
                cy="210"
                r="208"
                fill="none"
                stroke="currentColor"
                strokeWidth="1"
              />
              <circle
                cx="210"
                cy="210"
                r="182"
                fill="none"
                stroke="currentColor"
                strokeWidth="1"
                strokeDasharray="4 6"
                opacity="0.7"
              />
              {Array.from({ length: 36 }).map((_, i) => {
                const a = (i * 10 * Math.PI) / 180;
                const r1 = i % 3 === 0 ? 196 : 202;
                const r2 = 208;
                return (
                  <line
                    key={i}
                    x1={210 + Math.cos(a) * r1}
                    y1={210 + Math.sin(a) * r1}
                    x2={210 + Math.cos(a) * r2}
                    y2={210 + Math.sin(a) * r2}
                    stroke="currentColor"
                    strokeWidth={i % 3 === 0 ? '1.5' : '1'}
                    opacity={i % 3 === 0 ? '0.9' : '0.45'}
                  />
                );
              })}
            </svg>

            <div
              className="pointer-events-none absolute -left-[250px] top-1/2 -translate-y-1/2 w-[420px] h-[420px] rounded-full shadow-[inset_0_0_60px_rgba(62,102,248,0.16)]"
              aria-hidden="true"
            />

            <div
              ref={connectorBeamRef}
              className="absolute left-[184px] top-1/2 -translate-y-1/2 h-px w-[120px] bg-gradient-to-r from-chart-1/85 dark:from-chart-2/90 via-border to-transparent"
              aria-hidden="true"
            />

            {/* Active 3-O'Clock Hub Button (advances sequence on click) */}
            <button
              ref={activeHubRef}
              type="button"
              role="tab"
              aria-selected={true}
              aria-label={activeStage.label}
              onClick={() => handleSelectStage((activeIndex + 1) % TOTAL_STAGES)}
              className="absolute left-[148px] top-1/2 -translate-y-1/2 z-20 w-10 h-10 rounded-[2px] bg-card border border-chart-1/85 dark:border-chart-2/85 shadow-lg flex items-center justify-center text-chart-1 dark:text-chart-2 cursor-pointer active:scale-[0.96] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <NikiMark className="w-5 h-5" />
            </button>

            {/* Smoothly Gliding Orbital Nodes positioned directly on DOM refs */}
            {LIFECYCLE_STAGES.map((stage, idx) => {
              const isCurrent = idx === activeIndex;
              return (
                <button
                  key={stage.id}
                  ref={(el) => {
                    nodeRefs.current[idx] = el;
                  }}
                  type="button"
                  role={isCurrent ? undefined : 'tab'}
                  aria-selected={isCurrent ? undefined : false}
                  aria-hidden={isCurrent || undefined}
                  tabIndex={isCurrent ? -1 : 0}
                  onClick={() => handleSelectStage(idx)}
                  className="absolute top-0 left-0 z-10 px-3.5 py-1.5 rounded-[2px] border border-grid-strong bg-card/95 font-tech text-[12px] text-muted-foreground hover:text-foreground hover:border-ring transition-[border-color,color] duration-150 cursor-pointer will-change-transform"
                >
                  {stage.label}
                </button>
              );
            })}
          </div>

          {/* Center Zone: Timestamped Active Stage Narrative & Normalized Spec Links */}
          <article
            key={activeStage.id}
            aria-live="polite"
            className="lg:col-span-4 px-5 sm:px-8 py-5 lg:py-10 space-y-4 lg:space-y-6"
          >
            <div className="space-y-3">
              <div className="orbital-stage-title inline-flex items-center gap-2 font-tech font-medium text-[12px] uppercase tracking-[0.12em] text-muted-foreground">
                <span className="text-chart-1 dark:text-chart-2 tabular-nums">
                  STAGE 0{activeIndex + 1}
                </span>
                <span aria-hidden="true">/</span>
                <span>0{TOTAL_STAGES}</span>
              </div>
              <h3 className="orbital-stage-title font-display text-[36px] sm:text-[42px] font-semibold leading-[1.08] tracking-[-0.03em] text-foreground">
                {activeStage.label}
              </h3>
              <p className="orbital-stage-title font-feature-heading text-chart-1 dark:text-chart-2">
                {activeStage.headline}
              </p>
            </div>

            <ul className="space-y-3.5 font-normal text-[16px] leading-[1.55] text-muted-foreground">
              {activeStage.bullets.map((bullet) => (
                <li key={bullet.lead} className="orbital-stage-bullet flex items-start gap-3">
                  <span
                    className="mt-2 h-1.5 w-1.5 rotate-45 bg-chart-1 dark:bg-chart-2 shrink-0"
                    aria-hidden="true"
                  />
                  <span>
                    <strong className="font-semibold text-foreground">{bullet.lead}</strong>{' '}
                    {bullet.text}
                  </span>
                </li>
              ))}
            </ul>

            <div className="pt-2 flex flex-wrap gap-2">
              {activeStage.links.map((link) => (
                <button
                  key={link.label}
                  type="button"
                  onClick={() => onSelectDetail(link.detailId)}
                  className={`orbital-stage-link ${BTN.base} ${BTN.subtle}`}
                >
                  <span>{link.label}</span>
                  <ArrowUpRight
                    className="w-3.5 h-3.5 text-chart-1 dark:text-chart-2"
                    aria-hidden="true"
                  />
                </button>
              ))}
            </div>
          </article>

          {/* Right Zone: Sequenced Stageshow Miniature Product UI */}
          <div
            key={`diag-${activeStage.id}`}
            className="orbital-stage-diagram lg:col-span-5 p-4 sm:p-7"
          >
            <LifecycleDiagram stageId={activeStage.id} />
          </div>
        </div>
      </div>
    </GridBand>
  );
}
