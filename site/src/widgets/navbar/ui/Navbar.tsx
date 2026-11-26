import React, { useCallback, useEffect, useRef, useState } from 'react';
import gsap from 'gsap';
import {
  Activity,
  ArrowUpRight,
  BookOpen,
  ChevronDown,
  FileCode2,
  FileText,
  GitBranch,
  KeyRound,
  Layers,
  ListChecks,
  Menu,
  Play,
  ShieldCheck,
  Terminal,
  X,
} from 'lucide-react';
import { BTN, CARD, isNavbarAttached, Keycap, NikiMark, ProviderIcon } from '../../../shared';
import {
  ECOSYSTEM_LOGOS_ROW_1,
  ECOSYSTEM_LOGOS_ROW_2,
  EcosystemLogoItem,
  FOOTER_COLUMNS,
  MEGA_MENU_BASELINES,
  MEGA_MENU_CAPABILITIES,
  MEGA_MENU_GUIDES,
  MEGA_MENU_LEVELS,
  MEGA_MENU_PROVIDER_PAIRINGS,
  MEGA_MENU_TOP_PROVIDERS,
  NIKI_THEMES,
} from '../../../entities/niki';
import { ThemeToggleButton } from '../../../features/theme-switch';

type MegaMenuId = 'providers' | 'architecture' | 'resources';

interface NavbarProps {
  isLightTheme: boolean;
  colorMode: 'dark' | 'light';
  onToggleColorMode: () => void;
  onOpenInstall: (customCommand?: string) => void;
  onOpenChecklist: () => void;
  onSelectDetail: (detailId: string) => void;
  onSelectProvider: (item: EcosystemLogoItem) => void;
  onOpenGithub: () => void;
}

const ALL_ECOSYSTEM_ITEMS: EcosystemLogoItem[] = [
  ...ECOSYSTEM_LOGOS_ROW_1,
  ...ECOSYSTEM_LOGOS_ROW_2,
];

function CapabilityIcon({ icon }: { icon: 'motion' | 'terminal' | 'shield' | 'layers' }) {
  switch (icon) {
    case 'motion':
      return <Activity className="w-4 h-4" aria-hidden="true" />;
    case 'terminal':
      return <Terminal className="w-4 h-4" aria-hidden="true" />;
    case 'shield':
      return <ShieldCheck className="w-4 h-4" aria-hidden="true" />;
    case 'layers':
      return <Layers className="w-4 h-4" aria-hidden="true" />;
  }
}

