import React, { useCallback, useEffect, useRef, useState } from 'react';
import { isLightNavbarZone, resolveColorMode } from '../../../shared';
import {
  BENCHMARK_STORIES,
  BenchmarkStory,
  EcosystemLogoItem,
  PillarCardData,
} from '../../../entities/niki';
import { ActiveModal, InteractiveModals } from '../../../features/detail-modals';
import { Navbar } from '../../../widgets/navbar';
import { HeroSection } from '../../../widgets/hero';
import { FeatureBentoSection } from '../../../widgets/feature-bento';
import { FeatureParallaxSection } from '../../../widgets/feature-parallax';
import { FeatureOrbitalSection } from '../../../widgets/feature-orbital';
import { FeatureBenchmarksSection } from '../../../widgets/feature-benchmarks';
import { BottomCtaAndFooter } from '../../../widgets/cta-footer';

export function LandingPage() {
  const lightZoneRef = useRef<HTMLDivElement | null>(null);
  const [isLightNavbar, setIsLightNavbar] = useState(false);
  const [activeModal, setActiveModal] = useState<ActiveModal>(null);
  const [colorMode, setColorMode] = useState<'dark' | 'light'>(() => {
    if (typeof window === 'undefined') return 'dark';
    const saved = window.localStorage.getItem('niki-color-mode');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    return resolveColorMode(saved, prefersDark);
  });

  useEffect(() => {
    document.documentElement.classList.toggle('dark', colorMode === 'dark');
    try {
      window.localStorage.setItem('niki-color-mode', colorMode);
    } catch {
      // Ignore storage failures in restricted contexts
    }
  }, [colorMode]);

  useEffect(() => {
    const updateNavbarTheme = () => {
      const el = lightZoneRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const shouldBeLight = isLightNavbarZone(68, rect.top, rect.bottom);
      setIsLightNavbar((prev) => (prev === shouldBeLight ? prev : shouldBeLight));
    };

    updateNavbarTheme();
    window.addEventListener('scroll', updateNavbarTheme, { passive: true });
    window.addEventListener('resize', updateNavbarTheme);
    return () => {
      window.removeEventListener('scroll', updateNavbarTheme);
      window.removeEventListener('resize', updateNavbarTheme);
    };
  }, []);

  const handleToggleColorMode = useCallback(() => {
    setColorMode((prev) => (prev === 'dark' ? 'light' : 'dark'));
  }, []);
  const handleOpenInstall = useCallback((customCommand?: string) => {
    setActiveModal({ type: 'install', customCommand });
  }, []);
  const handleOpenChecklist = useCallback(() => {
    setActiveModal({ type: 'checklist' });
  }, []);
  const handleSelectDetail = useCallback((detailId: string) => {
    setActiveModal({ type: 'detail', detailId });
  }, []);
  const handleSelectProvider = useCallback((item: EcosystemLogoItem) => {
    setActiveModal({ type: 'provider', item });
  }, []);
  const handleOpenGithub = useCallback(() => {
    setActiveModal({ type: 'github' });
  }, []);
  const handleSelectPillar = useCallback((pillar: PillarCardData) => {
    setActiveModal({ type: 'pillar', pillar });
  }, []);
  const handleSelectBenchmark = useCallback((benchmark: BenchmarkStory) => {
    setActiveModal({ type: 'benchmark', benchmark });
  }, []);
  const handleViewAllBenchmarks = useCallback(() => {
    setActiveModal({ type: 'benchmark', benchmark: BENCHMARK_STORIES[0] });
  }, []);
  const handleCloseModal = useCallback(() => {
    setActiveModal(null);
  }, []);

  const isDark = colorMode === 'dark';

  return (
    <div
      data-testid="app-shell"
      data-color-mode={colorMode}
      className="min-h-screen bg-background text-foreground flex flex-col transition-colors duration-200"
    >
      <Navbar
        isLightTheme={isLightNavbar}
        colorMode={colorMode}
        onToggleColorMode={handleToggleColorMode}
        onOpenInstall={handleOpenInstall}
        onOpenChecklist={handleOpenChecklist}
        onSelectDetail={handleSelectDetail}
        onSelectProvider={handleSelectProvider}
        onOpenGithub={handleOpenGithub}
      />

      <main className="flex-1">
        {/* Hero: Centered Reference Structure + Three.js GLSL Pixelated Shader */}
        <HeroSection
          isDark={isDark}
          onInstall={handleOpenInstall}
          onSelectProvider={handleSelectProvider}
        />

        {/* Level 01: Asymmetric 12-Col Hairline Bento Grid */}
        <FeatureBentoSection onSelectDetail={handleSelectDetail} />

        {/* Level 02: Scroll-Driven Sticky 3-Depth Parallax Story */}
        <FeatureParallaxSection onSelectDetail={handleSelectDetail} />

        {/* Level 03: Interactive 6-Stage Orbital Architecture + /theme Switcher */}
        <FeatureOrbitalSection onSelectDetail={handleSelectDetail} />

        {/* Level 04: Editorial Light Contrast Band — Verified Baselines, Pillars & Proof Stats */}
        <div
          ref={lightZoneRef}
          className="surface-light-band w-full bg-background text-foreground border-t border-grid"
        >
          <FeatureBenchmarksSection
            onSelectPillar={handleSelectPillar}
            onSelectBenchmark={handleSelectBenchmark}
            onViewAllBenchmarks={handleViewAllBenchmarks}
          />
        </div>

        {/* Bottom CTA & Footer */}
        <BottomCtaAndFooter
          isDark={isDark}
          onInstall={handleOpenInstall}
          onOpenGithub={handleOpenGithub}
          onSelectDetail={handleSelectDetail}
        />
      </main>

      <InteractiveModals
        modal={activeModal}
        onClose={handleCloseModal}
        onOpenInstall={handleOpenInstall}
      />
    </div>
  );
}
