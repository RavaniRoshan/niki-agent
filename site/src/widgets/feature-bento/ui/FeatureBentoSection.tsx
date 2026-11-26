import React, { useEffect, useRef, useState } from 'react';
import { ArrowUpRight, Check, Copy } from 'lucide-react';
import { NIKI_THEMES, NikiThemePalette } from '../../../entities/niki';
import {
  BTN,
  CARD,
  ensureGsapRegistered,
  GridBand,
  gsap,
  MOTION_TIMESTAMPS,
  prefersReducedMotion,
  ProviderIcon,
  SectionEyebrow,
  TerminalTitleBar,
} from '../../../shared';

interface FeatureBentoSectionProps {
  onSelectDetail: (detailId: string) => void;
}

const PHASE_GLYPHS = ['◐', '◓', '◑', '◒'];

const TOML_GATEWAYS = [
  {
    id: 'kimi',
    label: 'Kimi',
    section: '[models.providers.kimi]',
    cls: 'langchain_openai:ChatOpenAI',
    env: 'MOONSHOT_API_KEY',
    model: 'kimi-k2.7-code',
  },
  {
    id: 'kilo',
    label: 'Kilo',
    section: '[models.providers.kilo]',
    cls: 'langchain_openrouter:ChatOpenRouter',
    env: 'KILO_API_KEY',
    model: 'claude-opus-4.7',
  },
  {
    id: 'opencode',
    label: 'OpenCode',
    section: '[models.providers.opencode_zen]',
    cls: 'langchain_openai:ChatOpenAI',
    env: 'OPENCODE_API_KEY',
    model: 'deepseek-v4-pro',
  },
];

const KEYMAP_BINDINGS = [
  {
    key: '1',
    short: 'Approve',
    handler: 'approval_position(0)',
    output: 'Applied write_file · keymap.py (+12 −4)',
  },
  {
    key: 'ctrl+o',
    short: 'Tools',
    handler: 'toggle_tool_output',
    output: 'Inline diff output expanded (14 lines)',
  },
  {
    key: 'ctrl+t',
    short: 'Subagents',
    handler: 'toggle_subagent_panel',
    output: 'Subagent runner docked · timers still',
  },
  {
    key: 'a',
    short: 'Auto',
    handler: 'approval_auto',
    output: 'Tool call accepted in auto mode',
  },
];

const NO_COLOR_THEME: NikiThemePalette = {
  id: 'no-color',
  name: 'NO_COLOR',
  description: 'Monochrome ANSI fallback.',
  tag: 'MONO',
  bg: '#0c0c0d',
  surface: '#161618',
  border: '#47474d',
  text: '#f2f2f2',
  muted: '#a6a6ad',
  accent: '#e2e2e6',
};

const COMPOSER_PALETTES: NikiThemePalette[] = [...NIKI_THEMES, NO_COLOR_THEME];

/**
 * Level 01 Feature Widget: Asymmetric 12-Column Hairline-Ruled Bento Grid
 * Tiled edge-to-edge on shared 1px architectural seams inside the global GridBand.
 */
