import React, { useEffect, useRef, useState } from 'react';
import gsap from 'gsap';
import { LifecycleStage, NIKI_THEMES, NikiThemePalette } from '../../../entities/niki';
import { ProviderIcon, TerminalTitleBar } from '../../../shared';

interface LifecycleDiagramProps {
  stageId: LifecycleStage['id'];
}

const PHASE_GLYPHS = ['◐', '◓', '◑', '◒'];

const STAGE_THEMES: NikiThemePalette[] = NIKI_THEMES;

export const LifecycleDiagram = React.memo(function LifecycleDiagram({
  stageId,
}: LifecycleDiagramProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;

    const ctx = gsap.context(() => {
      gsap.fromTo(
        el.querySelector('[data-stage-scene]'),
        { opacity: 0, y: 8 },
        { opacity: 1, y: 0, duration: 0.32, ease: 'power2.out' }
      );
    }, el);

    return () => ctx.revert();
  }, [stageId]);

  return (
    <div
      ref={containerRef}
      data-testid={`lifecycle-diagram-${stageId}`}
      className="dark relative w-full h-[330px] sm:h-[404px] border border-grid bg-[#06070a] shadow-lg overflow-hidden select-none"
    >
      <div
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_25%_15%,rgba(62,102,248,0.16),transparent_62%)]"
        aria-hidden="true"
      />
      <div data-stage-scene="" className="relative z-10 w-full h-full">
        {stageId === 'boot' && <BootStage />}
        {stageId === 'still' && <StillStage />}
        {stageId === 'theme' && <ThemeStage />}
        {stageId === 'bind' && <BindStage />}
        {stageId === 'scale' && <ScaleStage />}
        {stageId === 'route' && <RouteStage />}
      </div>
    </div>
  );
});

function BootStage() {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [glyphIdx, setGlyphIdx] = useState(0);

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;

    let dir = 1;
    let idx = 0;
    const timer = window.setInterval(() => {
      idx += dir;
      if (idx >= PHASE_GLYPHS.length - 1 || idx <= 0) dir *= -1;
      setGlyphIdx(idx);
    }, 320);

    const ctx = gsap.context(() => {
      const tl = gsap.timeline({ defaults: { ease: 'power2.out' } });
      tl.from('.boot-beat', { opacity: 0, y: 7, stagger: 0.14, duration: 0.36 })
        .from('.boot-hairline', { scaleX: 0, transformOrigin: 'left center', duration: 0.45 }, '-=0.12')
        .from('.boot-satellite', { opacity: 0, y: 10, duration: 0.38 }, '-=0.2');

      gsap.to('.boot-caret', {
        opacity: 0.15,
        repeat: -1,
        yoyo: true,
        duration: 0.55,
        ease: 'steps(1)',
      });
    }, el);

    return () => {
      ctx.revert();
      window.clearInterval(timer);
    };
  }, []);

  return (
    <div ref={rootRef} className="relative w-full h-full">
      <div className="absolute left-5 sm:left-8 top-5 sm:top-6 -right-8 -bottom-6 rounded-tl-[2px] border-t border-l border-[#282e46] bg-[#0d1018] shadow-[-10px_20px_50px_rgba(0,0,0,0.7)] overflow-hidden flex flex-col">
        <TerminalTitleBar title="~/.local/bin/niki" rightLabel="one binary" />

        <div className="p-5 sm:p-7 flex-1 flex flex-col justify-between font-tech">
          <div className="space-y-4">
            <div className="space-y-1 text-[10.5px] text-[#7f93ad]">
              <div>verified SHA256SUMS</div>
              <div>binary: niki · no runtime</div>
            </div>

            <div className="boot-beat flex items-center gap-2.5 text-[12.5px] text-[#dce7f5]">
              <span className="text-[#9ec8f8]">$</span>
              <span>niki</span>
            </div>

            <div className="boot-beat pt-1 flex items-center gap-3 text-[15px] text-[#f0f5fa]">
              <span className="text-[#9ec8f8] text-[18px] leading-none" aria-hidden="true">
                {PHASE_GLYPHS[glyphIdx]}
              </span>
              <span className="font-display font-normal tracking-[-0.02em]">Niki Agent</span>
            </div>

            <div className="boot-beat space-y-1.5 pt-1 text-[11px] text-[#9bb0c9]">
              <div className="flex items-center gap-2">
                <span className="text-[#9ec8f8]">✓</span>
                <span>provider: not set · theme: niki</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[#9ec8f8]">✓</span>
                <span>keymap: every binding bound · session: new</span>
              </div>
            </div>
          </div>

          <div className="pb-12 pt-5 space-y-2.5">
            <div className="boot-hairline h-px w-full bg-gradient-to-r from-[#9ec8f8] via-[#4a6282] to-transparent" />
            <div className="boot-beat flex items-center gap-2.5 text-[12.5px] text-[#e8eff5]">
              <span className="text-[#9ec8f8]">›</span>
              <span className="text-[#b5c6da]">Refactor keymap bindings</span>
              <span className="boot-caret inline-block w-2 h-4 bg-[#9ec8f8]" aria-hidden="true" />
            </div>
            <div className="text-[10px] text-[#7d91ab]">
              enter send · /theme · shift+tab mode · /resume
            </div>
          </div>
        </div>
      </div>

      <div
        className="pointer-events-none absolute inset-x-0 bottom-0 h-14 bg-gradient-to-t from-[#06070a] to-transparent"
        aria-hidden="true"
      />

      <div className="boot-satellite absolute left-4 bottom-4 z-20 rounded-[2px] border border-[#3c4f6b] bg-[#212d40]/95 px-3.5 py-2 shadow-xl font-tech text-[11px] flex items-center gap-3">
        <span className="text-[#eef4fa]">Composer ready</span>
        <span className="text-[#8295ae]">·</span>
        <span className="text-[#9ec8f8]">no API key</span>
      </div>
    </div>
  );
}

