import React, { useEffect, useRef, useState } from 'react';
import { X, Check, Copy, ArrowUpRight } from 'lucide-react';
import {
  BenchmarkStory,
  EcosystemLogoItem,
  FEATURE_DETAILS,
  INSTALL_COMMAND,
  PillarCardData,
  TEST_COMMAND,
  WINDOWS_INSTALL_COMMAND,
} from '../../../entities/niki';
import { BTN, CARD, ProviderIcon } from '../../../shared';

export type ActiveModal =
  | { type: 'install'; customCommand?: string }
  | { type: 'checklist' }
  | { type: 'github' }
  | { type: 'detail'; detailId: string }
  | { type: 'pillar'; pillar: PillarCardData }
  | { type: 'benchmark'; benchmark: BenchmarkStory }
  | { type: 'provider'; item: EcosystemLogoItem }
  | null;

interface InteractiveModalsProps {
  modal: ActiveModal;
  onClose: () => void;
  onOpenInstall: (customCommand?: string) => void;
}

const REPO_URL = 'https://github.com/RavaniRoshan/niki-agent';

function getModalHeaderLabel(modal: Exclude<ActiveModal, null>): string {
  switch (modal.type) {
    case 'install':
      return 'INSTALL · LINUX, WSL & WINDOWS';
    case 'checklist':
      return 'DOCS · WHAT IS VERIFIED';
    case 'github':
      return 'OPEN SOURCE · MIT LICENSE';
    case 'detail':
      return (FEATURE_DETAILS[modal.detailId]?.category || 'SPECIFICATION').toUpperCase();
    case 'pillar':
      return modal.pillar.packageName.toUpperCase();
    case 'benchmark':
      return `VERIFIED BASELINE · ${modal.benchmark.badge.toUpperCase()}`;
    case 'provider':
      return `${modal.item.actionType === 'Config' ? 'CONFIG-ONLY GATEWAY' : 'BUILT-IN PROVIDER'} · ${modal.item.name.toUpperCase()}`;
  }
}

export function InteractiveModals({
  modal,
  onClose,
  onOpenInstall,
}: InteractiveModalsProps) {
  const closeBtnRef = useRef<HTMLButtonElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!modal) return;
    previousFocusRef.current = document.activeElement as HTMLElement | null;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    // Move focus to close button for predictable keyboard navigation
    closeBtnRef.current?.focus();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', onKeyDown);

    return () => {
      document.body.style.overflow = prevOverflow;
      document.removeEventListener('keydown', onKeyDown);
      previousFocusRef.current?.focus?.();
    };
  }, [modal, onClose]);

  if (!modal) return null;

  const headerLabel = getModalHeaderLabel(modal);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="niki-modal-title"
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className={CARD.modal}>
        {/* Unified Architectural Hairline Header Bar matching SectionEyebrow */}
        <div className="flex items-center justify-between gap-3 h-11 pl-5 pr-2 border-b border-grid bg-background/60">
          <div className="flex items-center gap-2.5 min-w-0 font-tech font-medium text-[11px] tracking-[0.08em] text-chart-1 dark:text-chart-2">
            <svg
              viewBox="0 0 12 12"
              className="w-3 h-3 shrink-0"
              aria-hidden="true"
              focusable="false"
            >
              <path d="M6 1 11 6 6 11 1 6Z" fill="none" stroke="currentColor" strokeWidth="1.25" />
            </svg>
            <span className="truncate">{headerLabel}</span>
          </div>

          <button
            ref={closeBtnRef}
            type="button"
            onClick={onClose}
            aria-label="Close modal"
            className={`${BTN.base} ${BTN.icon} border-transparent bg-transparent`}
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-6 sm:p-7">
          {modal.type === 'install' && <InstallContent customCommand={modal.customCommand} />}
          {modal.type === 'checklist' && <ChecklistContent onOpenInstall={() => onOpenInstall()} />}
          {modal.type === 'github' && <GithubContent onOpenInstall={() => onOpenInstall()} />}
          {modal.type === 'detail' && (
            <DetailContent detailId={modal.detailId} onOpenInstall={() => onOpenInstall()} />
          )}
          {modal.type === 'pillar' && (
            <PillarContent pillar={modal.pillar} onOpenInstall={() => onOpenInstall()} />
          )}
          {modal.type === 'benchmark' && (
            <BenchmarkContent benchmark={modal.benchmark} onOpenInstall={() => onOpenInstall()} />
          )}
          {modal.type === 'provider' && <ProviderContent item={modal.item} />}
        </div>
      </div>
    </div>
  );
}