export function FeatureBentoSection({ onSelectDetail }: FeatureBentoSectionProps) {
  const sectionRef = useRef<HTMLElement | null>(null);
  const [composerState, setComposerState] = useState<'streaming' | 'idle'>('streaming');
  const [composerPaletteId, setComposerPaletteId] = useState<string>(NIKI_THEMES[0].id);
  const [glyphIdx, setGlyphIdx] = useState(0);
  const [isVisible, setIsVisible] = useState(true);
  const [activeGateway, setActiveGateway] = useState(0);
  const [tomlCopied, setTomlCopied] = useState(false);
  const tomlCopyTimerRef = useRef<number | null>(null);
  const [activeKeyIdx, setActiveKeyIdx] = useState(0);
  const [lastKeySource, setLastKeySource] = useState<'click' | 'keyboard'>('click');
  const keyPreviewRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    return () => {
      if (tomlCopyTimerRef.current !== null) {
        window.clearTimeout(tomlCopyTimerRef.current);
      }
    };
  }, []);

  // Pause offscreen interval loops via IntersectionObserver
  useEffect(() => {
    const el = sectionRef.current;
    if (!el || typeof IntersectionObserver === 'undefined') return;
    const io = new IntersectionObserver(
      (entries) => {
        setIsVisible(entries[0]?.isIntersecting ?? false);
      },
      { rootMargin: '120px' }
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  // Composer phase sweep runs ONLY while visible and streaming; freezes completely at rest
  useEffect(() => {
    if (prefersReducedMotion() || composerState === 'idle' || !isVisible) return;

    let dir = 1;
    let idx = 0;
    const timer = window.setInterval(() => {
      idx += dir;
      if (idx >= PHASE_GLYPHS.length - 1 || idx <= 0) dir *= -1;
      setGlyphIdx(idx);
    }, 300);

    return () => window.clearInterval(timer);
  }, [composerState, isVisible]);

  // Timestamped GSAP ScrollTrigger entrance for the whole Bento section
  useEffect(() => {
    ensureGsapRegistered();
    const el = sectionRef.current;
    if (!el || prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      const tl = gsap.timeline({
        defaults: { ease: 'power3.out' },
        scrollTrigger: {
          trigger: el,
          start: 'top 86%',
          once: true,
        },
      });

      tl.from(
        '.bento-seq-header',
        { opacity: 0, y: 18, duration: 0.48, stagger: 0.06, clearProps: 'transform,opacity' },
        MOTION_TIMESTAMPS.sectionHeader
      )
        .from(
          '.bento-seq-card',
          { opacity: 0, y: 22, duration: 0.52, stagger: 0.08, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.sectionGrid
        )
        .from(
          '.bento-seq-satellite',
          { opacity: 0, y: 10, scale: 0.96, duration: 0.38, stagger: 0.06, clearProps: 'transform,opacity' },
          MOTION_TIMESTAMPS.sectionSatellite
        );
    }, el);

    return () => ctx.revert();
  }, []);

  // Continuous composer sweep & TOML route pulse
  useEffect(() => {
    const el = sectionRef.current;
    if (!el || prefersReducedMotion()) return;

    const ctx = gsap.context(() => {
      if (composerState === 'streaming') {
        gsap.to('.bento-sweep-pill', {
          x: 74,
          duration: 1.35,
          repeat: -1,
          yoyo: true,
          ease: 'power1.inOut',
        });
      } else {
        gsap.set('.bento-sweep-pill', { x: 0 });
      }

      gsap.to('.bento-route-dash', {
        strokeDashoffset: -28,
        duration: 1.8,
        repeat: -1,
        ease: 'none',
      });
    }, el);

    return () => ctx.revert();
  }, [composerState]);

  const handleSelectKeycap = (idx: number, source: 'click' | 'keyboard' = 'click') => {
    setActiveKeyIdx(idx);
    setLastKeySource(source);
    if (!prefersReducedMotion() && keyPreviewRef.current) {
      gsap.fromTo(
        keyPreviewRef.current,
        { opacity: 0.65, y: 4 },
        { opacity: 1, y: 0, duration: 0.2, ease: 'power2.out' }
      );
    }
  };

  const handleKeymapKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const key = e.key.toLowerCase();
    if (!e.ctrlKey && !e.metaKey && !e.altKey && key === '1') {
      e.preventDefault();
      handleSelectKeycap(0, 'keyboard');
    } else if (e.ctrlKey && key === 'o') {
      e.preventDefault();
      handleSelectKeycap(1, 'keyboard');
    } else if (e.ctrlKey && key === 't') {
      e.preventDefault();
      handleSelectKeycap(2, 'keyboard');
    } else if (!e.ctrlKey && !e.metaKey && !e.altKey && key === 'a') {
      e.preventDefault();
      handleSelectKeycap(3, 'keyboard');
    }
  };

  const currentGateway = TOML_GATEWAYS[activeGateway];
  const currentKey = KEYMAP_BINDINGS[activeKeyIdx];
  const activePalette =
    COMPOSER_PALETTES.find((p) => p.id === composerPaletteId) || COMPOSER_PALETTES[0];

  const handleCopyToml = () => {
    const snippet = [
      currentGateway.section,
      `class_path = "${currentGateway.cls}"`,
      `api_key_env = "${currentGateway.env}"`,
      `models = ["${currentGateway.model}"]`,
    ].join('\n');
    navigator.clipboard?.writeText(snippet).catch(() => {});
    setTomlCopied(true);
    if (tomlCopyTimerRef.current !== null) {
      window.clearTimeout(tomlCopyTimerRef.current);
    }
    tomlCopyTimerRef.current = window.setTimeout(() => {
      setTomlCopied(false);
      tomlCopyTimerRef.current = null;
    }, 1800);
  };

  return (
    <GridBand
      id="bento-features"
      sectionRef={sectionRef}
      ariaLabel="Level 1 core surface bento"
    >
      {/* Unified Hairline Eyebrow Bar */}
      <SectionEyebrow
        index="01"
        label="Core Surface Architecture"
        rightMeta="4 MODULES · QUIET AT REST"
      />

      {/* Editorial Section Header Cell */}
      <div className="px-5 sm:px-8 lg:px-10 py-10 lg:py-14 border-b border-grid grid grid-cols-1 lg:grid-cols-12 gap-6 lg:gap-10 items-end">
        <div className="bento-seq-header lg:col-span-7">
          <h2 className="font-display text-[36px] sm:text-[44px] lg:text-[52px] font-semibold leading-[1.08] tracking-[-0.03em] text-foreground [text-wrap:balance]">
            Calm craft across every terminal pixel
          </h2>
        </div>
        <p className="bento-seq-header lg:col-span-5 font-normal text-[16px] sm:text-[17px] leading-[1.55] text-muted-foreground">
          Single-rule composer, one-line install, any model, and a live keymap registry verified in pytest.
        </p>
      </div>

      {/* Asymmetric 12-Column Hairline-Ruled Matrix (7+5, 5+7) */}
      <div className="grid grid-cols-1 lg:grid-cols-12">
        {/* TILE 1 (7 Cols): Zero-Flicker Composer + Live 4-Theme & NO_COLOR Preview */}
        <article
          className={`bento-seq-card lg:col-span-7 border-b lg:border-r flex flex-col justify-between ${CARD.cell}`}
        >
          <div className="p-6 sm:p-8 flex flex-col sm:flex-row sm:items-start justify-between gap-4">
            <div className="max-w-[440px]">
              <h3 className="font-feature-heading text-foreground">
                Single-rule composer with adaptive phase motion
              </h3>
              <p className="mt-2 font-normal text-[16px] leading-[1.55] text-muted-foreground">
                Motion runs only while streaming. Switch palettes or NO_COLOR live below; at rest, timers stop completely.
              </p>
            </div>
            <div
              role="group"
              aria-label="Composer motion mode"
              className="inline-flex items-center p-0.5 rounded-[2px] border border-grid-strong bg-background font-tech text-[11px] shrink-0 self-start"
            >
              <button
                type="button"
                aria-pressed={composerState === 'streaming'}
                onClick={() => setComposerState('streaming')}
                className={`px-2.5 py-1.5 rounded-[1px] transition-colors cursor-pointer whitespace-nowrap ${
                  composerState === 'streaming'
                    ? 'bg-primary text-primary-foreground font-medium'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                Active Stream
              </button>
              <button
                type="button"
                aria-pressed={composerState === 'idle'}
                onClick={() => setComposerState('idle')}
                className={`px-2.5 py-1.5 rounded-[1px] transition-colors cursor-pointer whitespace-nowrap ${
                  composerState === 'idle'
                    ? 'bg-primary text-primary-foreground font-medium'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                Idle Stillness
              </button>
            </div>
          </div>

          <div
            data-testid="bento-composer-stage"
            data-composer-state={composerState}
            data-composer-theme={activePalette.id}
            className={`${CARD.stageFrame} h-[268px] sm:h-[282px] border-t`}
          >
            <div
              style={{
                backgroundColor: activePalette.bg,
                borderColor: activePalette.border,
                color: activePalette.text,
              }}
              className="absolute left-5 sm:left-8 top-5 -right-8 -bottom-6 rounded-tl-[2px] border-t border-l shadow-xl flex flex-col transition-colors duration-200"
            >
              <TerminalTitleBar
                title={`~/project · /theme ${activePalette.name}`}
                rightLabel={
                  composerState === 'streaming'
                    ? `${activePalette.name} · streaming`
                    : `${activePalette.name} · at rest`
                }
                bgColor={activePalette.surface}
                borderColor={activePalette.border}
                textColor={activePalette.muted}
              />
              <div className="p-5 sm:p-6 flex-1 flex flex-col justify-between font-tech">
                {/* Real transcript diff lines replacing static filler bars */}
                <div className="space-y-1.5 text-[11px] leading-[1.45]">
                  <div className="flex items-center gap-2 truncate">
                    <span style={{ color: activePalette.accent }}>✓</span>
                    <span style={{ color: activePalette.muted }}>
                      pytest tests/unit_tests/niki · passed
                    </span>
                  </div>
                  <div className="flex items-center gap-2 truncate">
                    <span style={{ color: activePalette.accent }}>~</span>
                    <span style={{ color: activePalette.text }}>
                      niki_agent/tui/widgets/chat_input.py
                    </span>
                    <span style={{ color: activePalette.muted }}>(+14 −3)</span>
                  </div>
                </div>

                <div className="pb-12 space-y-2.5">
                  <div
                    className="flex items-center gap-3 text-[11.5px]"
                    style={{ color: activePalette.accent }}
                  >
                    <span className="text-[14px] leading-none" aria-hidden="true">
                      {composerState === 'streaming' ? PHASE_GLYPHS[glyphIdx] : '◐'}
                    </span>
                    <div
                      className="relative w-24 h-1.5 rounded-[1px] overflow-hidden"
                      style={{ backgroundColor: activePalette.border }}
                    >
                      <span
                        className={`bento-sweep-pill absolute left-0 top-0 h-1.5 transition-colors ${
                          composerState === 'streaming' ? 'w-5' : 'w-full opacity-45'
                        }`}
                        style={{ backgroundColor: activePalette.accent }}
                      />
                    </div>
                    <span className="text-[10.5px]" style={{ color: activePalette.muted }}>
                      {composerState === 'streaming'
                        ? 'Thinking · adaptive cadence'
                        : 'Idle · timers halted'}
                    </span>
                  </div>
                  <div
                    className="h-px w-full"
                    style={{
                      background: `linear-gradient(90deg, ${activePalette.accent}, ${activePalette.border}, transparent)`,
                    }}
                  />
                  <div
                    className="text-[12px] flex items-center gap-2"
                    style={{ color: activePalette.text }}
                  >
                    <span style={{ color: activePalette.accent }}>›</span>
                    <span>Extract bounded mount window tests</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Live 4-Theme + NO_COLOR Switcher Bar */}
            <div
              role="group"
              aria-label="Live terminal theme preview"
              className="bento-seq-satellite absolute left-4 right-4 bottom-3.5 z-20 flex flex-wrap items-center justify-between gap-2 pointer-events-none"
            >
              <div className="pointer-events-auto inline-flex flex-wrap items-center rounded-[2px] border border-[#3d5170] bg-[#0e131f]/95 p-0.5 shadow-xl font-tech text-[10px]">
                {COMPOSER_PALETTES.map((palette) => {
                  const selected = palette.id === activePalette.id;
                  return (
                    <button
                      key={palette.id}
                      type="button"
                      aria-pressed={selected}
                      onClick={() => setComposerPaletteId(palette.id)}
                      className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-[1px] transition-colors cursor-pointer whitespace-nowrap ${
                        selected
                          ? 'bg-[#1c263a] text-[#f8fafc] font-semibold ring-1 ring-inset ring-[#5db8ff]/70'
                          : 'text-[#b7c5d8] hover:text-[#f8fafc] hover:bg-[#161e2e]'
                      }`}
                    >
                      <span
                        className="w-2 h-2 rounded-full shrink-0 border border-white/20"
                        style={{ backgroundColor: palette.accent }}
                        aria-hidden="true"
                      />
                      <span>{palette.name}</span>
                    </button>
                  );
                })}
              </div>

              <div className="pointer-events-auto rounded-[2px] border border-[#3d5170] bg-[#141c2b]/95 px-3 py-1.5 shadow-xl font-tech text-[10.5px] flex items-center gap-2.5">
                <span className="text-[#9eb0c7]">Idle frames</span>
                <span className="text-[#9ec8f8] font-medium tabular-nums">
                  {composerState === 'idle' ? 'none' : 'active'}
                </span>
              </div>
            </div>
          </div>
        </article>

        {/* TILE 2 (5 Cols): Zero-Key Instant Boot */}
        <article
          className={`bento-seq-card lg:col-span-5 border-b flex flex-col justify-between ${CARD.cell}`}
        >
          <div className="p-6 sm:p-8 flex items-start justify-between gap-4">
            <div>
              <h3 className="font-feature-heading text-foreground">
                Zero-key instant boot
              </h3>
              <p className="mt-2 font-normal text-[16px] leading-[1.55] text-muted-foreground">
                One installer line, then a working composer. Add a provider key only when you send your first prompt.
              </p>
            </div>
            <button
              type="button"
              onClick={() => onSelectDetail('quickstart-uv')}
              aria-label="Inspect the install"
              className={`${BTN.base} ${BTN.icon}`}
            >
              <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
            </button>
          </div>

          <div className={`${CARD.stageFrame} h-[268px] sm:h-[282px] border-t font-tech`}>
            <div className="absolute left-5 top-5 -right-8 -bottom-6 rounded-tl-[2px] border-t border-l border-[#282e46] bg-[#0d1018] shadow-xl flex flex-col">
              <TerminalTitleBar title="~/.niki" rightLabel="py 3.12" />
              <div className="p-5 space-y-2.5 text-[11.5px]">
                <div className="text-[#dce7f5]">
                  <span className="text-[#9ec8f8] mr-2">$</span>niki
                </div>
                <div className="text-[#eef4fa] flex items-center gap-2 pt-0.5">
                  <span className="text-[#9ec8f8]">◐</span>
                  <span className="font-medium">Niki Agent</span>
                </div>
                <div className="space-y-1.5 pt-1.5 text-[11px] text-[#9eb0c7]">
                  <div className="truncate">
                    <span className="text-[#9ec8f8] mr-1.5">✓</span>keymap registry · every binding bound
                  </div>
                  <div className="truncate">
                    <span className="text-[#9ec8f8] mr-1.5">✓</span>theme: niki · mount: 30-row window
                  </div>
                  <div className="truncate text-[#dce7f5]">
                    <span className="text-[#9ec8f8] mr-1.5">›</span>Ready · add key on first prompt
                  </div>
                </div>
              </div>
            </div>

            <div className="bento-seq-satellite absolute left-4 bottom-3.5 z-20 rounded-[2px] border border-[#3d5170] bg-[#141c2b]/95 px-3.5 py-2 shadow-xl text-[10.5px] flex items-center gap-2.5">
              <span className="text-[#eef4fa]">one package</span>
              <span className="text-[#8295ae]">·</span>
              <span className="text-[#9ec8f8]">no API key</span>
            </div>
          </div>
        </article>

        {/* TILE 3 (5 Cols): Twelve Model Providers via TOML + One-Click Copy */}
        <article
          className={`bento-seq-card lg:col-span-5 border-b lg:border-b-0 lg:border-r flex flex-col justify-between ${CARD.cell}`}
        >
          <div className="p-6 sm:p-8 flex items-start justify-between gap-4">
            <div>
              <h3 className="font-feature-heading text-foreground">
                Any model, zero extra lock-in
              </h3>
              <p className="mt-2 font-normal text-[16px] leading-[1.55] text-muted-foreground">
                Three providers built in, eighteen extras, and any OpenAI-compatible gateway in one TOML block.
              </p>
            </div>
            <button
              type="button"
              onClick={() => onSelectDetail('providers')}
              aria-label="Inspect providers"
              className={`${BTN.base} ${BTN.icon}`}
            >
              <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
            </button>
          </div>

          <div className={`${CARD.stageFrame} h-[260px] sm:h-[272px] border-t font-tech`}>
            <div className="absolute -left-4 top-5 right-10 -bottom-6 rounded-tr-[2px] border-t border-r border-[#282e46] bg-[#0d1018] shadow-xl flex flex-col">
              <TerminalTitleBar title="~/.niki/config.toml" rightLabel="zero deps" />
              <div className="p-5 pl-8 space-y-1.5 text-[11px] text-[#a5b9d1]">
                <div className="flex items-center justify-between gap-2 pr-2">
                  <span className="text-[#9ec8f8]">{currentGateway.section}</span>
                  <button
                    type="button"
                    onClick={handleCopyToml}
                    aria-label={tomlCopied ? 'TOML configuration copied' : `Copy ${currentGateway.label} TOML`}
                    className="inline-flex items-center gap-1 rounded-[2px] border border-[#3d5170] bg-[#141c2b] px-2 py-0.5 text-[10px] text-[#dce7f5] hover:border-[#9ec8f8] hover:text-white transition-colors cursor-pointer"
                  >
                    {tomlCopied ? (
                      <>
                        <Check className="w-3 h-3 text-[#9ec8f8]" aria-hidden="true" />
                        <span>Copied</span>
                      </>
                    ) : (
                      <>
                        <Copy className="w-3 h-3 text-[#9eb0c7]" aria-hidden="true" />
                        <span>Copy TOML</span>
                      </>
                    )}
                  </button>
                </div>
                <div className="truncate">
                  <span className="text-[#8295ae]">class_path = </span>
                  <span className="text-[#eef4fa]">&quot;{currentGateway.cls}&quot;</span>
                </div>
                <div className="truncate">
                  <span className="text-[#8295ae]">api_key_env = </span>
                  <span className="text-[#eef4fa]">&quot;{currentGateway.env}&quot;</span>
                </div>
                <div className="truncate">
                  <span className="text-[#8295ae]">models = </span>
                  <span className="text-[#9ec8f8]">[&quot;{currentGateway.model}&quot;]</span>
                </div>
              </div>
            </div>

            <svg
              viewBox="0 0 110 70"
              className="pointer-events-none absolute right-14 bottom-12 w-24 h-16 z-10"
              aria-hidden="true"
            >
              <path
                d="M 4 50 C 44 50, 56 22, 104 22"
                fill="none"
                stroke="#9ec8f8"
                strokeWidth="1.5"
                strokeDasharray="4 4"
                className="bento-route-dash"
              />
            </svg>

            <div
              role="group"
              aria-label="TOML gateway preview selector"
              className="bento-seq-satellite absolute right-4 bottom-3.5 z-20 rounded-[2px] border border-[#3d5170] bg-[#0e131f]/95 p-1 shadow-2xl flex items-center gap-0.5"
            >
              {TOML_GATEWAYS.map((g, idx) => (
                <button
                  key={g.id}
                  type="button"
                  aria-pressed={idx === activeGateway}
                  onClick={() => {
                    setActiveGateway(idx);
                    setTomlCopied(false);
                  }}
                  className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-[1px] text-[10.5px] transition-colors cursor-pointer ${
                    idx === activeGateway
                      ? 'bg-[#1c263a] text-[#f8fafc] font-semibold ring-1 ring-inset ring-[#5db8ff]/70'
                      : 'text-[#b7c5d8] hover:text-[#f8fafc] hover:bg-[#161e2e]'
                  }`}
                >
                  <ProviderIcon id={g.id} className="w-3 h-3" />
                  <span>{g.label}</span>
                </button>
              ))}
            </div>
          </div>
        </article>

        {/* TILE 4 (7 Cols): Live Interactive Keymap Registry (Real Keyboard Shortcuts) */}
        <article
          className={`bento-seq-card lg:col-span-7 flex flex-col justify-between ${CARD.cell}`}
        >
          <div className="p-6 sm:p-8 flex items-start justify-between gap-4">
            <div className="max-w-[460px]">
              <h3 className="font-feature-heading text-foreground">
                Keyboard-native bindings with zero dead shortcuts
              </h3>
              <p className="mt-2 font-normal text-[16px] leading-[1.55] text-muted-foreground">
                Footer hints, --help, and KEYMAP.md render from one live registry checked in CI. Focus the preview and press 1, a, ctrl+o, or ctrl+t.
              </p>
            </div>
            <button
              type="button"
              onClick={() => onSelectDetail('keymap')}
              aria-label="Inspect Live Keymap Registry"
              className={`${BTN.base} ${BTN.icon}`}
            >
              <ArrowUpRight className="w-4 h-4" aria-hidden="true" />
            </button>
          </div>

          <div
            tabIndex={0}
            onKeyDown={handleKeymapKeyDown}
            aria-label="Interactive keymap terminal preview. Press 1, a, Control+O, or Control+T to test bindings."
            data-testid="bento-keymap-stage"
            data-active-key={currentKey.key}
            className={`${CARD.stageFrame} h-[260px] sm:h-[272px] border-t font-tech focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring`}
          >
            <div className="absolute left-6 sm:left-8 top-5 -right-8 -bottom-6 rounded-tl-[2px] border-t border-l border-[#282e46] bg-[#0d1018] shadow-xl flex flex-col">
              <TerminalTitleBar
                title="niki_agent/niki/keymap.py"
                rightLabel={lastKeySource === 'keyboard' ? `key: ${currentKey.key}` : 'all bound'}
              />
              <div className="p-5 sm:p-6 space-y-3.5">
                <div
                  ref={keyPreviewRef}
                  className="rounded-[2px] border border-[#3d5170] bg-[#141d2d] px-4 py-3 max-w-[460px] space-y-1.5"
                >
                  <div className="flex items-center justify-between text-[12.5px] font-normal gap-3">
                    <span className="text-[#9ec8f8] truncate">{currentKey.handler}</span>
                    <span className="text-[#9eb0c7] text-[11px] shrink-0">pytest verified</span>
                  </div>
                  <div className="text-[12.5px] font-normal text-[#eef4fa]">{currentKey.output}</div>
                </div>
                <div className="space-y-1 text-[11px] text-[#9eb0c7]">
                  <div className="truncate">
                    <span className="text-[#9ec8f8] mr-1.5">$</span>pytest tests/unit_tests/niki/test_keymap.py
                  </div>
                  <div className="truncate text-[#c5d6ea]">
                    <span className="text-[#9ec8f8] mr-1.5">✓</span>every binding matches --help and KEYMAP.md · press 1, a, ctrl+o, ctrl+t
                  </div>
                </div>
              </div>
            </div>

            <div
              role="group"
              aria-label="Interactive keymap bindings"
              className="bento-seq-satellite absolute right-4 bottom-3.5 z-20 rounded-[2px] border border-[#3d5170] bg-[#141c2b]/95 p-1.5 shadow-2xl flex flex-wrap items-center gap-1"
            >
              {KEYMAP_BINDINGS.map((b, idx) => {
                const active = idx === activeKeyIdx;
                return (
                  <button
                    key={b.key}
                    type="button"
                    aria-pressed={active}
                    onClick={() => handleSelectKeycap(idx, 'click')}
                    className={`px-2.5 py-1.5 rounded-[2px] border font-tech font-medium text-[12px] transition-[background-color,border-color,color,transform] duration-150 cursor-pointer ${
                      active
                        ? 'border-[#9ec8f8] bg-[#2c4263] text-[#f0f5fa] translate-y-[1px]'
                        : 'border-[#384c69] bg-[#172233] text-[#b4c5db] shadow-[0_2px_0_#101824] hover:border-[#79aee8]'
                    }`}
                  >
                    <span className="text-[#9ec8f8] mr-1.5">{b.key}</span>
                    <span>{b.short}</span>
                  </button>
                );
              })}
            </div>
          </div>
        </article>
      </div>
    </GridBand>
  );
}