function StillStage() {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [reducedSim, setReducedSim] = useState(false);

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const osReduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (osReduced || reducedSim) return;

    const ctx = gsap.context(() => {
      gsap.to('.still-jitter', {
        scaleY: () => 0.3 + Math.random() * 0.85,
        transformOrigin: 'bottom center',
        duration: 0.44,
        stagger: { each: 0.07, repeat: -1, yoyo: true },
        ease: 'sine.inOut',
      });
      gsap.to('.still-sweep', {
        x: 68,
        duration: 1.5,
        repeat: -1,
        yoyo: true,
        ease: 'power1.inOut',
      });
    }, el);

    return () => ctx.revert();
  }, [reducedSim]);

  return (
    <div ref={rootRef} className="relative w-full h-full">
      <div className="absolute -left-6 top-5 sm:top-6 right-10 sm:right-14 -bottom-6 rounded-tr-[2px] border-t border-r border-[#282e46] bg-[#0d1018] overflow-hidden flex flex-col">
        <TerminalTitleBar title="niki · release build" rightLabel="cargo" />
        <div className="p-5 sm:p-6 pl-10 sm:pl-12 space-y-5 font-tech">
          <div className="space-y-1.5 text-[10.5px] text-[#8da1ba]">
            <div>[build] cargo build --release -p niki --bin niki</div>
            <div>[link] turn loop · tools · policy · tui</div>
            <div>[install] SHA256SUMS verified · niki installed</div>
          </div>

          <div className="pt-4 sm:pt-7 space-y-2.5">
            <div className="h-px w-full bg-[#455a78]" />
            <div className="flex items-center justify-between text-[12px]">
              <span className="text-[#dce7f5]">
                <span className="text-[#9ec8f8] mr-2">›</span>./target/release/niki
              </span>
            </div>
          </div>

          <div className="pt-1 flex items-center gap-3 text-[10.5px] text-[#8ea0b8]">
            <span>◐ ◓ ◑ ◒</span>
            <div className="relative w-20 h-1.5 rounded-[2px] bg-[#243248] overflow-hidden">
              <span className="still-sweep absolute left-0 top-0 h-1.5 w-3 rounded-[2px] bg-[#9ec8f8]" />
            </div>
          </div>
        </div>
      </div>

      <div className="absolute right-3 sm:right-5 top-10 sm:top-12 w-[218px] sm:w-[254px] z-20 rounded-[2px] border border-[#3d5170] bg-[#212e42]/95 p-3.5 sm:p-4 shadow-2xl font-tech space-y-3">
        <div className="flex items-center justify-between text-[10px]">
          <span className="text-[#9ec8f8]">Runtime deps</span>
          <button
            type="button"
            aria-pressed={reducedSim}
            onClick={() => setReducedSim((v) => !v)}
            className="px-2 py-0.5 rounded-[2px] border border-[#3b4f6c] bg-[#162030] text-[9.5px] text-[#dce7f5] hover:border-[#9ec8f8] cursor-pointer"
          >
            {reducedSim ? 'REDUCED=1' : 'REDUCED=0'}
          </button>
        </div>

        <div className="space-y-1.5">
          <div className="flex items-center justify-between text-[10.5px] text-[#9eb0c7]">
            <span>Runtime setup</span>
            <span className="tabular-nums">none</span>
          </div>
          <div className="h-9 flex items-end gap-1 px-1 rounded-[2px] bg-[#162030] border border-[#2c3c54] py-1.5">
            {[62, 44, 80, 52, 88, 48, 76, 56, 84, 50].map((h, i) => (
              <div
                key={i}
                style={{ height: `${h}%` }}
                className="still-jitter flex-1 bg-[#4a6284]"
              />
            ))}
          </div>
        </div>

        <div className="rounded-[2px] border border-[#9ec8f8]/55 bg-[#273954] px-3 py-2 space-y-2">
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-[#f0f5fa] font-medium">Niki</span>
            <span className="text-[#9ec8f8] font-medium tabular-nums">1 file</span>
          </div>
          <div className="h-0.5 w-full bg-[#9ec8f8]" />
        </div>
      </div>

      <div
        className="pointer-events-none absolute inset-x-0 bottom-0 h-12 bg-gradient-to-t from-[#06070a] to-transparent"
        aria-hidden="true"
      />
    </div>
  );
}