function CopySnippetBlock({
  label,
  code,
}: {
  label?: string;
  code: string;
}) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
      }
    };
  }, []);

  const handleCopy = () => {
    navigator.clipboard?.writeText(code).catch(() => {});
    setCopied(true);
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
    }
    timerRef.current = window.setTimeout(() => {
      setCopied(false);
      timerRef.current = null;
    }, 1800);
  };

  return (
    <div className="rounded-[2px] border border-grid bg-background overflow-hidden">
      <div className="flex items-center justify-between px-4 py-2 border-b border-grid bg-muted/50">
        <span className="font-tech text-[11px] text-muted-foreground">{label || 'Terminal'}</span>
        <button
          type="button"
          onClick={handleCopy}
          aria-label={`Copy ${label || 'snippet'}`}
          className={`${BTN.base} h-7 px-2.5 rounded-[2px] border border-grid bg-card text-chart-1 dark:text-chart-2 hover:bg-muted hover:border-grid-strong font-tech text-[11px]`}
        >
          {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
          <span>{copied ? 'Copied' : 'Copy'}</span>
        </button>
      </div>
      <pre className="p-4 font-tech text-[12.5px] leading-relaxed text-foreground overflow-x-auto">
        {code}
      </pre>
    </div>
  );
}

function InstallContent({ customCommand }: { customCommand?: string }) {
  const quickstartCmd = customCommand || `${INSTALL_COMMAND}
niki`;

  return (
    <div className="space-y-5">
      <div>
        <h2
          id="niki-modal-title"
          className="font-display text-[28px] sm:text-[32px] font-semibold tracking-[-0.03em] leading-[1.12] text-foreground"
        >
          Install Niki Agent
        </h2>
        <p className="mt-2 text-[15px] leading-[1.55] text-muted-foreground">
          The installer verifies SHA256SUMS, installs one self-contained binary, and
          boots straight into a working composer. No account and no API key are needed to look
          around.
        </p>
      </div>

      <CopySnippetBlock label="Install & launch" code={quickstartCmd} />

      <div className="rounded-[2px] border border-grid bg-muted/45 p-4 font-tech text-[12px] space-y-1.5 text-muted-foreground">
        <div className="text-chart-1 dark:text-chart-2">Windows PowerShell</div>
        <div className="text-foreground break-all">{WINDOWS_INSTALL_COMMAND}</div>
        <div className="pt-2 text-chart-1 dark:text-chart-2">macOS arm64 and x86_64</div>
        <div className="text-foreground break-all">{INSTALL_COMMAND}</div>
        <div className="pt-2 text-chart-1 dark:text-chart-2">Verify the workspace without a network</div>
        <div className="text-foreground">{TEST_COMMAND}</div>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3 pt-2 border-t border-grid">
        <span className="font-tech text-[12px] text-muted-foreground">MIT Licensed · Branch niki-agent</span>
        <a
          href={REPO_URL}
          target="_blank"
          rel="noopener noreferrer"
          className={`${BTN.base} ${BTN.primary}`}
        >
          <span>Open on GitHub</span>
          <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
        </a>
      </div>
    </div>
  );
}

