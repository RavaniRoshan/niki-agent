import React, { useEffect, useMemo, useState } from 'react';
import { ArrowUpRight, ChevronDown, Download, Terminal } from 'lucide-react';
import { BTN, CARD, CopyButton } from '../../../shared';
import { VERSION as FALLBACK_VERSION } from '../../../shared/version';
import { PageShell } from '../../../widgets/page-shell';
import { INSTALL_COMMAND, WINDOWS_INSTALL_COMMAND } from '../../../entities/niki';
import { CHANGELOG } from '../../changelog/changelog.generated';

const GITHUB_URL = 'https://github.com/RavaniRoshan/niki-agent';
const DOCS_URL = 'https://niki-docs.vercel.app';
const RELEASES_URL = `${GITHUB_URL}/releases`;

const latestRelease = CHANGELOG.find((release) => !release.unreleased);
const VERSION = latestRelease?.version ?? FALLBACK_VERSION;
const RELEASED = latestRelease?.date ?? null;

type PlatformId = 'linux' | 'macos-arm64' | 'macos-x64' | 'windows';

interface Archive {
  id: PlatformId;
  os: 'Linux' | 'macOS' | 'Windows';
  arch: string;
  asset: string;
  note: string;
}

const ARCHIVES: Archive[] = [
  {
    id: 'linux',
    os: 'Linux',
    arch: 'x86_64',
    asset: `niki-${VERSION}-linux-x86_64.tar.gz`,
    note: 'glibc 2.31+ · also runs in WSL',
  },
  {
    id: 'macos-arm64',
    os: 'macOS',
    arch: 'Apple silicon',
    asset: `niki-${VERSION}-macos-aarch64.tar.gz`,
    note: 'macOS 11 or newer',
  },
  {
    id: 'macos-x64',
    os: 'macOS',
    arch: 'Intel',
    asset: `niki-${VERSION}-macos-x86_64.tar.gz`,
    note: 'macOS 11 or newer',
  },
  {
    id: 'windows',
    os: 'Windows',
    arch: 'x86_64',
    asset: `niki-${VERSION}-windows-x86_64.zip`,
    note: 'Windows 10 or newer',
  },
];

interface ReleaseAsset {
  name: string;
  size: number;
  digest: string | null;
  url: string;
}

function detectPlatform(): PlatformId {
  if (typeof navigator === 'undefined') return 'linux';
  const agent = navigator.userAgent;
  if (/Windows/i.test(agent)) return 'windows';
  if (/Mac OS X|Macintosh/i.test(agent)) return 'macos-arm64';
  return 'linux';
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '';
  const units = ['B', 'KiB', 'MiB', 'GiB'];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** exponent).toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

function formatDate(date: string | null): string {
  if (!date) return '';
  return new Date(`${date}T00:00:00Z`).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
}

function CodeLine({ command, label = 'Copy' }: { command: string; label?: string }) {
  return (
    <div className="flex min-w-0 items-center gap-2 overflow-hidden rounded-[2px] border border-grid bg-[#06070a] px-3 py-2.5">
      <span aria-hidden="true" className="font-mono text-[12px] text-chart-2">
        $
      </span>
      <code className="min-w-0 flex-1 overflow-x-auto whitespace-nowrap font-mono text-[12px] text-foreground/90">
        {command}
      </code>
      <CopyButton value={command} label={label} className="shrink-0" />
    </div>
  );
}