function ThemeStage() {
  const [selected, setSelected] = useState<NikiThemePalette>(NIKI_THEMES[0]);
  const previewRef = useRef<HTMLDivElement | null>(null);
  const ratioRef = useRef<HTMLSpanElement | null>(null);

  const handleSelectTheme = (palette: NikiThemePalette) => {
    setSelected(palette);
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;

    if (previewRef.current) {
      gsap.fromTo(
        previewRef.current,
        { opacity: 0.9 },
        { opacity: 1, duration: 0.24, ease: 'power2.out' }
      );
    }
    if (ratioRef.current) {
      gsap.fromTo(
        ratioRef.current,
        { y: 6, opacity: 0 },
        { y: 0, opacity: 1, duration: 0.25, ease: 'power2.out' }
      );
    }
  };

  return (
    <div className="relative w-full h-full">
      <div
        ref={previewRef}
        data-testid="theme-preview-card"
        data-active-theme={selected.id}
        style={{
          backgroundColor: selected.bg,
          borderColor: selected.border,
          color: selected.text,
        }}
        className="absolute left-10 sm:left-14 top-5 sm:top-6 -right-6 -bottom-6 rounded-tl-[2px] border-t border-l transition-colors duration-250 overflow-hidden flex flex-col shadow-[-10px_20px_50px_rgba(0,0,0,0.5)]"
      >
        <TerminalTitleBar
          title={`◐ /theme ${selected.name}`}
          rightLabel="0 violations"
          borderColor={selected.border}
          bgColor={selected.surface}
          textColor={selected.muted}
        />

        <div className="py-5 pr-8 sm:pr-10 pl-[116px] sm:pl-[124px] flex-1 flex flex-col justify-between font-tech">
          <div className="space-y-1 pt-0.5 text-[10.5px]">
            <div style={{ color: selected.muted }}>
              palette: <span style={{ color: selected.text }}>{selected.name}</span> · wcag aa+
            </div>
            <div style={{ color: selected.accent }}>
              + focus: {selected.accent} · bg: {selected.bg}
            </div>
          </div>

          <div
            style={{ backgroundColor: selected.surface, borderColor: selected.border }}
            className="rounded-[2px] border px-3.5 py-3 max-w-[310px] space-y-1.5 transition-colors duration-250"
          >
            <div className="flex items-center justify-between gap-2 text-[11px]">
              <span style={{ color: selected.muted }}>Contrast</span>
              <span
                ref={ratioRef}
                style={{ color: selected.accent }}
                className="font-semibold tabular-nums text-[13px]"
              >
                {selected.tag}
              </span>
            </div>
            <div className="text-[11px] leading-relaxed" style={{ color: selected.text }}>
              {selected.description}
            </div>
          </div>

          <div className="pb-10 space-y-2">
            <div className="h-px w-full" style={{ backgroundColor: selected.border }} />
            <div className="text-[11.5px] flex items-center gap-2">
              <span style={{ color: selected.accent }}>›</span>
              <span style={{ color: selected.text }}>apply patch</span>
            </div>
          </div>
        </div>
      </div>

      <div
        role="group"
        aria-label="Runtime theme selector"
        className="absolute left-3 top-14 sm:top-16 z-20 w-[134px] rounded-[2px] border border-[#3d5170] bg-[#1e2a3d]/95 p-2 shadow-2xl font-tech space-y-1"
      >
        <div className="px-2 py-1 text-[10px] text-[#9ec8f8] border-b border-[#2e3f57] mb-1">
          /theme
        </div>
        {STAGE_THEMES.map((t) => {
          const active = t.id === selected.id;
          return (
            <button
              key={t.id}
              type="button"
              aria-pressed={active}
              onClick={() => handleSelectTheme(t)}
              className={`w-full text-left px-2.5 py-1.5 rounded-[2px] text-[10.5px] flex items-center justify-between transition-colors cursor-pointer ${
                active
                  ? 'bg-[#2c4160] text-[#f0f5fa]'
                  : 'text-[#9eb0c7] hover:bg-[#26364f] hover:text-[#f0f5fa]'
              }`}
            >
              <span>{t.name}</span>
              <span
                style={{ backgroundColor: t.accent }}
                className="w-2 h-2 rounded-[2px] shrink-0"
                aria-hidden="true"
              />
            </button>
          );
        })}
      </div>
    </div>
  );
}