function ChecklistContent({ onOpenInstall }: { onOpenInstall: () => void }) {
  return (
    <div className="space-y-5">
      <div>
        <h2
          id="niki-modal-title"
          className="font-display text-[28px] sm:text-[32px] font-semibold tracking-[-0.03em] leading-[1.12] text-foreground"
        >
          What the tests verify
        </h2>
        <p className="mt-2 text-[15px] leading-[1.55] text-muted-foreground">
          Every claim on this page has a command behind it in the repository. Nothing here needs
          an API key or the network to check.
        </p>
      </div>

      <div className="grid grid-cols-3 border border-grid divide-x divide-grid font-tech">
        <div className="bg-muted/70 p-4">
          <div className="text-[24px] font-semibold text-chart-1 dark:text-chart-2 tabular-nums">1</div>
          <div className="text-[11px] text-foreground mt-0.5">BINARY</div>
        </div>
        <div className="bg-muted/40 p-4">
          <div className="text-[24px] font-semibold text-foreground tabular-nums">4</div>
          <div className="text-[11px] text-muted-foreground mt-0.5">THEMES</div>
        </div>
        <div className="bg-background p-4">
          <div className="text-[24px] font-semibold text-muted-foreground tabular-nums">3</div>
          <div className="text-[11px] text-muted-foreground mt-0.5">APPROVAL MODES</div>
        </div>
      </div>

      <ul className="space-y-2.5 text-[14.5px] leading-[1.55] text-muted-foreground">
        <li className="flex items-start gap-2.5">
          <span className="mt-2 w-1.5 h-1.5 rotate-45 bg-chart-1 dark:bg-chart-2 shrink-0" aria-hidden="true" />
          <span>The workspace suite runs with cargo test -j 4 --workspace, with no network or API key.</span>
        </li>
        <li className="flex items-start gap-2.5">
          <span className="mt-2 w-1.5 h-1.5 rotate-45 bg-chart-1 dark:bg-chart-2 shrink-0" aria-hidden="true" />
          <span>Four palettes, each measured at 4.5:1 text and 3:1 UI glyph contrast against every surface.</span>
        </li>
        <li className="flex items-start gap-2.5">
          <span className="mt-2 w-1.5 h-1.5 rotate-45 bg-chart-1 dark:bg-chart-2 shrink-0" aria-hidden="true" />
          <span>Every advertised shortcut parses back to its action in the binding registry test.</span>
        </li>
      </ul>

      <div className="pt-4 border-t border-grid flex flex-wrap items-center justify-between gap-3">
        <a
          href={`${REPO_URL}/blob/niki-agent/docs/niki/INDEPENDENCE.md`}
          target="_blank"
          rel="noopener noreferrer"
          className={`${BTN.base} ${BTN.secondary}`}
        >
          <span>Read INDEPENDENCE.md</span>
          <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
        </a>
        <button
          type="button"
          onClick={onOpenInstall}
          className={`${BTN.base} ${BTN.primary}`}
        >
          Install Niki
        </button>
      </div>
    </div>
  );
}

function GithubContent({ onOpenInstall }: { onOpenInstall: () => void }) {
  return (
    <div className="space-y-5">
      <div>
        <h2
          id="niki-modal-title"
          className="font-display text-[28px] sm:text-[32px] font-semibold tracking-[-0.03em] leading-[1.12] text-foreground"
        >
          RavaniRoshan/niki-agent
        </h2>
        <p className="mt-2 text-[15px] leading-[1.55] text-muted-foreground">
          A terminal coding agent written in Rust and shipped as one self-contained binary. The
          turn loop, tools, policy, sessions, MCP client, and TUI are crates in this workspace.
          Sessions stay on your machine.
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 border-t border-l border-grid font-tech text-[12px]">
        {[
          'docs/niki/ARCHITECTURE.md',
          'docs/niki/KEYMAP.md',
          'docs/niki/PROVIDERS.md',
          'docs/niki/INDEPENDENCE.md',
        ].map((docPath) => (
          <a
            key={docPath}
            href={`${REPO_URL}/blob/niki-agent/${docPath}`}
            target="_blank"
            rel="noopener noreferrer"
            className={`border-r border-b p-3.5 flex items-center justify-between ${CARD.cellInteractive}`}
          >
            <span>{docPath}</span>
            <ArrowUpRight className="w-3.5 h-3.5 text-chart-1 dark:text-chart-2" aria-hidden="true" />
          </a>
        ))}
      </div>

      <div className="pt-4 border-t border-grid flex flex-wrap items-center justify-end gap-2.5">
        <button
          type="button"
          onClick={onOpenInstall}
          className={`${BTN.base} ${BTN.secondary}`}
        >
          Install commands
        </button>
        <a
          href={REPO_URL}
          target="_blank"
          rel="noopener noreferrer"
          className={`${BTN.base} ${BTN.primary}`}
        >
          <span>Open repository</span>
          <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
        </a>
      </div>
    </div>
  );
}

