import React, { useEffect, useRef, useState } from 'react';
import { ArrowUpRight } from 'lucide-react';
import { PARALLAX_CHAPTERS, ParallaxChapter } from '../../../entities/niki';
import {
  CARD,
  computeActiveChapterIndex,
  computeParallaxOffset,
  ensureGsapRegistered,
  GridBand,
  gsap,
  MOTION_TIMESTAMPS,
  prefersReducedMotion,
  ScrollTrigger,
  SectionEyebrow,
  TerminalTitleBar,
} from '../../../shared';

interface FeatureParallaxSectionProps {
  onSelectDetail: (detailId: string) => void;
}

/**
 * Level 02 Feature Widget: Scroll-Linked Sticky Parallax Story
 * Tiled directly inside the symmetrical GridBand with flush hairline-ruled chapter cells
 * and a sticky 3-depth Stageshow viewport. Zero per-pixel React scroll re-renders.
 */
export function FeatureParallaxSection({ onSelectDetail }: FeatureParallaxSectionProps) {
  const sectionRef = useRef<HTMLElement | null>(null);
  const stageRef = useRef<HTMLDivElement | null>(null);
  const bgLayerRef = useRef<HTMLDivElement | null>(null);
  const mainLayerRef = useRef<HTMLDivElement | null>(null);
  const satLayerRef = useRef<HTMLDivElement | null>(null);

  const [activeIndex, setActiveIndex] = useState(0);
  const manualOverrideRef = useRef(false);
  const manualOverrideTimerRef = useRef<number | null>(null);

  // Interactive toggles for Chapter 03 (Shell-native accessibility)
  const [a11yFlags, setA11yFlags] = useState({
    reducedMotion: false,
    mono: false,
    ascii: false,
  });

  // Timestamped GSAP ScrollTrigger entrance + continuous 3-depth parallax scrub
  useEffect(() => {
    ensureGsapRegistered();
    const el = sectionRef.current;
    if (!el) return;
    const reduced = prefersReducedMotion();

    const ctx = gsap.context(() => {
      if (!reduced) {
        const introTl = gsap.timeline({
          defaults: { ease: 'power3.out' },
          scrollTrigger: {
            trigger: el,
            start: 'top 90%',
            once: true,
          },
        });

        introTl
          .from(
            '.parallax-seq-header',
            { opacity: 0, y: 16, duration: 0.44, immediateRender: false, clearProps: 'transform,opacity' },
            MOTION_TIMESTAMPS.sectionHeader
          )
          .from(
            '.parallax-seq-chapter',
            { opacity: 0, x: -12, duration: 0.4, stagger: 0.06, immediateRender: false, clearProps: 'transform,opacity' },
            MOTION_TIMESTAMPS.sectionGrid
          )
          .from(
            '.parallax-seq-viewport',
            { opacity: 0, y: 18, duration: 0.48, immediateRender: false, clearProps: 'transform,opacity' },
            MOTION_TIMESTAMPS.sectionStage
          );
      }

      ScrollTrigger.create({
        trigger: el,
        start: 'top 65%',
        end: 'bottom 45%',
        scrub: 0.45,
        onUpdate: (self) => {
          const progress = self.progress;
          // Direct DOM attribute update avoids 60Hz React re-renders on scroll
          el.dataset.scrollProgress = progress.toFixed(2);

          if (!manualOverrideRef.current && self.isActive) {
            const nextIdx = computeActiveChapterIndex(progress, PARALLAX_CHAPTERS.length);
            setActiveIndex((prev) => (prev === nextIdx ? prev : nextIdx));
          }

          if (!reduced) {
            const bgY = computeParallaxOffset(progress, -0.55, 28);
            const mainY = computeParallaxOffset(progress, 0.25, 22);
            const satY = computeParallaxOffset(progress, 1.0, 34);

            if (bgLayerRef.current) {
              gsap.to(bgLayerRef.current, { y: bgY, duration: 0.3, ease: 'power2.out', overwrite: 'auto' });
            }
            if (mainLayerRef.current) {
              gsap.to(mainLayerRef.current, { y: mainY, duration: 0.3, ease: 'power2.out', overwrite: 'auto' });
            }
            if (satLayerRef.current) {
              gsap.to(satLayerRef.current, { y: satY, duration: 0.3, ease: 'power2.out', overwrite: 'auto' });
            }
          }
        },
      });
    }, el);

    return () => {
      ctx.revert();
      if (manualOverrideTimerRef.current !== null) {
        window.clearTimeout(manualOverrideTimerRef.current);
      }
    };
  }, []);

  // Timestamped GSAP Core chapter transition choreography
  useEffect(() => {
    const el = stageRef.current;
    if (!el || prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      const tl = gsap.timeline({ defaults: { ease: 'power2.out' } });
      if (mainLayerRef.current) {
        tl.fromTo(
          mainLayerRef.current,
          { opacity: 0.72, scale: 0.985 },
          { opacity: 1, scale: 1, duration: 0.3, clearProps: 'scale' },
          0
        );
      }
      if (satLayerRef.current) {
        tl.fromTo(
          satLayerRef.current,
          { opacity: 0.55, x: 10 },
          { opacity: 1, x: 0, duration: 0.32, clearProps: 'x' },
          0.07
        );
      }
    }, el);

    return () => ctx.revert();
  }, [activeIndex]);

  const handleSelectChapter = (idx: number) => {
    manualOverrideRef.current = true;
    setActiveIndex(idx);
    if (manualOverrideTimerRef.current !== null) {
      window.clearTimeout(manualOverrideTimerRef.current);
    }
    manualOverrideTimerRef.current = window.setTimeout(() => {
      manualOverrideRef.current = false;
      manualOverrideTimerRef.current = null;
    }, 1800);
  };

  const activeChapter: ParallaxChapter = PARALLAX_CHAPTERS[activeIndex];

  return (
    <GridBand
      id="parallax-story"
      sectionRef={sectionRef}
      dataTestId="parallax-story-section"
      ariaLabel="Level 2 scroll parallax engineering depth"
      dataAttrs={{
        'data-active-chapter': activeChapter.id,
        'data-scroll-progress': '0.00',
      }}
    >
      {/* Unified Hairline Eyebrow Bar */}
      <SectionEyebrow
        index="02"
        label="Layered Performance Engineering"
        rightMeta={`${activeChapter.indexLabel} / 04 · ${activeChapter.metricValue}`}
      />

      {/* Editorial Header Cell */}
      <div className="parallax-seq-header px-5 sm:px-8 lg:px-10 py-12 lg:py-16 border-b border-grid">
        <h2 className="font-display text-[36px] sm:text-[44px] lg:text-[52px] font-semibold leading-[1.08] tracking-[-0.03em] text-foreground max-w-[740px] [text-wrap:balance]">
          Engineered as one binary, from turn loop to theme
        </h2>
      </div>

      {/* Symmetrical Hairline Split: Left Ruled Chapter Rail + Right Sticky Depth Stage */}
      <div className="grid grid-cols-1 lg:grid-cols-12 lg:divide-x divide-grid items-stretch">
        {/* Left Column: 4 Flush Hairline-Ruled Story Chapters */}
        <div
          className="lg:col-span-5 divide-y divide-grid border-b lg:border-b-0 border-grid"
          role="tablist"
          aria-label="Performance craft chapters"
        >
          {PARALLAX_CHAPTERS.map((chapter, idx) => {
            const isSelected = idx === activeIndex;
            return (
              <button
                key={chapter.id}
                type="button"
                role="tab"
                aria-selected={isSelected}
                onClick={() => handleSelectChapter(idx)}
                className={`parallax-seq-chapter relative w-full text-left p-6 sm:p-7 transition-colors duration-150 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring ${
                  isSelected
                    ? 'bg-card text-foreground'
                    : 'bg-background/40 hover:bg-card/70 text-foreground/75 hover:text-foreground'
                }`}
              >
                {/* Thin 2px active chapter indicator bar echoing Footer index bar */}
                {isSelected && (
                  <span
                    aria-hidden="true"
                    className="pointer-events-none absolute left-0 top-0 bottom-0 w-[2px] bg-primary"
                  />
                )}
                <div className="flex items-center justify-between gap-3">
                  <span className="font-tech font-medium text-[12px] text-chart-1 dark:text-chart-2 tabular-nums">
                    {chapter.indexLabel} · {chapter.metricValue}
                  </span>
                  <span className="font-tech font-medium text-[11px] text-muted-foreground">
                    {chapter.metricLabel}
                  </span>
                </div>
                <h3 className="mt-2 font-feature-heading text-foreground">
                  {chapter.title}
                </h3>
                <p className="mt-2 font-normal text-[16px] leading-[1.55] text-muted-foreground">
                  {chapter.summary}
                </p>
              </button>
            );
          })}
        </div>

        {/* Right Column: Sticky 3-Layer Parallax Stageshow Viewport */}
        <div className="parallax-seq-viewport lg:col-span-7 p-5 sm:p-8 lg:p-10 bg-background">
          <div className="lg:sticky lg:top-24">
            <div
              ref={stageRef}
              className={`${CARD.stageFrame} w-full h-[395px] sm:h-[430px] border shadow-xl`}
            >
              <div
                className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_65%_20%,rgba(62,102,248,0.18),transparent_62%)]"
                aria-hidden="true"
              />

              {/* Z-Layer 1 (Back Parallax Plane) */}
              <div
                ref={bgLayerRef}
                aria-hidden="true"
                className="parallax-scene-enter pointer-events-none absolute left-6 top-6 right-12 bottom-16 rounded-[2px] border border-[#222638]/80 bg-[#0b0d14]/85 p-5 font-tech opacity-70"
              >
                <div className="space-y-1.5 text-[10px] text-[#556480]">
                  <div className="truncate"># rust/crates/niki-tui-themes/src/lib.rs</div>
                  <div className="truncate">palettes = 4 · state = ~/.niki</div>
                  <div className="truncate">contrast_floor_text = 4.5 · glyph = 3.0</div>
                </div>
              </div>

              {/* Z-Layer 2 (Mid Parallax Plane): Cropped Primary Stageshow Terminal */}
              <div
                ref={mainLayerRef}
                className="parallax-scene-enter absolute left-10 sm:left-14 top-12 -right-6 -bottom-6 rounded-tl-[2px] border-t border-l border-[#2a314a] bg-[#0d1018] shadow-[-12px_20px_50px_rgba(0,0,0,0.75)] overflow-hidden flex flex-col"
              >
                {activeChapter.id === 'binary' && (
                  <>
                    <TerminalTitleBar title="niki · release build" rightLabel="one binary" />
                    <div className="p-6 flex-1 flex flex-col justify-between font-tech">
                      <div className="space-y-4 max-w-[380px]">
                        <div>
                          <div className="flex justify-between text-[11px] text-[#9eb0c7] mb-1.5">
                            <span>Runtime + package manager</span>
                            <span className="tabular-nums">setup</span>
                          </div>
                          <div className="h-2 rounded-[1px] bg-[#131b29] overflow-hidden">
                            <div className="h-full w-full bg-[#4a6284]" />
                          </div>
                        </div>
                        <div>
                          <div className="flex justify-between text-[11px] text-[#eef4fa] mb-1.5">
                            <span>Niki Agent binary</span>
                            <span className="text-[#9ec8f8] tabular-nums">1 file</span>
                          </div>
                          <div className="h-2 rounded-[1px] bg-[#131b29] overflow-hidden">
                            <div className="h-full w-[12%] bg-[#9ec8f8]" />
                          </div>
                        </div>
                        <div className="pt-1 space-y-1 text-[10.5px] text-[#9eb0c7]">
                          <div className="truncate">
                            <span className="text-[#9ec8f8] mr-1.5">✓</span>cargo build --release -p niki --bin niki
                          </div>
                          <div className="truncate">
                            <span className="text-[#9ec8f8] mr-1.5">✓</span>SHA256SUMS verified before install
                          </div>
                        </div>
                      </div>

                      <div className="pb-10 space-y-2.5">
                        <div className="h-px w-full bg-[#384b68]" />
                        <div className="text-[12px] text-[#dce7f5]">
                          <span className="text-[#9ec8f8] mr-2">›</span>./niki · ready
                        </div>
                      </div>
                    </div>
                  </>
                )}

                {activeChapter.id === 'sessions' && (
                  <>
                    <TerminalTitleBar title="sessions · sqlite" rightLabel="/resume" />
                    <div className="p-6 space-y-3 font-tech">
                      <div className="rounded-[2px] border border-dashed border-[#2f415a] bg-[#141d2b] px-4 py-2.5 opacity-60 max-w-[380px]">
                        <div className="text-[10.5px] text-[#8ea0b8]">Older turns summarised by /compact · store intact</div>
                      </div>
                      <div className="rounded-[2px] border border-[#9ec8f8]/60 bg-[#23354f] px-4 py-3.5 max-w-[380px] space-y-2">
                        <div className="flex justify-between text-[11.5px]">
                          <span className="text-[#f0f5fa] font-medium">Session 7f3a · 128 turns</span>
                          <span className="text-[#9ec8f8] tabular-nums">resumable</span>
                        </div>
                        <div className="text-[10.5px] text-[#dce7f5] truncate">
                          + write_file rust/crates/niki-tui/src/lib.rs
                        </div>
                        <div className="text-[10.5px] text-[#b8cde4]">Resume by id or index · pageup/end still works</div>
                      </div>
                      <div className="rounded-[2px] border border-[#31425b] bg-[#1c273a] px-4 py-2.5 max-w-[380px]">
                        <div className="text-[10.5px] text-[#dce7f5]">New session · streaming turn 1</div>
                      </div>
                    </div>
                  </>
                )}

                {activeChapter.id === 'accessibility' && (
                  <>
                    <TerminalTitleBar
                      title={a11yFlags.ascii ? 'niki [ASCII]' : '◐ niki · env fallbacks'}
                      rightLabel={a11yFlags.mono ? 'theme: mono' : 'WCAG AA'}
                    />
                    <div className="p-6 flex-1 flex flex-col justify-between font-tech">
                      <div className="space-y-3 max-w-[390px]">
                        <div
                          className={`rounded-[2px] border px-4 py-3 space-y-2 transition-colors ${
                            a11yFlags.mono
                              ? 'border-[#7a828c] bg-[#181a1d] text-[#e5e7eb]'
                              : 'border-[#3d5170] bg-[#1f2c40] text-[#eef4fa]'
                          }`}
                        >
                          <div className="flex items-center justify-between text-[11px]">
                            <span>{a11yFlags.ascii ? '[*] Phase indicator' : '◐ Phase indicator'}</span>
                            <span className={a11yFlags.mono ? 'text-[#d1d5db]' : 'text-[#9ec8f8]'}>
                              {a11yFlags.reducedMotion ? 'Static' : 'Adaptive'}
                            </span>
                          </div>
                          <div className="text-[11px] opacity-85">
                            {a11yFlags.ascii
                              ? '-----------------------------------'
                              : 'Single hairline rule · contrast measured in tests'}
                          </div>
                        </div>

                        <div className="flex flex-wrap gap-1.5">
                          <button
                            type="button"
                            aria-pressed={a11yFlags.reducedMotion}
                            onClick={() =>
                              setA11yFlags((f) => ({ ...f, reducedMotion: !f.reducedMotion }))
                            }
                            className={`px-2.5 py-1 rounded-[2px] border text-[10px] cursor-pointer transition-colors ${
                              a11yFlags.reducedMotion
                                ? 'border-[#9ec8f8] bg-[#2a4060] text-[#f0f5fa]'
                                : 'border-[#354965] bg-[#151e2e] text-[#9eb0c7]'
                            }`}
                          >
                            NIKI_REDUCED_MOTION={a11yFlags.reducedMotion ? '1' : '0'}
                          </button>
                          <button
                            type="button"
                            aria-pressed={a11yFlags.mono}
                            onClick={() => setA11yFlags((f) => ({ ...f, mono: !f.mono }))}
                            className={`px-2.5 py-1 rounded-[2px] border text-[10px] cursor-pointer transition-colors ${
                              a11yFlags.mono
                                ? 'border-[#d1d5db] bg-[#2f3339] text-white'
                                : 'border-[#354965] bg-[#151e2e] text-[#9eb0c7]'
                            }`}
                          >
                            theme={a11yFlags.mono ? 'mono' : 'niki'}
                          </button>
                          <button
                            type="button"
                            aria-pressed={a11yFlags.ascii}
                            onClick={() => setA11yFlags((f) => ({ ...f, ascii: !f.ascii }))}
                            className={`px-2.5 py-1 rounded-[2px] border text-[10px] cursor-pointer transition-colors ${
                              a11yFlags.ascii
                                ? 'border-[#9ec8f8] bg-[#2a4060] text-[#f0f5fa]'
                                : 'border-[#354965] bg-[#151e2e] text-[#9eb0c7]'
                            }`}
                          >
                            ASCII={a11yFlags.ascii ? '1' : '0'}
                          </button>
                        </div>

                        <div className="rounded-[2px] border border-[#2b3b54] bg-[#141c2b]/90 px-3.5 py-2.5 space-y-1.5 text-[10px]">
                          <div className="flex items-center justify-between text-[#c6d6ea]">
                            <span>text / surface</span>
                            <span className="tabular-nums text-[#eef4fa]">≥ 4.5:1</span>
                          </div>
                          <div className="flex items-center justify-between text-[#a6bad2]">
                            <span>glyph / surface</span>
                            <span className="tabular-nums text-[#eef4fa]">≥ 3:1</span>
                          </div>
                        </div>
                      </div>

                      <div className="pb-10 space-y-2">
                        <div className="h-px w-full bg-[#31435d]" />
                        <div className="text-[11px] text-[#b4c7de]">
                          {a11yFlags.ascii ? '> NIKI_ASCII=1 niki' : '› niki'}
                        </div>
                      </div>
                    </div>
                  </>
                )}

                {activeChapter.id === 'approvals' && (
                  <>
                    <TerminalTitleBar title="approval · policy" rightLabel="shift+tab" />
                    <div className="p-6 flex-1 flex flex-col justify-between font-tech text-[11.5px]">
                      <div className="space-y-2.5 max-w-[390px]">
                        <div className="rounded-[2px] border border-[#3d5170] bg-[#202f45] px-4 py-2.5 space-y-1">
                          <div className="text-[#9ec8f8]">manual</div>
                          <div className="text-[#eef4fa]">Ask before every gated tool call</div>
                        </div>
                        <div className="rounded-[2px] border border-[#2f4059] bg-[#162030] px-4 py-2.5 space-y-1">
                          <div className="text-[#9eb0c7]">auto</div>
                          <div className="text-[#dce7f5]">Skip the prompt only for the shell allow-list</div>
                        </div>
                        <div className="rounded-[2px] border border-[#3d5170] bg-[#202f45] px-4 py-2.5 space-y-1">
                          <div className="text-[#9ec8f8]">yolo</div>
                          <div className="text-[#eef4fa]">Allow everything the policy does not refuse</div>
                        </div>
                        <div className="pt-1 space-y-1 text-[10.5px] text-[#9eb0c7]">
                          <div className="truncate">
                            <span className="text-[#9ec8f8] mr-1.5">+</span>y approve · n reject
                          </div>
                          <div className="truncate">
                            <span className="text-[#9ec8f8] mr-1.5">=</span>workspace path boundary
                          </div>
                        </div>
                      </div>
                      <div className="pb-10 text-[11px] text-[#9eb0c7]">
                        › paths outside the workspace root are refused
                      </div>
                    </div>
                  </>
                )}
              </div>

              {/* Z-Layer 3 (Front Parallax Satellite) */}
              <div
                ref={satLayerRef}
                className="parallax-scene-enter absolute right-4 bottom-4 z-20 rounded-[2px] border border-[#313a5a] bg-[#101420]/96 px-4 py-3.5 shadow-2xl font-tech min-w-[205px] space-y-2"
              >
                <div className="text-[10px] text-[#5db8ff]">{activeChapter.indexLabel} · Verified</div>
                <div className="text-[20px] font-display font-semibold text-[#f4f6fc] leading-none tabular-nums">
                  {activeChapter.metricValue}
                </div>
                <div className="text-[10.5px] text-[#9ea9bf]">{activeChapter.metricLabel}</div>
                <button
                  type="button"
                  onClick={() => onSelectDetail(activeChapter.detailId)}
                  className="pt-1 inline-flex items-center gap-1 text-[10.5px] text-[#5db8ff] hover:underline cursor-pointer"
                >
                  <span>Inspect spec</span>
                  <ArrowUpRight className="w-3 h-3" aria-hidden="true" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </GridBand>
  );
}
