import React, { useEffect, useRef, useState } from 'react';
import { ArrowUpRight, ChevronDown } from 'lucide-react';
import {
  BTN,
  EcosystemLogo,
  ensureGsapRegistered,
  gsap,
  MOTION_TIMESTAMPS,
  NikiMark,
  PixelatedShaderCanvas,
  prefersReducedMotion,
} from '../../../shared';
import {
  ECOSYSTEM_LOGOS_ROW_1,
  ECOSYSTEM_LOGOS_ROW_2,
  EcosystemLogoItem,
  HERO_FEATURED_LOGOS,
} from '../../../entities/niki';
import { HeroCommandBar } from '../../../features/quick-launch';

interface HeroSectionProps {
  isDark: boolean;
  onInstall: (customCommand?: string) => void;
  onSelectProvider: (item: EcosystemLogoItem) => void;
}

const HERO_LEVEL_LINKS = [
  { label: '01 · Core Surface Bento', href: '#bento-features', note: 'Composer, Install, Providers, Keymap' },
  { label: '02 · Scroll Parallax Craft', href: '#parallax-story', note: 'Binary, sessions, themes, approvals' },
  { label: '03 · 6-Stage Orbital Dial', href: '#lifecycle', note: 'Scroll-scrubbed /theme & CLI stages' },
  { label: '04 · Verified Baselines', href: '#benchmarks', note: 'Install · sessions · contrast · keymap' },
];

const MORE_ECOSYSTEM_COUNT =
  ECOSYSTEM_LOGOS_ROW_1.length - HERO_FEATURED_LOGOS.length + ECOSYSTEM_LOGOS_ROW_2.length;