function DetailContent({
  detailId,
  onOpenInstall,
}: {
  detailId: string;
  onOpenInstall: () => void;
}) {
  const detail = FEATURE_DETAILS[detailId] || FEATURE_DETAILS.install;

  return (
    <div className="space-y-5">
      <div>
        <h2
          id="niki-modal-title"
          className="font-display text-[28px] sm:text-[32px] font-semibold tracking-[-0.03em] leading-[1.12] text-foreground"
        >
          {detail.name}
        </h2>
        <p className="mt-2 font-feature-heading text-chart-1 dark:text-chart-2">{detail.tagline}</p>
      </div>

      <p className="text-[15px] leading-[1.6] text-muted-foreground">{detail.description}</p>

      <ul className="space-y-2.5 text-[14.5px] leading-[1.55] text-foreground">
        {detail.capabilities.map((cap) => (
          <li key={cap} className="flex items-start gap-2.5">
            <span className="mt-2 w-1.5 h-1.5 rotate-45 bg-chart-1 dark:bg-chart-2 shrink-0" aria-hidden="true" />
            <span>{cap}</span>
          </li>
        ))}
      </ul>

      {detail.codePreview && <CopySnippetBlock label="Command" code={detail.codePreview} />}

      <div className="pt-4 border-t border-grid flex flex-wrap items-center gap-2.5">
        <button
          type="button"
          onClick={onOpenInstall}
          className={`${BTN.base} ${BTN.primary}`}
        >
          Install Niki
        </button>
        <a
          href={REPO_URL}
          target="_blank"
          rel="noopener noreferrer"
          className={`${BTN.base} ${BTN.secondary}`}
        >
          <span>GitHub</span>
          <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
        </a>
      </div>
    </div>
  );
}

function PillarContent({
  pillar,
  onOpenInstall,
}: {
  pillar: PillarCardData;
  onOpenInstall: () => void;
}) {
  return (
    <div className="space-y-5">
      <div>
        <h2
          id="niki-modal-title"
          className="font-display text-[26px] sm:text-[30px] font-semibold tracking-[-0.03em] leading-[1.12] text-foreground"
        >
          {pillar.title}
        </h2>
        <p className="mt-2 text-[15px] leading-[1.55] text-muted-foreground">{pillar.subtitle}</p>
      </div>

      <CopySnippetBlock label="Quick command" code={pillar.installCmd} />
      <CopySnippetBlock label="Configuration & usage" code={pillar.codeSnippet} />

      <ul className="space-y-2 text-[14px] leading-[1.55] text-muted-foreground">
        {pillar.highlights.map((item) => (
          <li key={item} className="flex items-start gap-2.5">
            <span className="mt-2 w-1.5 h-1.5 rotate-45 bg-chart-1 dark:bg-chart-2 shrink-0" aria-hidden="true" />
            <span>{item}</span>
          </li>
        ))}
      </ul>

      <div className="pt-4 border-t border-grid flex justify-end">
        <button
          type="button"
          onClick={onOpenInstall}
          className={`${BTN.base} ${BTN.primary}`}
        >
          Install Niki
        </button>
      </div>
    </div>
  );
}