export function Navbar({
  isLightTheme,
  colorMode,
  onToggleColorMode,
  onOpenInstall,
  onOpenChecklist,
  onSelectDetail,
  onSelectProvider,
  onOpenGithub,
}: NavbarProps) {
  const [openMenu, setOpenMenu] = useState<MegaMenuId | null>(null);
  const [mobileOpen, setMobileOpen] = useState(false);
  // 1-bit scroll threshold state: avoids re-rendering the Navbar on every scroll pixel
  const [scrolled, setScrolled] = useState(false);

  const navRootRef = useRef<HTMLDivElement | null>(null);
  const megaPanelRef = useRef<HTMLDivElement | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const hoverOpenedRef = useRef<{ menu: MegaMenuId; at: number } | null>(null);

  const cancelCloseTimer = useCallback(() => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  }, []);

  const scheduleClose = useCallback(() => {
    cancelCloseTimer();
    closeTimerRef.current = window.setTimeout(() => {
      setOpenMenu(null);
      hoverOpenedRef.current = null;
      closeTimerRef.current = null;
    }, 160);
  }, [cancelCloseTimer]);

  const openMegaMenu = useCallback(
    (menu: MegaMenuId) => {
      cancelCloseTimer();
      setOpenMenu((prev) => {
        if (prev !== menu) {
          hoverOpenedRef.current = { menu, at: performance.now() };
        }
        return menu;
      });
    },
    [cancelCloseTimer]
  );

  const toggleMegaMenu = useCallback(
    (menu: MegaMenuId) => {
      cancelCloseTimer();
      setOpenMenu((prev) => {
        if (prev === menu) {
          const justOpenedByHover =
            hoverOpenedRef.current &&
            hoverOpenedRef.current.menu === menu &&
            performance.now() - hoverOpenedRef.current.at < 400;
          if (justOpenedByHover) {
            hoverOpenedRef.current = null;
            return menu;
          }
          hoverOpenedRef.current = null;
          return null;
        }
        hoverOpenedRef.current = null;
        return menu;
      });
    },
    [cancelCloseTimer]
  );

  useEffect(() => {
    const onScroll = () => {
      const next = isNavbarAttached(window.scrollY, false, 24);
      setScrolled((prev) => (prev === next ? prev : next));
    };
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (navRootRef.current && !navRootRef.current.contains(e.target as Node)) {
        setOpenMenu(null);
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpenMenu(null);
        setMobileOpen(false);
        return;
      }

      const target = e.target as HTMLElement | null;
      const isEditable =
        target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable);

      if (!isEditable && !e.metaKey && !e.ctrlKey && !e.altKey && e.key.toLowerCase() === 'i') {
        e.preventDefault();
        setOpenMenu(null);
        onOpenInstall();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      cancelCloseTimer();
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [cancelCloseTimer, onOpenInstall]);

  // GSAP Mega Menu entrance & smooth panel-switch stagger
  useEffect(() => {
    const panel = megaPanelRef.current;
    if (!openMenu || !panel) return;
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;

    const ctx = gsap.context(() => {
      gsap.fromTo(
        panel,
        { opacity: 0, y: -6 },
        { opacity: 1, y: 0, duration: 0.2, ease: 'power2.out', clearProps: 'transform' }
      );
      gsap.fromTo(
        panel.querySelectorAll('.mega-col'),
        { opacity: 0, y: 8 },
        {
          opacity: 1,
          y: 0,
          duration: 0.24,
          stagger: 0.04,
          ease: 'power2.out',
          clearProps: 'transform',
        }
      );
    }, panel);

    return () => ctx.revert();
  }, [openMenu]);

  const effectiveLight = colorMode === 'light' || isLightTheme;
  const isAttached = isNavbarAttached(scrolled ? 25 : 0, Boolean(openMenu) || mobileOpen, 24);

  const handleOpenProviderTile = (providerId: string) => {
    setOpenMenu(null);
    const match = ALL_ECOSYSTEM_ITEMS.find((item) => item.id === providerId);
    if (match) {
      onSelectProvider(match);
    } else {
      onSelectDetail('providers');
    }
  };

  const renderTrigger = (id: MegaMenuId, label: string) => {
    const isActive = openMenu === id;
    return (
      <button
        type="button"
        aria-expanded={isActive}
        aria-controls={`megamenu-${id}`}
        onClick={() => toggleMegaMenu(id)}
        onMouseEnter={() => openMegaMenu(id)}
        className={`group ${BTN.base} ${BTN.ghost} ${
          isActive
            ? 'bg-muted border-grid-strong text-foreground'
            : isAttached
              ? 'border-transparent text-foreground/80 hover:text-foreground hover:bg-muted/70'
              : 'border-transparent text-foreground/80 hover:text-foreground hover:bg-foreground/[0.06]'
        }`}
      >
        <span>{label}</span>
        <span
          className={`inline-flex items-center justify-center w-[18px] h-[18px] rounded-[2px] border transition-[background-color,border-color,color] duration-150 ${
            isActive
              ? 'border-grid-strong bg-background/85 text-foreground'
              : isAttached
                ? 'border-grid bg-muted/70 text-muted-foreground group-hover:text-foreground'
                : 'border-transparent bg-transparent text-foreground/55 group-hover:text-foreground'
          }`}
          aria-hidden="true"
        >
          <ChevronDown
            className={`w-3 h-3 transition-transform duration-150 ${
              isActive ? 'rotate-180' : ''
            }`}
          />
        </span>
      </button>
    );
  };

  return (
    <header
      ref={navRootRef}
      onMouseLeave={scheduleClose}
      className="fixed top-0 inset-x-0 z-50 w-full"
    >
      {/* Architectural Single-Row Navbar: Inlaid into hero at top, attached to symmetrical grid on scroll */}
      <div
        data-testid="floating-navbar"
        data-theme={effectiveLight ? 'light' : 'dark'}
        data-scrolled={scrolled ? 'true' : 'false'}
        data-attached={isAttached ? 'true' : 'false'}
        data-open-menu={openMenu ?? 'none'}
        className={`relative z-30 w-full border-b transition-[background-color,border-color,box-shadow,transform] duration-200 ease-out ${
          effectiveLight ? 'surface-light-band text-foreground' : 'text-foreground'
        } ${
          isAttached
            ? openMenu || mobileOpen
              ? 'bg-background border-grid shadow-md translate-y-0'
              : effectiveLight
                ? 'bg-background/92 backdrop-blur-md border-grid shadow-sm translate-y-0'
                : 'bg-background/88 backdrop-blur-md border-grid shadow-md translate-y-0'
            : 'bg-transparent border-transparent shadow-none translate-y-1.5'
        }`}
      >
        {/* Symmetrical 3-track architectural frame matching the entire site */}
        <div className="arch-grid-band">
          <div
            aria-hidden="true"
            className={`border-r transition-colors duration-200 ${
              isAttached ? 'border-grid' : 'border-transparent'
            }`}
          />

          <div className="min-w-0 px-4 sm:px-6 h-[68px] flex items-center justify-between gap-4">
            {/* Left Zone: Brand Mark & Wordmark */}
            <a
              href="#hero"
              aria-label="Niki Agent home"
              onClick={() => setOpenMenu(null)}
              className="inline-flex items-center gap-2.5 whitespace-nowrap shrink-0 text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-[2px]"
            >
              <NikiMark className="w-5 h-5 text-chart-1 dark:text-chart-2" />
              <span className="font-display text-[19px] font-semibold tracking-[-0.03em]">
                niki agent
              </span>
            </a>

            {/* Center Zone: Clean Single-Line Nav Triggers */}
            <nav
              aria-label="Primary navigation"
              className="hidden lg:flex items-center gap-1"
            >
              {renderTrigger('providers', 'Providers')}
              {renderTrigger('architecture', 'Architecture')}
              {renderTrigger('resources', 'Resources')}
              <button
                type="button"
                onClick={() => {
                  setOpenMenu(null);
                  onOpenChecklist();
                }}
                onMouseEnter={() => setOpenMenu(null)}
                className={`${BTN.base} ${BTN.ghost}`}
              >
                Docs
              </button>
              <a
                href="#benchmarks"
                onClick={() => setOpenMenu(null)}
                onMouseEnter={() => setOpenMenu(null)}
                className={`${BTN.base} ${BTN.ghost}`}
              >
                Benchmarks
              </a>
            </nav>

            {/* Right Zone: Theme Toggle + Normalized Secondary & Primary CTAs */}
            <div className="hidden lg:flex items-center gap-2.5 shrink-0">
              <ThemeToggleButton colorMode={colorMode} onToggle={onToggleColorMode} compact />
              <button
                type="button"
                onClick={() => {
                  setOpenMenu(null);
                  onOpenGithub();
                }}
                className={`${BTN.base} ${BTN.secondary} ${
                  isAttached
                    ? ''
                    : 'bg-transparent border-foreground/20 hover:bg-foreground/[0.06] hover:border-foreground/35'
                }`}
              >
                GitHub
              </button>
              <button
                type="button"
                onClick={() => {
                  setOpenMenu(null);
                  onOpenInstall();
                }}
                className={`${BTN.base} ${BTN.primary}`}
              >
                <span>Install Niki</span>
                <Keycap inverted>I</Keycap>
              </button>
            </div>

            {/* Mobile Controls */}
            <div className="lg:hidden flex items-center gap-2">
              <ThemeToggleButton colorMode={colorMode} onToggle={onToggleColorMode} compact />
              <button
                type="button"
                aria-label={mobileOpen ? 'Close navigation menu' : 'Open navigation menu'}
                aria-expanded={mobileOpen}
                onClick={() => setMobileOpen((v) => !v)}
                className={`${BTN.base} ${BTN.icon} text-foreground`}
              >
                {mobileOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
              </button>
            </div>
          </div>

          <div
            aria-hidden="true"
            className={`border-l transition-colors duration-200 ${
              isAttached ? 'border-grid' : 'border-transparent'
            }`}
          />
        </div>
      </div>

      {/* Full-Width Architectural Hairline Mega Menu Overlay */}
      {openMenu && (
        <div
          id={`megamenu-${openMenu}`}
          ref={megaPanelRef}
          role="region"
          aria-label={`${openMenu} mega menu`}
          data-testid="navbar-megamenu"
          data-active-panel={openMenu}
          onMouseEnter={cancelCloseTimer}
          onMouseLeave={scheduleClose}
          className={`hidden lg:block absolute left-0 right-0 top-full z-20 border-b border-grid shadow-2xl ${
            effectiveLight
              ? 'surface-light-band bg-background text-foreground'
              : 'bg-background text-foreground'
          }`}
        >
          <div className="arch-grid-band">
            <div aria-hidden="true" className="border-r border-grid" />

            <div className="min-w-0">
              {/* PANEL 1: PROVIDERS */}
              {openMenu === 'providers' && (
                <div className="grid grid-cols-12 divide-x divide-grid">
                  {/* Col 1: Paired Quick-Launch Grid */}
                  <div className="mega-col col-span-6 p-6 flex flex-col justify-between gap-5">
                    <div>
                      <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                        QUICK LAUNCH WITH UV
                      </p>
                      <div className="grid grid-cols-2 border-t border-l border-grid">
                        {MEGA_MENU_PROVIDER_PAIRINGS.map((pair) => (
                          <button
                            key={pair.id}
                            type="button"
                            onClick={() => {
                              setOpenMenu(null);
                              onOpenInstall(pair.command);
                            }}
                            className={`group flex items-center border-r border-b overflow-hidden text-left ${CARD.cellInteractive}`}
                          >
                            <div className="flex items-center divide-x divide-grid border-r border-grid bg-muted/55 shrink-0">
                              <span className="inline-flex items-center justify-center w-10 h-11 text-chart-1 dark:text-chart-2">
                                <NikiMark className="w-4 h-4" />
                              </span>
                              <span className="inline-flex items-center justify-center w-10 h-11 text-foreground/90 group-hover:text-foreground">
                                <ProviderIcon id={pair.id} className="w-4 h-4" />
                              </span>
                            </div>
                            <span className="px-3 py-2 text-[13.5px] font-medium text-foreground/90 group-hover:text-foreground truncate">
                              {pair.label}
                            </span>
                          </button>
                        ))}
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={() => {
                        setOpenMenu(null);
                        onSelectDetail('providers');
                      }}
                      className={`${BTN.base} ${BTN.subtle} w-full`}
                    >
                      All providers
                    </button>
                  </div>

                  {/* Col 2: Top Providers Square Icon Matrix */}
                  <div className="mega-col col-span-3 p-6 flex flex-col justify-between gap-5">
                    <div>
                      <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                        TOP PROVIDERS
                      </p>
                      <div className="grid grid-cols-4 border-t border-l border-grid">
                        {MEGA_MENU_TOP_PROVIDERS.map((prov) => (
                          <button
                            key={prov.id}
                            type="button"
                            title={prov.name}
                            aria-label={`Inspect ${prov.name}`}
                            onClick={() => handleOpenProviderTile(prov.id)}
                            className={`group flex items-center justify-center h-12 border-r border-b text-foreground/85 hover:text-foreground ${CARD.cellInteractive}`}
                          >
                            <ProviderIcon id={prov.id} className="w-5 h-5" />
                          </button>
                        ))}
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={() => {
                        setOpenMenu(null);
                        onSelectDetail('providers');
                      }}
                      className={`${BTN.base} ${BTN.subtle} w-full`}
                    >
                      All connectors
                    </button>
                  </div>

                  {/* Col 3: Capabilities Icon List */}
                  <div className="mega-col col-span-3 p-6">
                    <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                      CAPABILITIES
                    </p>
                    <div className="divide-y divide-grid border-y border-grid">
                      {MEGA_MENU_CAPABILITIES.map((cap) => (
                        <button
                          key={cap.id}
                          type="button"
                          onClick={() => {
                            setOpenMenu(null);
                            onSelectDetail(cap.detailId);
                          }}
                          className="group w-full flex items-start gap-3 text-left py-3 px-2 hover:bg-muted/65 transition-colors cursor-pointer"
                        >
                          <span className="inline-flex items-center justify-center w-8 h-8 rounded-[2px] border border-grid bg-card text-chart-1 dark:text-chart-2 shrink-0 mt-0.5 group-hover:border-grid-strong transition-colors">
                            <CapabilityIcon icon={cap.icon} />
                          </span>
                          <span className="min-w-0">
                            <span className="block text-[13.5px] font-medium text-foreground">
                              {cap.title}
                            </span>
                            <span className="block text-[12px] leading-snug text-muted-foreground mt-0.5">
                              {cap.desc}
                            </span>
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              )}

              {/* PANEL 2: ARCHITECTURE */}
              {openMenu === 'architecture' && (
                <div className="grid grid-cols-12 divide-x divide-grid">
                  {/* Col 1: 4 Feature Levels */}
                  <div className="mega-col col-span-5 p-6 flex flex-col justify-between gap-5">
                    <div>
                      <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                        FOUR FEATURE LEVELS
                      </p>
                      <div className="grid grid-cols-2 border-t border-l border-grid">
                        {MEGA_MENU_LEVELS.map((lvl) => (
                          <a
                            key={lvl.level}
                            href={lvl.href}
                            onClick={() => setOpenMenu(null)}
                            className={`group border-r border-b p-3.5 ${CARD.cellInteractive}`}
                          >
                            <div className="flex items-center justify-between font-tech text-[11px] text-chart-1 dark:text-chart-2">
                              <span>LEVEL {lvl.level}</span>
                              <ArrowUpRight className="w-3.5 h-3.5 opacity-60 group-hover:opacity-100 transition-opacity" />
                            </div>
                            <div className="mt-1.5 text-[14px] font-medium text-foreground">
                              {lvl.title}
                            </div>
                            <p className="mt-1 text-[12px] leading-snug text-muted-foreground">
                              {lvl.desc}
                            </p>
                          </a>
                        ))}
                      </div>
                    </div>

                    <a
                      href="#lifecycle"
                      onClick={() => setOpenMenu(null)}
                      className={`${BTN.base} ${BTN.subtle} w-full`}
                    >
                      Explore 6-stage orbital lifecycle
                    </a>
                  </div>

                  {/* Col 2: Runtime Themes (/theme) */}
                  <div className="mega-col col-span-4 p-6 flex flex-col justify-between gap-5">
                    <div>
                      <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                        RUNTIME THEMES · /THEME
                      </p>
                      <div className="border border-grid divide-y divide-grid">
                        {NIKI_THEMES.map((theme) => (
                          <button
                            key={theme.id}
                            type="button"
                            onClick={() => {
                              setOpenMenu(null);
                              onSelectDetail('themes-system');
                            }}
                            className={`group w-full flex items-center justify-between gap-3 px-3.5 py-2.5 text-left ${CARD.cellInteractive}`}
                          >
                            <div className="flex items-center gap-3 min-w-0">
                              <span
                                className="w-7 h-7 rounded-[2px] border border-grid-strong shrink-0 flex items-center justify-center"
                                style={{ backgroundColor: theme.bg, color: theme.accent }}
                                aria-hidden="true"
                              >
                                <span
                                  className="w-2 h-2 rotate-45"
                                  style={{ backgroundColor: theme.accent }}
                                />
                              </span>
                              <div className="min-w-0">
                                <div className="font-tech text-[12.5px] font-medium text-foreground">
                                  {theme.name}
                                </div>
                                <div className="text-[11.5px] text-muted-foreground truncate">
                                  {theme.description}
                                </div>
                              </div>
                            </div>
                            <span className="font-tech text-[11px] text-chart-1 dark:text-chart-2 tabular-nums shrink-0">
                              {theme.tag}
                            </span>
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>

                  {/* Col 3: Core Modules */}
                  <div className="mega-col col-span-3 p-6">
                    <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                      CORE MODULES
                    </p>
                    <div className="divide-y divide-grid border-y border-grid">
                      {[
                        {
                          icon: <KeyRound className="w-4 h-4" aria-hidden="true" />,
                          title: 'keymap.py Registry',
                          desc: 'No dead shortcuts, checked in CI.',
                          detailId: 'keymap',
                        },
                        {
                          icon: <Layers className="w-4 h-4" aria-hidden="true" />,
                          title: 'Bounded Mount Window',
                          desc: 'Offscreen turns stay unmounted.',
                          detailId: 'mount-window',
                        },
                        {
                          icon: <ListChecks className="w-4 h-4" aria-hidden="true" />,
                          title: 'Hermetic Test Suite',
                          desc: 'Runs offline without API keys or TTY.',
                          detailId: 'ci-tests',
                        },
                        {
                          icon: <GitBranch className="w-4 h-4" aria-hidden="true" />,
                          title: 'One Agent Loop',
                          desc: 'Tools, approvals & subagents.',
                          detailId: 'agent-loop',
                        },
                      ].map((mod) => (
                        <button
                          key={mod.detailId}
                          type="button"
                          onClick={() => {
                            setOpenMenu(null);
                            onSelectDetail(mod.detailId);
                          }}
                          className="group w-full flex items-start gap-3 text-left py-3 px-2 hover:bg-muted/65 transition-colors cursor-pointer"
                        >
                          <span className="inline-flex items-center justify-center w-8 h-8 rounded-[2px] border border-grid bg-card text-chart-1 dark:text-chart-2 shrink-0 mt-0.5 group-hover:border-grid-strong">
                            {mod.icon}
                          </span>
                          <span className="min-w-0">
                            <span className="block text-[13.5px] font-medium text-foreground">
                              {mod.title}
                            </span>
                            <span className="block text-[12px] leading-snug text-muted-foreground mt-0.5">
                              {mod.desc}
                            </span>
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              )}

              {/* PANEL 3: RESOURCES */}
              {openMenu === 'resources' && (
                <div className="grid grid-cols-12 divide-x divide-grid">
                  {/* Col 1: Interactive Guides */}
                  <div className="mega-col col-span-5 p-6 flex flex-col justify-between gap-5">
                    <div>
                      <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                        INTERACTIVE GUIDES
                      </p>
                      <div className="grid grid-cols-2 border-t border-l border-grid">
                        {MEGA_MENU_GUIDES.map((guide) => (
                          <button
                            key={guide.id}
                            type="button"
                            onClick={() => {
                              setOpenMenu(null);
                              onSelectDetail(guide.detailId);
                            }}
                            className={`group flex flex-col border-r border-b overflow-hidden text-left ${CARD.cellInteractive}`}
                          >
                            <div className="relative h-[120px] w-full bg-[#06070a] border-b border-grid p-3.5 flex flex-col justify-between overflow-hidden">
                              <div className="flex items-center justify-between">
                                <span className="font-tech text-[9.5px] tracking-[0.1em] text-[#5db8ff]">
                                  {guide.badge}
                                </span>
                                <span className="inline-flex items-center justify-center w-5 h-5 rounded-[2px] bg-[#141a2e] text-[#5db8ff]">
                                  <NikiMark className="w-3 h-3" />
                                </span>
                              </div>
                              <div className="font-tech text-[10.5px] leading-relaxed space-y-1">
                                <div className="text-[#f4f6fc] truncate">{guide.terminalCmd}</div>
                                <div className="text-[#9aa4bc] truncate">{guide.terminalOut}</div>
                              </div>
                            </div>

                            <div className="p-3.5 flex items-start justify-between gap-2.5 flex-1">
                              <span className="text-[13px] font-medium leading-snug text-foreground">
                                {guide.title}
                              </span>
                              <Play
                                className="w-3.5 h-3.5 text-chart-1 dark:text-chart-2 shrink-0 mt-0.5 group-hover:translate-x-0.5 transition-transform"
                                aria-hidden="true"
                              />
                            </div>
                          </button>
                        ))}
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={() => {
                        setOpenMenu(null);
                        onOpenChecklist();
                      }}
                      className={`${BTN.base} ${BTN.subtle} w-full`}
                    >
                      Read full checklist
                    </button>
                  </div>

                  {/* Col 2: Verified Baselines */}
                  <div className="mega-col col-span-4 p-6 flex flex-col justify-between gap-5">
                    <div>
                      <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                        VERIFIED BASELINES
                      </p>
                      <div className="border border-grid divide-y divide-grid">
                        {MEGA_MENU_BASELINES.map((base) => (
                          <button
                            key={base.id}
                            type="button"
                            onClick={() => {
                              setOpenMenu(null);
                              onSelectDetail(base.detailId);
                            }}
                            className={`group relative w-full flex items-center gap-4 p-4 text-left overflow-hidden ${CARD.cellInteractive}`}
                          >
                            <div
                              aria-hidden="true"
                              className="pointer-events-none absolute inset-y-0 left-0 w-24 opacity-25 dark:opacity-30"
                              style={{
                                backgroundImage:
                                  'radial-gradient(currentColor 1px, transparent 1px)',
                                backgroundSize: '8px 8px',
                              }}
                            />
                            <span className="relative z-10 inline-flex items-center justify-center w-12 h-12 rounded-[2px] border border-grid-strong bg-background font-display text-[17px] font-semibold text-chart-1 dark:text-chart-2 tabular-nums shrink-0">
                              {base.metric}
                            </span>
                            <span className="relative z-10 min-w-0">
                              <span className="block text-[14.5px] font-semibold text-foreground">
                                {base.title}
                              </span>
                              <span className="block text-[12.5px] leading-snug text-muted-foreground mt-1">
                                {base.desc}
                              </span>
                            </span>
                          </button>
                        ))}
                      </div>
                    </div>

                    <a
                      href="#benchmarks"
                      onClick={() => setOpenMenu(null)}
                      className={`${BTN.base} ${BTN.subtle} w-full`}
                    >
                      All benchmarks
                    </a>
                  </div>

                  {/* Col 3: Docs & Specs */}
                  <div className="mega-col col-span-3 p-6">
                    <p className="font-tech text-[11px] tracking-[0.12em] uppercase text-muted-foreground mb-4">
                      DOCS &amp; SPECS
                    </p>
                    <ul className="divide-y divide-grid border-y border-grid">
                      {FOOTER_COLUMNS.docs.map((doc, idx) => (
                        <li key={doc.label}>
                          <a
                            href={doc.href}
                            target="_blank"
                            rel="noopener noreferrer"
                            onClick={() => setOpenMenu(null)}
                            className="group flex items-center gap-3 px-2 py-2.5 text-[13.5px] font-medium text-foreground/85 hover:text-foreground hover:bg-muted/75 transition-colors"
                          >
                            <span className="text-chart-1 dark:text-chart-2 shrink-0" aria-hidden="true">
                              {idx === 0 ? (
                                <BookOpen className="w-4 h-4" />
                              ) : idx % 2 === 1 ? (
                                <ListChecks className="w-4 h-4" />
                              ) : (
                                <FileCode2 className="w-4 h-4" />
                              )}
                            </span>
                            <span className="font-tech text-[12.5px] truncate">{doc.label}</span>
                          </a>
                        </li>
                      ))}
                    </ul>
                  </div>
                </div>
              )}
            </div>

            <div aria-hidden="true" className="border-l border-grid" />
          </div>
        </div>
      )}

      {/* Subtle Dimming Scrim Below Mega Menu when Open */}
      {openMenu && (
        <button
          type="button"
          aria-label="Close mega menu"
          onClick={() => setOpenMenu(null)}
          className="hidden lg:block fixed inset-x-0 top-[68px] bottom-0 z-10 bg-black/50 cursor-default"
        />
      )}

      {/* Mobile Navigation Drawer */}
      {mobileOpen && (
        <div
          className={`lg:hidden border-b border-grid px-5 py-5 space-y-5 shadow-xl ${
            effectiveLight
              ? 'surface-light-band bg-background text-foreground'
              : 'bg-background text-foreground'
          }`}
        >
          <div className="grid grid-cols-2 border-t border-l border-grid font-tech text-[12.5px]">
            {MEGA_MENU_LEVELS.map((lvl) => (
              <a
                key={lvl.level}
                href={lvl.href}
                onClick={() => setMobileOpen(false)}
                className="border-r border-b border-grid bg-card/70 p-3 hover:bg-muted"
              >
                <div className="text-[10.5px] text-chart-1 dark:text-chart-2">
                  LEVEL {lvl.level}
                </div>
                <div className="mt-1 text-foreground font-medium">{lvl.title}</div>
              </a>
            ))}
          </div>
          <div className="flex flex-col sm:flex-row gap-2.5">
            <button
              type="button"
              onClick={() => {
                setMobileOpen(false);
                onOpenInstall();
              }}
              className={`${BTN.base} ${BTN.primary} flex-1`}
            >
              <span>Install Niki</span>
              <Keycap inverted>I</Keycap>
            </button>
            <button
              type="button"
              onClick={() => {
                setMobileOpen(false);
                onOpenGithub();
              }}
              className={`${BTN.base} ${BTN.secondary} flex-1`}
            >
              GitHub
            </button>
          </div>
          <div className="pt-3 border-t border-grid flex items-center justify-between text-[12px] text-muted-foreground font-tech">
            <button
              type="button"
              onClick={() => {
                setMobileOpen(false);
                onOpenChecklist();
              }}
              className="inline-flex items-center gap-1.5 text-foreground hover:underline cursor-pointer"
            >
              <FileText className="w-3.5 h-3.5" aria-hidden="true" />
              <span>Read the docs</span>
            </button>
            <span>MIT Licensed</span>
          </div>
        </div>
      )}
    </header>
  );
}