function ArchiveCard({
  archive,
  asset,
  highlighted,
}: {
  archive: Archive;
  asset?: ReleaseAsset;
  highlighted: boolean;
}) {
  const href = asset?.url ?? `${RELEASES_URL}/download/v${VERSION}/${archive.asset}`;
  const digest = asset?.digest?.replace(/^sha256:/, '') ?? '';

  return (
    <div
      className={`flex min-w-0 flex-col gap-3 border p-4 ${
        highlighted ? 'border-grid-strong bg-card/70' : CARD.cell
      }`}
    >
      <div className="flex items-baseline justify-between gap-2">
        <div>
          <p className="font-tech text-[12px] font-medium tracking-[0.04em] text-foreground">
            {archive.os} <span className="text-muted-foreground">/ {archive.arch}</span>
          </p>
          <p className="mt-0.5 text-[11.5px] text-muted-foreground">{archive.note}</p>
        </div>
        {highlighted && (
          <span className="font-tech text-[9.5px] uppercase tracking-[0.16em] text-chart-2">Detected</span>
        )}
      </div>

      <p className="truncate font-mono text-[11px] text-muted-foreground" title={archive.asset}>
        {archive.asset}
      </p>

      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-[11px] text-muted-foreground">
          {asset ? formatBytes(asset.size) : '\u00a0'}
        </span>
        <a
          href={href}
          className={`${BTN.base} ${highlighted ? BTN.primary : BTN.secondary} h-8 px-3 text-[12.5px]`}
          rel="noreferrer"
        >
          <Download className="w-3.5 h-3.5" aria-hidden="true" />
          Download
        </a>
      </div>

      {digest && (
        <div className="flex items-center gap-2 border-t border-grid pt-2">
          <code className="min-w-0 flex-1 truncate font-mono text-[10.5px] text-muted-foreground" title={digest}>
            sha256:{digest}
          </code>
          <CopyButton value={digest} label="Hash" />
        </div>
      )}
    </div>
  );
}