function BenchmarkContent({
  benchmark,
  onOpenInstall,
}: {
  benchmark: BenchmarkStory;
  onOpenInstall: () => void;
}) {
  return (
    <div className="space-y-5">
      <div>
        <h2
          id="niki-modal-title"
          className="font-display text-[26px] sm:text-[30px] font-semibold leading-[1.15] tracking-[-0.03em] text-foreground"
        >
          {benchmark.headline}
        </h2>
      </div>

      <div className="grid grid-cols-2 border border-grid divide-x divide-grid font-tech">
        <div className="bg-muted/40 p-4">
          <div className="text-[11px] text-muted-foreground">Before</div>
          <div className="mt-1 text-[14px] text-foreground">{benchmark.before}</div>
        </div>
        <div className="bg-muted/75 p-4">
          <div className="text-[11px] text-chart-1 dark:text-chart-2">After (Niki Agent)</div>
          <div className="mt-1 text-[14px] text-foreground font-semibold">{benchmark.after}</div>
        </div>
      </div>

      <div className="space-y-3 text-[14.5px] leading-[1.6] text-muted-foreground">
        <div>
          <h3 className="font-tech text-[11px] text-chart-1 dark:text-chart-2 mb-1 uppercase tracking-[0.06em]">
            Measurement Method
          </h3>
          <p>{benchmark.method}</p>
        </div>
        <div>
          <h3 className="font-tech text-[11px] text-chart-1 dark:text-chart-2 mb-1 uppercase tracking-[0.06em]">
            Verification
          </h3>
          <p className="text-foreground">{benchmark.verification}</p>
        </div>
      </div>

      <CopySnippetBlock label="Reproduce locally" code={benchmark.command} />

      <div className="pt-4 border-t border-grid flex flex-wrap items-center justify-between gap-3">
        <a
          href={`${REPO_URL}/tree/niki-agent/rust/crates`}
          target="_blank"
          rel="noopener noreferrer"
          className={`${BTN.base} ${BTN.secondary}`}
        >
          <span>Read the test suite</span>
          <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
        </a>
        <button
          type="button"
          onClick={onOpenInstall}
          className={`${BTN.base} ${BTN.primary}`}
        >
          Install Niki Agent
        </button>
      </div>
    </div>
  );
}

function ProviderContent({ item }: { item: EcosystemLogoItem }) {
  return (
    <div className="space-y-5">
      <div className="flex items-center gap-3.5">
        <span className="inline-flex items-center justify-center w-11 h-11 rounded-[2px] border border-grid-strong bg-background shrink-0">
          <ProviderIcon id={item.id} className="w-6 h-6" />
        </span>
        <h2
          id="niki-modal-title"
          className="font-display text-[26px] sm:text-[30px] font-semibold tracking-[-0.03em] leading-[1.12] text-foreground"
        >
          Connect {item.name} in Niki Agent
        </h2>
      </div>

      {item.summary && (
        <p className="text-[15px] leading-[1.6] text-muted-foreground">{item.summary}</p>
      )}

      {item.envVar && (
        <div className="rounded-[2px] border border-grid bg-muted/45 px-4 py-2.5 font-tech text-[12px] flex items-center justify-between">
          <span className="text-muted-foreground">Environment variable</span>
          <code className="text-chart-1 dark:text-chart-2">{item.envVar}</code>
        </div>
      )}

      {item.configSnippet && (
        <CopySnippetBlock
          label={item.actionType === 'Config' ? '~/.niki/config.toml' : 'Shell setup'}
          code={item.configSnippet}
        />
      )}

      <div className="pt-4 border-t border-grid flex justify-end">
        <a
          href={`${REPO_URL}/blob/niki-agent/docs/niki/PROVIDERS.md`}
          target="_blank"
          rel="noopener noreferrer"
          className={`${BTN.base} ${BTN.primary}`}
        >
          <span>Full PROVIDERS.md</span>
          <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
        </a>
      </div>
    </div>
  );
}
