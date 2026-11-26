import React, { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, ArrowUpRight } from 'lucide-react';
import {
  BENCHMARK_STORIES,
  BenchmarkStory,
  PILLAR_CARDS,
  PillarCardData,
} from '../../../entities/niki';
import {
  BenchmarkCardHeader,
  BTN,
  CARD,
  ensureGsapRegistered,
  getNextCarouselIndex,
  GridBand,
  gsap,
  MOTION_TIMESTAMPS,
  prefersReducedMotion,
  SectionEyebrow,
} from '../../../shared';

interface FeatureBenchmarksSectionProps {
  onSelectPillar: (pillar: PillarCardData) => void;
  onSelectBenchmark: (story: BenchmarkStory) => void;
  onViewAllBenchmarks: () => void;
}

/**
 * Level 04 Feature Widget: Editorial Light-Band Proof Deck & Measured Craft
 * Completely normalized to the global symmetrical GridBand (replacing the old asymmetric
 * left-only rail), hairline-ruled 3-column decks, and normalized BTN controls.
 */
export function FeatureBenchmarksSection({
  onSelectPillar,
  onSelectBenchmark,
  onViewAllBenchmarks,
}: FeatureBenchmarksSectionProps) {
  const [activeSlide, setActiveSlide] = useState(0);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const trackRef = useRef<HTMLDivElement | null>(null);
  const themesCounterRef = useRef<HTMLSpanElement | null>(null);
  const modesCounterRef = useRef<HTMLSpanElement | null>(null);

  const handlePrev = () => {
    setActiveSlide((prev) => getNextCarouselIndex(prev, -1, BENCHMARK_STORIES.length));
  };

  const handleNext = () => {
    setActiveSlide((prev) => getNextCarouselIndex(prev, 1, BENCHMARK_STORIES.length));
  };

  // Whole-section GSAP ScrollTrigger choreography
  useEffect(() => {
    ensureGsapRegistered();
    const root = rootRef.current;
    if (!root || prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      const benchTl = gsap.timeline({
        defaults: { ease: 'power3.out' },
        scrollTrigger: {
          trigger: '#benchmarks',
          start: 'top 86%',
          once: true,
        },
      });
      benchTl
        .from(
          '.bench-seq-header',
          { opacity: 0, y: 18, duration: 0.48, stagger: 0.06, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.sectionHeader
        )
        .from(
          '.bench-seq-card',
          { opacity: 0, y: 20, duration: 0.5, stagger: 0.08, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.sectionGrid
        );

      gsap.from('.pillar-seq-card', {
        opacity: 0,
        y: 18,
        duration: 0.46,
        stagger: 0.07,
        ease: 'power3.out',
        clearProps: 'transform,opacity',
        scrollTrigger: {
          trigger: '#pillars',
          start: 'top 88%',
          once: true,
        },
      });

      const statsTl = gsap.timeline({
        defaults: { ease: 'power3.out' },
        scrollTrigger: {
          trigger: '#stats',
          start: 'top 88%',
          once: true,
        },
      });
      statsTl.from(
        '.stats-seq-item',
        { opacity: 0, y: 18, duration: 0.48, stagger: 0.08, clearProps: 'transform,opacity' },
        0
      );

      if (themesCounterRef.current) {
        const themesObj = { val: 1 };
        statsTl.to(
          themesObj,
          {
            val: 4,
            duration: 0.7,
            ease: 'power2.out',
            onUpdate: () => {
              if (themesCounterRef.current) {
                themesCounterRef.current.textContent = `${Math.round(themesObj.val)}`;
              }
            },
          },
          0.1
        );
      }

      if (modesCounterRef.current) {
        const modesObj = { val: 1 };
        statsTl.to(
          modesObj,
          {
            val: 3,
            duration: 0.7,
            ease: 'power2.out',
            onUpdate: () => {
              if (modesCounterRef.current) {
                modesCounterRef.current.textContent = `${Math.round(modesObj.val)}`;
              }
            },
          },
          0.16
        );
      }
    }, root);

    return () => ctx.revert();
  }, []);

  // Re-animate before/after bars whenever carousel slide changes
  useEffect(() => {
    const el = trackRef.current;
    if (!el || prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      gsap.fromTo(
        '.bench-bar-fill',
        { scaleX: 0.15, transformOrigin: 'left center' },
        { scaleX: 1, duration: 0.5, stagger: 0.05, ease: 'power2.out' }
      );
    }, el);

    return () => ctx.revert();
  }, [activeSlide]);

  const orderedStories = Array.from({ length: BENCHMARK_STORIES.length }, (_, offset) => {
    const idx = (activeSlide + offset) % BENCHMARK_STORIES.length;
    return { story: BENCHMARK_STORIES[idx], originalIdx: idx };
  });

  return (
    <div ref={rootRef} className="w-full relative">
      {/* Part A: Benchmarks Comparison Deck & Package Pillars inside Symmetrical GridBand */}
      <GridBand
        id="benchmarks"
        ariaLabel="Level 4 engineering benchmarks"
        topBorder={false}
      >
        <SectionEyebrow
          index="04"
          label="Verified Baselines & Modularity"
          rightMeta={`SLIDE 0${activeSlide + 1} / 0${BENCHMARK_STORIES.length}`}
        />

        {/* Editorial Section Header Cell */}
        <div className="px-5 sm:px-8 lg:px-10 py-9 lg:py-12 border-b border-grid flex flex-col sm:flex-row sm:items-end justify-between gap-6">
          <div className="bench-seq-header">
            <h2 className="font-display text-[36px] sm:text-[44px] lg:text-[52px] font-semibold leading-[1.08] tracking-[-0.03em] text-foreground max-w-[760px] [text-wrap:balance]">
              Every mechanism ships with a check
            </h2>
          </div>
          <button
            type="button"
            onClick={onViewAllBenchmarks}
            className={`bench-seq-header self-start sm:self-end ${BTN.base} ${BTN.primary}`}
          >
            <span>Inspect full benchmarks</span>
            <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
          </button>
        </div>

        {/* Hairline-Ruled 3-Column Benchmark Comparison Deck */}
        <div
          ref={trackRef}
          data-testid="benchmark-carousel-track"
          data-active-slide={activeSlide}
          className="grid grid-cols-1 md:grid-cols-3 divide-y md:divide-y-0 md:divide-x divide-grid border-b border-grid"
        >
          {orderedStories.slice(0, 3).map(({ story, originalIdx }) => (
            <button
              key={`${story.id}-${originalIdx}`}
              type="button"
              onClick={() => onSelectBenchmark(story)}
              aria-label={`${story.badge}: ${story.headline}`}
              className={`bench-seq-card group text-left p-6 sm:p-7 min-h-[296px] flex flex-col justify-between ${CARD.cellInteractive}`}
            >
              <div className="space-y-5">
                <BenchmarkCardHeader
                  id={story.id}
                  badge={story.badge}
                  metric={story.metric}
                />

                <div className="space-y-2.5 pt-1 font-tech text-[10.5px]">
                  <div className="space-y-1">
                    <div className="flex justify-between text-muted-foreground">
                      <span>Before</span>
                      <span className="tabular-nums truncate max-w-[60%] text-right">
                        {story.before}
                      </span>
                    </div>
                    <div className="h-1.5 rounded-[1px] bg-muted overflow-hidden">
                      <div
                        style={{ width: `${story.beforePct}%` }}
                        className="bench-bar-fill h-full bg-muted-foreground/40"
                      />
                    </div>
                  </div>
                  <div className="space-y-1">
                    <div className="flex justify-between text-foreground">
                      <span>Niki</span>
                      <span className="tabular-nums text-chart-1 truncate max-w-[60%] text-right">
                        {story.after}
                      </span>
                    </div>
                    <div className="h-1.5 rounded-[1px] bg-muted overflow-hidden">
                      <div
                        style={{ width: `${story.afterPct}%` }}
                        className="bench-bar-fill h-full bg-chart-1"
                      />
                    </div>
                  </div>
                </div>
              </div>

              <div className="mt-8 space-y-6">
                <p className="font-feature-heading text-foreground">
                  {story.headline}
                </p>
                <span className="inline-flex items-center gap-2 font-tech text-[12px] text-foreground group-hover:text-chart-1 transition-colors">
                  <span>Inspect Benchmark</span>
                  <ArrowUpRight
                    className="w-3.5 h-3.5 transition-transform group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
                    aria-hidden="true"
                  />
                </span>
              </div>
            </button>
          ))}
        </div>

        {/* Hairline Control Bar: GPU-scaled Slide Indicators + Normalized Square Arrows */}
        <div className="px-5 sm:px-8 lg:px-10 py-4 border-b border-grid flex items-center justify-between bg-background">
          <div className="flex items-center gap-2" role="tablist" aria-label="Benchmark slides">
            {BENCHMARK_STORIES.map((story, idx) => {
              const isActive = idx === activeSlide;
              return (
                <button
                  key={story.id}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
                  aria-label={`Go to slide ${idx + 1}`}
                  onClick={() => setActiveSlide(idx)}
                  className="p-1.5 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-[2px]"
                >
                  <span
                    className={`block w-2 h-2 rotate-45 transition-[transform,background-color] duration-150 ${
                      isActive
                        ? 'scale-110 bg-primary'
                        : 'scale-75 bg-muted-foreground/40 hover:bg-muted-foreground'
                    }`}
                  />
                </button>
              );
            })}
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handlePrev}
              aria-label="Previous slide"
              className={`${BTN.base} ${BTN.iconLg}`}
            >
              <ArrowLeft className="w-4 h-4" aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={handleNext}
              aria-label="Next slide"
              className={`${BTN.base} ${BTN.iconLg}`}
            >
              <ArrowRight className="w-4 h-4" aria-hidden="true" />
            </button>
          </div>
        </div>

        {/* Compact 3-Package Hairline-Ruled Deep-Dive Row */}
        <div
          id="pillars"
          className="grid grid-cols-1 md:grid-cols-3 divide-y md:divide-y-0 md:divide-x divide-grid"
        >
          {PILLAR_CARDS.map((card) => (
            <button
              key={card.id}
              type="button"
              onClick={() => onSelectPillar(card)}
              className={`pillar-seq-card group text-left p-6 sm:p-7 flex flex-col justify-between ${CARD.cellInteractive}`}
            >
              <div>
                <span className="font-tech font-medium text-[12px] text-chart-1">
                  {card.packageName}
                </span>
                <h3 className="mt-2 font-feature-heading text-foreground">
                  {card.title}
                </h3>
                <p className="mt-1.5 font-normal text-[16px] leading-[1.55] text-muted-foreground">
                  {card.subtitle}
                </p>
              </div>
              <span className="mt-5 inline-flex items-center gap-2 font-tech text-[12px] text-muted-foreground group-hover:text-foreground transition-colors">
                <span>{card.ctaLabel}</span>
                <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
              </span>
            </button>
          ))}
        </div>
      </GridBand>

      {/* Part B: Measured Craft Proof Stats Band */}
      <GridBand id="stats" ariaLabel="Measured craft metrics">
        <SectionEyebrow
          index="05"
          label="What Ships, Tested"
          rightMeta="WORKSPACE TESTS · NO API KEY"
        />

        <div className="px-5 sm:px-8 lg:px-10 py-8 lg:py-10 border-b border-grid">
          <h2 className="stats-seq-item font-display text-[36px] sm:text-[44px] lg:text-[52px] font-semibold leading-[1.08] tracking-[-0.03em] text-foreground max-w-[740px] [text-wrap:balance]">
            The mechanisms here have tests behind them
          </h2>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 divide-y md:divide-y-0 md:divide-x divide-grid">
          <div className="stats-seq-item p-6 sm:p-7 lg:p-8">
            <div className="font-display text-[52px] sm:text-[64px] lg:text-[70px] font-semibold leading-none tracking-[-0.03em] text-foreground tabular-nums">
              <span ref={themesCounterRef}>4</span>
            </div>
            <p className="mt-2.5 font-tech font-medium text-[12px] text-muted-foreground">
              Runtime themes, contrast checked in tests
            </p>
          </div>

          <div className="stats-seq-item p-6 sm:p-7 lg:p-8">
            <div className="flex items-center gap-4">
              <span className="font-display text-[52px] sm:text-[64px] lg:text-[70px] font-semibold leading-none tracking-[-0.03em] text-foreground tabular-nums">
                1
              </span>
              <span className="font-tech text-[17px] sm:text-[19px] leading-[1.25] text-foreground/80">
                Self-contained{' '}
                <br />
                Binary
              </span>
            </div>
            <p className="mt-2.5 font-tech font-medium text-[12px] text-muted-foreground">
              niki — the only entry point
            </p>
          </div>

          <div className="stats-seq-item p-6 sm:p-7 lg:p-8">
            <div className="flex items-center gap-4">
              <span
                ref={modesCounterRef}
                className="font-display text-[52px] sm:text-[64px] lg:text-[70px] font-semibold leading-none tracking-[-0.03em] text-foreground tabular-nums"
              >
                3
              </span>
              <span className="font-tech text-[17px] sm:text-[19px] leading-[1.25] text-foreground/80">
                Approval{' '}
                <br />
                Modes
              </span>
            </div>
            <p className="mt-2.5 font-tech text-[12.5px] text-muted-foreground">
              Manual → Auto → YOLO on Shift+Tab
            </p>
          </div>
        </div>
      </GridBand>
    </div>
  );
}