export function HeroSection({
  isDark,
  onInstall,
  onSelectProvider,
}: HeroSectionProps) {
  const heroCopyRef = useRef<HTMLDivElement | null>(null);
  const switcherRef = useRef<HTMLDivElement | null>(null);
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const [showFullStack, setShowFullStack] = useState(false);

  useEffect(() => {
    ensureGsapRegistered();
    const el = heroCopyRef.current;
    if (!el || prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      const tl = gsap.timeline({ defaults: { ease: 'power3.out' } });
      tl.from(
        '.hero-seq-eyebrow',
        { opacity: 0, y: 14, duration: 0.5, clearProps: 'transform,opacity' },
        MOTION_TIMESTAMPS.heroEyebrow
      )
        .from(
          '.hero-seq-headline',
          { opacity: 0, y: 22, duration: 0.62, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.heroHeadline
        )
        .from(
          '.hero-seq-command',
          { opacity: 0, y: 16, scale: 0.985, duration: 0.54, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.heroCommand
        )
        .from(
          '.hero-seq-logos',
          { opacity: 0, y: 14, duration: 0.5, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.heroLogos
        );
    }, el);

    return () => ctx.revert();
  }, []);

  useEffect(() => {
    const handleOutside = (e: MouseEvent) => {
      if (switcherRef.current && !switcherRef.current.contains(e.target as Node)) {
        setSwitcherOpen(false);
      }
    };
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setSwitcherOpen(false);
    };
    document.addEventListener('mousedown', handleOutside);
    document.addEventListener('keydown', handleEsc);
    return () => {
      document.removeEventListener('mousedown', handleOutside);
      document.removeEventListener('keydown', handleEsc);
    };
  }, []);

  const renderLogoItem = (item: EcosystemLogoItem, framed = false) => {
    const interactive = Boolean(item.actionType);
    if (!interactive) {
      return (
        <li
          key={item.id}
          className={`flex items-center justify-center ${
            framed
              ? 'h-12 px-3 bg-card/95 border-r border-b border-grid'
              : 'h-12 px-3'
          }`}
        >
          <EcosystemLogo id={item.id} name={item.name} />
        </li>
      );
    }

    return (
      <li
        key={item.id}
        className={framed ? 'border-r border-b border-grid bg-card/95' : 'flex'}
      >
        <button
          type="button"
          onClick={() => onSelectProvider(item)}
          aria-label={`${item.name} — ${item.actionType}`}
          className={`group relative w-full h-12 px-3.5 inline-flex items-center justify-center transition-colors duration-150 hover:bg-muted/85 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring ${
            framed ? '' : 'rounded-[2px]'
          }`}
        >
          <span className="transition-opacity duration-150 group-hover:opacity-0">
            <EcosystemLogo id={item.id} name={item.name} />
          </span>
          <span className="pointer-events-none absolute inset-0 flex items-center justify-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150 font-tech text-[11.5px] font-medium text-foreground whitespace-nowrap">
            <span>{item.name}</span>
            <span className="text-chart-1 dark:text-chart-2">· {item.actionType}</span>
            <ArrowUpRight className="w-3.5 h-3.5 text-chart-1 dark:text-chart-2" aria-hidden="true" />
          </span>
        </button>
      </li>
    );
  };

  return (
    <section
      id="hero"
      aria-label="Meet your calm coding agent"
      className="relative w-full bg-background overflow-hidden transition-colors duration-200"
    >
      {/* Three.js GLSL Pixelated Atmospheric Plume Shader */}
      <PixelatedShaderCanvas isDark={isDark} />

      {/* Symmetrical Outer Hairline Grid Track Matching Entire Site */}
      <div className="relative z-10 arch-grid-band min-h-[780px] lg:min-h-[860px]">
        <div aria-hidden="true" className="border-r border-grid" />

        <div
          ref={heroCopyRef}
          className="min-w-0 flex flex-col justify-center items-center text-center px-5 sm:px-8 pt-28 pb-16 sm:pt-32 sm:pb-20 lg:pt-36 lg:pb-24"
        >
          {/* 1. Centered Interactive Eyebrow Switcher */}
          <div ref={switcherRef} className="hero-seq-eyebrow relative inline-flex flex-col items-center">
            <button
              type="button"
              aria-expanded={switcherOpen}
              onClick={() => setSwitcherOpen((v) => !v)}
              className="inline-flex items-center gap-2.5 rounded-[2px] border border-grid-strong bg-card/95 px-2.5 py-1.5 shadow-sm hover:border-ring transition-colors cursor-pointer active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <span className="inline-flex items-center justify-center w-6 h-6 rounded-[2px] bg-primary text-primary-foreground shrink-0">
                <NikiMark className="w-3.5 h-3.5" />
              </span>
              <span className="font-tech text-[12px] font-medium tracking-[0.14em] text-foreground whitespace-nowrap">
                NIKI AGENT
              </span>
              <ChevronDown
                className={`w-3.5 h-3.5 text-muted-foreground transition-transform duration-150 ${
                  switcherOpen ? 'rotate-180' : ''
                }`}
                aria-hidden="true"
              />
            </button>

            {switcherOpen && (
              <div
                role="menu"
                aria-label="Jump to feature level"
                className="absolute top-full mt-2 w-[300px] rounded-[2px] border border-grid-strong bg-popover text-popover-foreground divide-y divide-grid shadow-2xl text-left z-30"
              >
                {HERO_LEVEL_LINKS.map((item) => (
                  <a
                    key={item.href}
                    href={item.href}
                    role="menuitem"
                    onClick={() => setSwitcherOpen(false)}
                    className="block px-3.5 py-2.5 hover:bg-muted transition-colors"
                  >
                    <div className="font-tech text-[12px] font-medium text-foreground">{item.label}</div>
                    <div className="text-[12px] text-muted-foreground mt-0.5">{item.note}</div>
                  </a>
                ))}
              </div>
            )}
          </div>

          {/* 2. Centered Monumental 2-Line Headline (Geist Sans 600, 64–84px) */}
          <h1 className="hero-seq-headline mt-7 sm:mt-8 font-display text-[52px] sm:text-[68px] lg:text-[84px] font-semibold leading-[1.02] tracking-[-0.04em] text-foreground max-w-[840px] [text-wrap:balance]">
            Meet your{' '}
            <br />
            calm coding agent
          </h1>

          {/* Concise value-proposition subheadline for first-time visitors */}
          <p className="hero-seq-headline mt-5 max-w-[580px] rounded-[2px] bg-background/75 backdrop-blur-sm border border-grid/70 px-4 py-2 text-[15px] sm:text-[16px] leading-[1.55] text-foreground/90 shadow-sm">
            A terminal coding agent in one self-contained binary: four contrast-tested themes, session resume, and any model you already pay for. No account required.
          </p>

          {/* 3. Centered Architectural Quick-Launch Command Bar */}
          <div className="hero-seq-command mt-7 sm:mt-8 w-full">
            <HeroCommandBar onLaunchInstall={(cmd) => onInstall(cmd)} />
          </div>

          {/* 4. Centered High-Contrast Hairline-Ruled Authentic Brand Logo Bar */}
          <div className="hero-seq-logos mt-14 sm:mt-16 w-full max-w-[860px]">
            <ul
              aria-label="Featured model providers"
              className="grid grid-cols-2 sm:grid-cols-5 rounded-[2px] border-t border-l border-grid-strong bg-card/95 backdrop-blur-md shadow-lg overflow-hidden"
            >
              {HERO_FEATURED_LOGOS.map((item) => renderLogoItem(item, true))}
            </ul>

            {/* Expandable Full Provider + Runtime Stack Drawer */}
            <div className="mt-5 flex flex-col items-center">
              <button
                type="button"
                aria-expanded={showFullStack}
                onClick={() => setShowFullStack((v) => !v)}
                className={`${BTN.base} ${BTN.subtle} bg-card/95 border-grid-strong shadow-sm`}
              >
                <span>
                  {showFullStack
                    ? 'Hide full ecosystem stack'
                    : `+${MORE_ECOSYSTEM_COUNT} more providers & the runtime stack`}
                </span>
                <ChevronDown
                  className={`w-3.5 h-3.5 transition-transform duration-150 ${
                    showFullStack ? 'rotate-180' : ''
                  }`}
                  aria-hidden="true"
                />
              </button>

              {showFullStack && (
                <div className="mt-4 w-full rounded-[2px] border-t border-l border-grid-strong bg-card/95 backdrop-blur-md shadow-xl overflow-hidden">
                  <ul
                    aria-label="All supported model providers"
                    className="grid grid-cols-2 sm:grid-cols-5"
                  >
                    {ECOSYSTEM_LOGOS_ROW_1.map((item) => renderLogoItem(item, true))}
                  </ul>
                  <ul
                    aria-label="Core runtime stack and gateways"
                    className="grid grid-cols-2 sm:grid-cols-5"
                  >
                    {ECOSYSTEM_LOGOS_ROW_2.map((item) => renderLogoItem(item, true))}
                  </ul>
                </div>
              )}
            </div>
          </div>
        </div>

        <div aria-hidden="true" className="border-l border-grid" />
      </div>
    </section>
  );
}
