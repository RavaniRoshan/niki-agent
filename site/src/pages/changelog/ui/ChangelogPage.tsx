import React, { useMemo, useState } from 'react';
import { ArrowUpRight, Download, Rss } from 'lucide-react';
import { BTN, CARD } from '../../../shared';
import { PageShell } from '../../../widgets/page-shell';
import { CHANGELOG, ChangelogRelease } from '../changelog.generated';

const GITHUB_URL = 'https://github.com/RavaniRoshan/niki-agent';

export function versionAnchor(version: string): string {
  return `v${version.replace(/[^a-zA-Z0-9]+/g, '-').replace(/^-|-$/g, '').toLowerCase()}`;
}

/** Render the inline Markdown the changelog uses: `code`, **bold**, and links. */
function RichText({ text }: { text: string }) {
  const nodes: React.ReactNode[] = [];
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\))/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let key = 0;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) nodes.push(text.slice(lastIndex, match.index));
    const token = match[0];
    if (token.startsWith('`')) {
      nodes.push(
        <code
          key={key++}
          className="rounded-[2px] border border-grid bg-muted/70 px-1 py-0.5 font-mono text-[0.92em]"
        >
          {token.slice(1, -1)}
        </code>
      );
    } else if (token.startsWith('**')) {
      nodes.push(
        <strong key={key++} className="font-semibold text-foreground">
          {token.slice(2, -2)}
        </strong>
      );
    } else {
      const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      if (link) {
        nodes.push(
          <a
            key={key++}
            href={link[2]}
            className="text-foreground underline decoration-grid-strong underline-offset-2 hover:decoration-foreground"
          >
            {link[1]}
          </a>
        );
      } else {
        nodes.push(token);
      }
    }
    lastIndex = match.index + token.length;
  }

  if (lastIndex < text.length) nodes.push(text.slice(lastIndex));
  return <>{nodes}</>;
}

function formatDate(date: string | null): string {
  if (!date) return 'Unreleased';
  return new Date(`${date}T00:00:00Z`).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
}

function ReleaseEntry({ release }: { release: ChangelogRelease }) {
  const anchor = versionAnchor(release.version);
  const itemCount = release.sections.reduce((sum, section) => sum + section.items.length, 0);

  return (
    <article id={anchor} className={`scroll-mt-20 border ${CARD.cell} p-5 md:p-6`}>
      <header className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <h2 className="font-tech text-[15px] font-medium tracking-[0.02em] text-foreground">
          {release.unreleased ? 'Unreleased' : `v${release.version}`}
        </h2>
        <time className="text-[12px] text-muted-foreground" dateTime={release.date ?? undefined}>
          {formatDate(release.date)}
        </time>
        <a
          href={`${GITHUB_URL}/releases/tag/v${release.version}`}
          className="ml-auto text-[11px] text-muted-foreground hover:text-foreground"
          rel="noreferrer"
        >
          GitHub release ↗
        </a>
      </header>

      {itemCount === 0 ? (
        <p className="mt-3 text-[13px] text-muted-foreground">No changes recorded yet.</p>
      ) : (
        release.sections.map((section) => (
          <section key={section.title} className="mt-4">
            <h3 className="font-tech text-[10.5px] uppercase tracking-[0.18em] text-muted-foreground">
              {section.title}
            </h3>
            <ul className="mt-2 space-y-1.5">
              {section.items.map((item, index) => (
                <li key={index} className="flex gap-2 text-[13.5px] leading-relaxed text-foreground/90">
                  <span aria-hidden="true" className="mt-[9px] h-1 w-1 shrink-0 rounded-full bg-chart-2" />
                  <span>
                    <RichText text={item} />
                  </span>
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
    </article>
  );
}

export function ChangelogPage() {
  const [showAll, setShowAll] = useState(false);

  const releases = useMemo(() => CHANGELOG.filter((release) => !release.unreleased), []);
  const unreleased = useMemo(() => CHANGELOG.find((release) => release.unreleased), []);
  const latest = releases[0];
  const visible = showAll ? releases : releases.slice(0, 5);

  return (
    <PageShell
      title="Changelog"
      description="Every Niki Agent release, newest first: what shipped, when, and which archive carries it."
    >
      <div className="mx-auto w-full max-w-[900px] px-5 py-12 md:px-8 md:py-16">
        <p className="font-tech text-[11px] uppercase tracking-[0.2em] text-muted-foreground">Changelog</p>
        <h1 className="mt-3 text-[34px] leading-[1.1] font-semibold tracking-[-0.02em] md:text-[46px]">
          Every release, in order.
        </h1>
        <p className="mt-4 max-w-[62ch] text-[14.5px] leading-relaxed text-muted-foreground">
          Generated from <code className="font-mono text-foreground">CHANGELOG.md</code> in the repository, so
          the page and the archive carry the same history. Newest first.
        </p>

        <div className="mt-6 flex flex-wrap items-center gap-2">
          <a href="/download" className={`${BTN.base} ${BTN.primary}`}>
            <Download className="w-4 h-4" aria-hidden="true" />
            Download {latest ? `v${latest.version}` : 'the latest release'}
          </a>
          <a href={`${GITHUB_URL}/releases`} className={`${BTN.base} ${BTN.secondary}`} rel="noreferrer">
            All releases on GitHub
            <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
          </a>
          <a href={`${GITHUB_URL}/releases.atom`} className={`${BTN.base} ${BTN.ghost}`} rel="noreferrer">
            <Rss className="w-3.5 h-3.5" aria-hidden="true" />
            RSS
          </a>
        </div>

        {latest && (
          <div className="mt-8 border border-grid-strong bg-card/70 p-5 md:p-6">
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
              <span className="font-tech text-[10.5px] uppercase tracking-[0.18em] text-chart-2">Latest</span>
              <span className="font-tech text-[15px] font-medium">v{latest.version}</span>
              <span className="text-[12px] text-muted-foreground">{formatDate(latest.date)}</span>
            </div>
            <ul className="mt-3 space-y-1.5">
              {latest.sections
                .flatMap((section) => section.items)
                .slice(0, 5)
                .map((item, index) => (
                  <li key={index} className="flex gap-2 text-[13.5px] leading-relaxed text-foreground/90">
                    <span aria-hidden="true" className="mt-[9px] h-1 w-1 shrink-0 rounded-full bg-chart-2" />
                    <span>
                      <RichText text={item} />
                    </span>
                  </li>
                ))}
            </ul>
            <a
              href={`#${versionAnchor(latest.version)}`}
              className="mt-4 inline-block font-tech text-[11.5px] text-muted-foreground underline decoration-grid-strong underline-offset-2 hover:text-foreground"
            >
              Read the full entry
            </a>
          </div>
        )}

        <p className="mt-8 text-[12.5px] text-muted-foreground">
          Check the build you run with <code className="font-mono text-foreground">niki --version</code>.
        </p>

        <div className="mt-6 space-y-4">
          {visible.map((release) => (
            <ReleaseEntry key={release.version} release={release} />
          ))}
          {unreleased && unreleased.sections.length > 0 && <ReleaseEntry release={unreleased} />}
        </div>

        {!showAll && releases.length > visible.length && (
          <button type="button" onClick={() => setShowAll(true)} className={`${BTN.base} ${BTN.secondary} mt-6`}>
            Show all {releases.length} releases
          </button>
        )}
      </div>
    </PageShell>
  );
}