function BindStage() {
  const [approved, setApproved] = useState(true);
  const [autoMode, setAutoMode] = useState(false);
  const key1Ref = useRef<HTMLButtonElement | null>(null);

  const handleToggleApprove = () => {
    setApproved((v) => !v);
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (!reduced && key1Ref.current) {
      gsap.fromTo(
        key1Ref.current,
        { y: 2, scale: 0.96 },
        { y: 0, scale: 1, duration: 0.22, ease: 'power2.out' }
      );
    }
  };

  return (
    <div className="relative w-full h-full">
      <div className="absolute left-5 sm:left-8 top-5 sm:top-6 -right-8 -bottom-6 rounded-tl-[2px] border-t border-l border-[#282e46] bg-[#0d1018] overflow-hidden flex flex-col">
        <TerminalTitleBar title="keymap · live bindings" rightLabel="every key bound" />

        <div className="p-5 sm:p-6 space-y-3.5 font-tech">
          <div className="text-[10.5px] text-[#8da1ba]">
            $ cargo test -j 4 -p niki-tui
          </div>

          <div
            className={`rounded-[2px] border px-4 py-3 max-w-[360px] transition-colors duration-200 ${
              approved
                ? 'border-[#9ec8f8]/60 bg-[#243650]'
                : 'border-[#334661] bg-[#1e2a3d]'
            }`}
          >
            <div className="flex items-center justify-between gap-3 text-[11.5px]">
              <span className="text-[#f0f5fa]">write_file · lib.rs</span>
              <span
                className={`px-2 py-0.5 rounded-[2px] text-[10px] transition-colors ${
                  approved
                    ? 'bg-[#9ec8f8] text-[#141c2a] font-medium'
                    : 'bg-[#2c3d57] text-[#c5d7eb]'
                }`}
              >
                {approved ? 'Approved' : 'Pending'}
              </span>
            </div>

            <div className="overflow-hidden">
              <div className="mt-3 pt-2.5 border-t border-[#334966] space-y-1.5 text-[10.5px] text-[#adc0d6]">
                <div>+ approval_yes</div>
                <div className="text-[#9ec8f8]">✓ handler verified in cargo test</div>
              </div>
            </div>
          </div>

          <div className="space-y-1 pt-0.5 text-[10.5px] text-[#8da1ba]">
            <div className="text-[#9ec8f8]">passed · no unbound shortcuts</div>
            <div>hint_row: synced with the binding registry</div>
          </div>
        </div>
      </div>

      <div className="absolute right-4 sm:right-5 bottom-4 z-20 rounded-[2px] border border-[#3d5170] bg-[#1e2a3d]/95 p-2 shadow-2xl flex items-center gap-2 font-tech text-[11px]">
        <button
          ref={key1Ref}
          type="button"
          aria-pressed={approved}
          onClick={handleToggleApprove}
          className="px-2.5 py-1.5 rounded-[2px] border border-[#4a6285] bg-[#293d5a] text-[#f0f5fa] shadow-[0_2px_0_#1a273b] hover:border-[#9ec8f8] cursor-pointer"
        >
          <span className="text-[#9ec8f8] mr-1.5">y</span>
          {approved ? 'Undo' : 'Approve'}
        </button>
        <button
          type="button"
          aria-pressed={autoMode}
          onClick={() => setAutoMode((v) => !v)}
          className="px-2.5 py-1.5 rounded-[2px] border border-[#374b68] bg-[#1b2739] text-[#dce7f5] shadow-[0_2px_0_#131b29] hover:border-[#9ec8f8] cursor-pointer"
        >
          <span className="text-[#9ec8f8] mr-1.5">shift+tab</span>
          {autoMode ? 'Mode: auto' : 'Mode: manual'}
        </button>
      </div>
    </div>
  );
}