export function DownloadPage() {
  const [platform, setPlatform] = useState<PlatformId>('linux');
  const [shell, setShell] = useState<'unix' | 'windows'>('unix');
  const [assets, setAssets] = useState<Record<string, ReleaseAsset>>({});
  const [showAllMethods, setShowAllMethods] = useState(false);

  useEffect(() => {
    const detected = detectPlatform();
    setPlatform(detected);
    setShell(detected === 'windows' ? 'windows' : 'unix');
  }, []);

  useEffect(() => {
    let cancelled = false;
    fetch(`${GITHUB_URL.replace('github.com', 'api.github.com/repos')}/releases/latest`, {
      headers: { Accept: 'application/vnd.github+json' },
    })
      .then((response) => (response.ok ? response.json() : null))
      .then((release) => {
        if (cancelled || !release?.assets) return;
        const next: Record<string, ReleaseAsset> = {};
        for (const item of release.assets as Array<{
          name: string;
          size: number;
          digest?: string | null;
          browser_download_url: string;
        }>) {
          next[item.name] = {
            name: item.name,
            size: item.size,
            digest: item.digest ?? null,
            url: item.browser_download_url,
          };
        }
        setAssets(next);
      })
      .catch(() => {
        // The pinned links below work without the API.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const detectedArchive = useMemo(
    () => ARCHIVES.find((archive) => archive.id === platform) ?? ARCHIVES[0],
    [platform]
  );
  const detectedAsset = assets[detectedArchive.asset];

  return (
    <PageShell
      title="Download"
      description={`Download Niki Agent ${VERSION}: one native binary for Linux, macOS, and Windows, with a one-line installer and SHA256SUMS.`}
    >
      <div className="mx-auto w-full max-w-[1100px] px-5 py-12 md:px-8 md:py-16">
        <p className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">Download</p>
        <h1 className="mt-3 max-w-[18ch] text-[34px] leading-[1.05] font-semibold tracking-[-0.02em] md:text-[50px]">
          One binary. No runtime.
        </h1>
        <p className="mt-4 max-w-[62ch] text-[14.5px] leading-relaxed text-muted-foreground">
          Niki ships as a single self-contained executable. There is no interpreter to install, no package
          manager step, and no account. Pick a platform, or run the installer and let it verify the archive
          against <code className="font-mono text-foreground">SHA256SUMS</code>.
        </p>

        <div className="mt-6 flex flex-wrap items-center gap-x-3 gap-y-1 text-[12.5px] text-muted-foreground">
          <span className="font-tech text-[11px] uppercase tracking-[0.16em] text-foreground">
            Latest v{VERSION}
          </span>
          <span aria-hidden="true">·</span>
          <span>{formatDate(RELEASED)}</span>
          <span aria-hidden="true">·</span>
          <a href="/changelog" className="underline decoration-grid-strong underline-offset-2 hover:text-foreground">
            Release notes
          </a>
        </div>

        <div className="mt-8 grid gap-4 lg:grid-cols-[1.05fr_1fr]">
          <div className="min-w-0 border border-grid-strong bg-card/70 p-5">
            <p className="font-tech text-[10.5px] uppercase tracking-[0.18em] text-muted-foreground">
              Recommended for this device
            </p>
            <p className="mt-2 text-[18px] font-medium">
              {detectedArchive.os} · {detectedArchive.arch}
            </p>
            <p className="mt-1 text-[12.5px] text-muted-foreground">{detectedArchive.note}</p>

            <div className="mt-4 flex flex-wrap items-center gap-2">
              <a
                href={detectedAsset?.url ?? `${RELEASES_URL}/download/v${VERSION}/${detectedArchive.asset}`}
                className={`${BTN.base} ${BTN.primary}`}
                rel="noreferrer"
              >
                <Download className="w-4 h-4" aria-hidden="true" />
                Download archive
              </a>
              <a href="#all-platforms" className={`${BTN.base} ${BTN.secondary}`}>
                Change platform
              </a>
            </div>

            <p className="mt-4 break-all font-mono text-[11px] text-muted-foreground">
              {detectedArchive.asset}
              {detectedAsset ? ` · ${formatBytes(detectedAsset.size)}` : ''}
            </p>
            {detectedAsset?.digest && (
              <div className="mt-2 flex items-start gap-2">
                <code className="min-w-0 flex-1 break-all font-mono text-[10.5px] text-muted-foreground">
                  {detectedAsset.digest}
                </code>
                <CopyButton value={detectedAsset.digest.replace(/^sha256:/, '')} label="Hash" />
              </div>
            )}
          </div>

          <div className="min-w-0 border border-grid bg-card/45 p-5">
            <p className="font-tech text-[10.5px] uppercase tracking-[0.18em] text-muted-foreground">
              Installer script
            </p>
            <div className="mt-3 flex gap-1" role="tablist" aria-label="Installer platform">
              <button
                type="button"
                role="tab"
                aria-selected={shell === 'unix'}
                onClick={() => setShell('unix')}
                className={`h-8 px-3 rounded-[2px] font-tech text-[11.5px] border ${
                  shell === 'unix'
                    ? 'border-grid-strong bg-muted text-foreground'
                    : 'border-grid bg-transparent text-muted-foreground hover:text-foreground'
                }`}
              >
                Linux &amp; macOS
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={shell === 'windows'}
                onClick={() => setShell('windows')}
                className={`h-8 px-3 rounded-[2px] font-tech text-[11.5px] border ${
                  shell === 'windows'
                    ? 'border-grid-strong bg-muted text-foreground'
                    : 'border-grid bg-transparent text-muted-foreground hover:text-foreground'
                }`}
              >
                Windows
              </button>
            </div>

            <div className="mt-3 space-y-2">
              <CodeLine command={shell === 'windows' ? WINDOWS_INSTALL_COMMAND : INSTALL_COMMAND} />
              <p className="text-[11.5px] leading-relaxed text-muted-foreground">
                The installer downloads the archive for this platform, verifies it against{' '}
                <code className="font-mono text-foreground">SHA256SUMS</code>, and places{' '}
                <code className="font-mono text-foreground">niki</code> under{' '}
                <code className="font-mono text-foreground">~/.local/niki</code>. Pass{' '}
                <code className="font-mono text-foreground">NIKI_VERSION={VERSION}</code> before the command
                to pin this release.
              </p>
              <div className="flex flex-wrap gap-2 pt-1">
                <a
                  href={`${GITHUB_URL}/blob/niki-agent/install/install.sh`}
                  className="font-tech text-[11px] text-muted-foreground underline decoration-grid-strong underline-offset-2 hover:text-foreground"
                  rel="noreferrer"
                >
                  Read the shell installer
                </a>
                <a
                  href={`${GITHUB_URL}/blob/niki-agent/install/install.ps1`}
                  className="font-tech text-[11px] text-muted-foreground underline decoration-grid-strong underline-offset-2 hover:text-foreground"
                  rel="noreferrer"
                >
                  Read the PowerShell installer
                </a>
              </div>
            </div>
          </div>
        </div>

        <section id="all-platforms" className="mt-12 scroll-mt-20">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h2 className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">
              All platforms
            </h2>
            <a href={RELEASES_URL} className="font-tech text-[11px] text-muted-foreground hover:text-foreground" rel="noreferrer">
              Every release and checksum ↗
            </a>
          </div>
          <div className="mt-4 grid gap-3 sm:grid-cols-2">
            {ARCHIVES.map((archive) => (
              <ArchiveCard
                key={archive.id}
                archive={archive}
                asset={assets[archive.asset]}
                highlighted={archive.id === platform}
              />
            ))}
          </div>
        </section>

        <section className="mt-12">
          <h2 className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">Verify</h2>
          <p className="mt-3 max-w-[62ch] text-[13.5px] leading-relaxed text-muted-foreground">
            Download <code className="font-mono text-foreground">SHA256SUMS</code> from the same release,
            then check the archive before unpacking it. A match proves the bytes are the ones the release
            published; it does not prove who built them.
          </p>
          <div className="mt-4 grid gap-3 md:grid-cols-3">
            <div className="min-w-0 border border-grid bg-card/45 p-4">
              <p className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">Linux</p>
              <div className="mt-2 space-y-2">
                <CodeLine command={`curl -sSLO ${RELEASES_URL}/download/v${VERSION}/SHA256SUMS`} />
                <CodeLine command={`sha256sum -c SHA256SUMS --ignore-missing`} />
              </div>
            </div>
            <div className="min-w-0 border border-grid bg-card/45 p-4">
              <p className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">macOS</p>
              <div className="mt-2 space-y-2">
                <CodeLine command={`curl -sSLO ${RELEASES_URL}/download/v${VERSION}/SHA256SUMS`} />
                <CodeLine command={`shasum -a 256 -c SHA256SUMS --ignore-missing`} />
              </div>
            </div>
            <div className="min-w-0 border border-grid bg-card/45 p-4">
              <p className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">Windows</p>
              <div className="mt-2 space-y-2">
                <CodeLine
                  command={`(Get-FileHash -Algorithm SHA256 niki-${VERSION}-windows-x86_64.zip).Hash`}
                />
              </div>
            </div>
          </div>
        </section>

        <section className="mt-12 grid gap-4 md:grid-cols-2">
          <div className="min-w-0 border border-grid bg-card/45 p-5">
            <h2 className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">
              Requirements
            </h2>
            <ul className="mt-3 space-y-1.5 text-[13px] leading-relaxed text-foreground/90">
              <li>Linux x86_64 with glibc 2.31 or newer, including WSL.</li>
              <li>macOS 11 or newer, Apple silicon or Intel.</li>
              <li>Windows 10 or newer, x86_64.</li>
              <li>One API key for a provider, or a local endpoint such as Ollama.</li>
              <li>No runtime, no package manager, no account.</li>
            </ul>
          </div>
          <div className="min-w-0 border border-grid bg-card/45 p-5">
            <h2 className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">
              First run
            </h2>
            <div className="mt-3 space-y-2">
              <CodeLine command="export ANTHROPIC_API_KEY=sk-ant-..." />
              <CodeLine command="niki" />
            </div>
            <p className="mt-3 text-[12.5px] leading-relaxed text-muted-foreground">
              <code className="font-mono text-foreground">niki doctor</code> checks the config, the model, the
              credential, sessions, skills, MCP servers and hooks, and names the variable to set when a turn
              cannot run. The{' '}
              <a
                href={`${DOCS_URL}/getting-started/first-turn`}
                className="underline decoration-grid-strong underline-offset-2 hover:text-foreground"
              >
                first-turn guide
              </a>{' '}
              covers the rest.
            </p>
          </div>
        </section>

        <section className="mt-12">
          <button
            type="button"
            onClick={() => setShowAllMethods((value) => !value)}
            aria-expanded={showAllMethods}
            className="flex w-full items-center justify-between border border-grid bg-card/45 px-4 py-3 text-left"
          >
            <span className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">
              Upgrade, uninstall, and other installs
            </span>
            <ChevronDown
              className={`h-4 w-4 text-muted-foreground transition-transform ${showAllMethods ? 'rotate-180' : ''}`}
              aria-hidden="true"
            />
          </button>
          {showAllMethods && (
            <div className="grid gap-4 border border-t-0 border-grid bg-card/30 p-5 md:grid-cols-2">
              <div>
                <h3 className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">Upgrade</h3>
                <div className="mt-2">
                  <CodeLine command={INSTALL_COMMAND} />
                </div>
                <p className="mt-2 text-[12px] text-muted-foreground">
                  Re-running the installer replaces the binary in place and keeps{' '}
                  <code className="font-mono text-foreground">~/.niki</code>.
                </p>
              </div>
              <div>
                <h3 className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">Uninstall</h3>
                <div className="mt-2">
                  <CodeLine command="rm -rf ~/.local/niki" />
                </div>
                <p className="mt-2 text-[12px] text-muted-foreground">
                  Configuration and sessions live in <code className="font-mono text-foreground">~/.niki</code>{' '}
                  and are separate; remove that directory only when you want them gone.
                </p>
              </div>
              <div>
                <h3 className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">
                  From source
                </h3>
                <div className="mt-2">
                  <CodeLine command="cargo install --locked --path rust/crates/niki" />
                </div>
                <p className="mt-2 text-[12px] text-muted-foreground">
                  Requires Rust 1.85 or newer. The workspace README covers the other crates.
                </p>
              </div>
              <div>
                <h3 className="font-tech text-[11px] uppercase tracking-[0.14em] text-foreground">
                  Troubleshooting
                </h3>
                <ul className="mt-2 space-y-1 text-[12px] text-muted-foreground">
                  <li>
                    <span className="text-foreground">command not found</span> — add{' '}
                    <code className="font-mono">~/.local/niki/bin</code> to <code className="font-mono">PATH</code>.
                  </li>
                  <li>
                    <span className="text-foreground">macOS blocks the binary</span> — a browser download is
                    quarantined; use the installer, or allow it in System Settings.
                  </li>
                  <li>
                    <span className="text-foreground">PowerShell refuses the script</span> — run{' '}
                    <code className="font-mono">Set-ExecutionPolicy -Scope Process Bypass</code> first.
                  </li>
                </ul>
              </div>
            </div>
          )}
        </section>

        <div className="mt-12 flex flex-wrap items-center gap-3 border-t border-grid pt-6">
          <a href="/changelog" className={`${BTN.base} ${BTN.secondary}`}>
            <Terminal className="w-4 h-4" aria-hidden="true" />
            Changelog
          </a>
          <a href={`${DOCS_URL}/getting-started/install`} className={`${BTN.base} ${BTN.ghost}`}>
            Install guide
            <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
          </a>
          <a href={`${DOCS_URL}/beta`} className={`${BTN.base} ${BTN.ghost}`}>
            Beta builds
            <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
          </a>
        </div>
      </div>
    </PageShell>
  );
}