function ScaleStage() {
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;

    const ctx = gsap.context(() => {
      gsap.fromTo(
        '.scale-meter-fill',
        { scaleX: 0.15, transformOrigin: 'left center' },
        { scaleX: 1, duration: 0.72, stagger: 0.14, ease: 'power3.out' }
      );
    }, el);

    return () => ctx.revert();
  }, []);

  return (
    <div ref={rootRef} className="relative w-full h-full">
      <div className="absolute left-5 sm:left-7 -top-4 right-20 sm:right-28 -bottom-4 rounded-[2px] border border-[#282e46] bg-[#0d1018] p-4 sm:p-5 space-y-2.5 font-tech">
        <div className="rounded-[2px] border border-dashed border-[#242a40] bg-[#080a10] px-3.5 py-2 opacity-70 space-y-1">
          <div className="text-[10px] text-[#8ea0b8]">Older rows folded by /compact</div>
          <div className="text-[10px] text-[#6d7f99] truncate">
            stored: 128 turns in the session db
          </div>
        </div>

        <div className="rounded-[2px] border border-[#5db8ff]/60 bg-[#131d36] px-4 py-3 space-y-1.5 shadow-lg">
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-[#f4f6fc] font-medium">Active Session</span>
            <span className="text-[#5db8ff] tabular-nums">resumable</span>
          </div>
          <div className="space-y-1 text-[10px] text-[#9eb6d4]">
            <div>resume: /resume 7f3a · by id or index</div>
            <div>view: older rows folded, store intact</div>
          </div>
          <div className="text-[10px] text-[#b8cde4]">128 turns stored</div>
        </div>

        <div className="rounded-[2px] border border-[#282e46] bg-[#111524] px-3.5 py-2 space-y-1">
          <div className="text-[10.5px] text-[#dce7f5]">New turn · streaming</div>
          <div className="text-[10px] text-[#8ea0b8]">delta: +48 tokens · session db</div>
        </div>
      </div>

      <div
        className="pointer-events-none absolute inset-x-0 top-0 h-12 bg-gradient-to-b from-[#06070a] to-transparent z-10"
        aria-hidden="true"
      />
      <div
        className="pointer-events-none absolute inset-x-0 bottom-0 h-14 bg-gradient-to-t from-[#06070a] to-transparent z-10"
        aria-hidden="true"
      />

      <div className="absolute right-3 sm:right-5 bottom-5 sm:bottom-8 z-20 w-[198px] sm:w-[210px] rounded-[2px] border border-[#3d5170] bg-[#202d41]/95 p-3.5 sm:p-4 shadow-2xl font-tech space-y-3">
        <div className="text-[10px] text-[#9ec8f8]">Session Store</div>
        <div className="space-y-2">
          <div>
            <div className="flex justify-between text-[10px] text-[#9eb0c7] mb-1">
              <span>Folded rows</span>
              <span className="tabular-nums">kept</span>
            </div>
            <div className="h-1.5 bg-[#162030] overflow-hidden">
              <div className="scale-meter-fill h-full w-full bg-[#4c6486]" />
            </div>
          </div>
          <div>
            <div className="flex justify-between text-[10.5px] text-[#f0f5fa] mb-1">
              <span>Stored turns</span>
              <span className="text-[#9ec8f8] tabular-nums">128</span>
            </div>
            <div className="h-1.5 bg-[#162030] overflow-hidden">
              <div className="scale-meter-fill h-full w-[64%] bg-[#9ec8f8]" />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function RouteStage() {
  const routes = [
    { id: 'kimi', name: 'Kimi', mode: 'config.toml', model: 'kimi-k2.7-code', base: 'https://api.moonshot.ai/v1', env: 'MOONSHOT_API_KEY', protocol: 'openai' },
    { id: 'kilo', name: 'Kilo', mode: 'config.toml', model: 'claude-opus-4.7', base: 'https://api.kilo.ai/api/gateway', env: 'KILO_API_KEY', protocol: 'openai' },
    { id: 'opencode', name: 'OpenCode Zen', mode: 'config.toml', model: 'deepseek-v4-pro', base: 'https://opencode.ai/zen/v1', env: 'OPENCODE_API_KEY', protocol: 'openai' },
  ];
  const [activeRoute, setActiveRoute] = useState(1);

  useEffect(() => {
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;
    const id = window.setInterval(() => {
      setActiveRoute((prev) => (prev + 1) % routes.length);
    }, 2400);
    return () => window.clearInterval(id);
  }, []);

  const current = routes[activeRoute];

  return (
    <div className="relative w-full h-full">
      <div className="absolute -left-4 top-5 sm:top-6 right-16 sm:right-20 -bottom-6 rounded-tr-[2px] border-t border-r border-[#282e46] bg-[#0d1018] overflow-hidden flex flex-col">
        <TerminalTitleBar title="~/.niki/config.toml" rightLabel="one block" />
        <div className="p-5 sm:p-6 pl-8 sm:pl-9 font-tech text-[11px] sm:text-[11.5px] leading-[1.7] space-y-1 text-[#a5b9d1]">
          <div className="text-[#9ec8f8]">[providers.{current.id}]</div>
          <div>
            <span className="text-[#8295ae]">protocol = </span>
            <span className="text-[#eef4fa]">&quot;{current.protocol}&quot;</span>
          </div>
          <div>
            <span className="text-[#8295ae]">base_url = </span>
            <span className="text-[#eef4fa]">&quot;{current.base}&quot;</span>
          </div>
          <div>
            <span className="text-[#8295ae]">api_key_env = </span>
            <span className="text-[#eef4fa]">&quot;{current.env}&quot;</span>
          </div>
          <div className="pt-2 space-y-1 text-[11px]">
            <div>
              <span className="text-[#8295ae]">models = </span>
              <span className="text-[#eef4fa]">[&quot;{current.model}&quot;]</span>
            </div>
          </div>
        </div>
      </div>

      <div className="absolute right-3 sm:right-5 top-12 sm:top-16 z-20 w-[204px] sm:w-[228px] rounded-[2px] border border-[#3d5170] bg-[#141c2b]/95 p-3 sm:p-3.5 shadow-2xl font-tech space-y-2">
        <div className="flex items-center justify-between text-[10px] pb-1 border-b border-[#2f4059]">
          <span className="text-[#9ec8f8]">◐ Gateways</span>
          <span className="text-[#9eb0c7]">config.toml</span>
        </div>

        {routes.map((r, idx) => {
          const isSelected = idx === activeRoute;
          return (
            <button
              key={r.name}
              type="button"
              aria-pressed={isSelected}
              onClick={() => setActiveRoute(idx)}
              className={`w-full text-left rounded-[2px] border px-2.5 py-2 transition-colors cursor-pointer flex items-center justify-between gap-2 ${
                isSelected
                  ? 'border-[#9ec8f8] bg-[#22334d] text-[#f8fafc]'
                  : 'border-[#2f4059] bg-[#101724] text-[#b7c5d8] hover:text-[#f8fafc]'
              }`}
            >
              <span className="inline-flex items-center gap-1.5 text-[11px] font-medium">
                <ProviderIcon id={r.id} className="w-3.5 h-3.5" />
                <span>{r.name}</span>
              </span>
              <span className="text-[9.5px] text-[#9ec8f8] shrink-0">{r.mode}</span>
            </button>
          );
        })}

        <div className="pt-1 text-[10px] text-[#dce7f5] flex items-center justify-between">
          <span className="text-[#8295ae]">Active</span>
          <span>{current.model}</span>
        </div>
      </div>
    </div>
  );
}
