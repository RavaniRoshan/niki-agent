import React from 'react';
import TicketStubFooter, {
  type StubLink,
  type StubLinkGroup,
} from '@/components/ui/ticket-stub-footer';
import { FOOTER_COLUMNS, INSTALL_COMMAND } from '../../../entities/niki';

interface BottomCtaAndFooterProps {
  isDark: boolean;
  onInstall: () => void;
  onOpenGithub: () => void;
  onSelectDetail: (detailId: string) => void;
}

const FOOTER_SECTIONS: StubLink[] = [
  { label: 'Overview', href: '#hero' },
  { label: 'Core Surface', href: '#bento-features' },
  { label: 'Parallax Craft', href: '#parallax-story' },
  { label: 'Orbital Dial', href: '#lifecycle' },
  { label: 'Benchmarks', href: '#benchmarks' },
  { label: 'Pillars', href: '#pillars' },
  { label: 'Proof Stats', href: '#stats' },
  { label: 'Get Started', href: '#get-started' },
];

const FOOTER_LINK_GROUPS: StubLinkGroup[] = [
  {
    title: 'Docs',
    links: FOOTER_COLUMNS.docs.slice(0, 6).map((item) => ({
      label: item.label,
      href: item.href,
    })),
  },
  {
    title: 'Ecosystem',
    links: FOOTER_COLUMNS.ecosystem.map((item) => ({
      label: item.label,
      href: item.href,
    })),
  },
];

const FOOTER_EYEBROW = [
  INSTALL_COMMAND,
  'one self-contained Rust binary · four contrast-tested themes',
  '11 providers built in · any OpenAI-compatible gateway in one TOML block',
];

const FOOTER_LEGAL: StubLink[] = [
  { label: 'MIT Licensed' },
  { label: 'Download', href: '/download' },
  { label: 'Documentation', href: 'https://niki-docs.vercel.app' },
  { label: 'Beta builds', href: 'https://niki-docs.vercel.app/beta' },
  { label: 'Changelog', href: '/changelog' },
  { label: 'GitHub', href: 'https://github.com/RavaniRoshan/niki-agent' },
  {
    label: 'NOTICE',
    href: 'https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE',
  },
];

export function BottomCtaAndFooter({
  isDark,
  onInstall,
}: BottomCtaAndFooterProps) {
  return (
    <div className="w-full border-t border-grid bg-background text-foreground transition-colors duration-200">
      <TicketStubFooter
        id="get-started"
        brand="Niki Agent"
        wordmark="Niki Agent"
        company="Niki Agent"
        year={2026}
        sections={FOOTER_SECTIONS}
        defaultSection={7}
        eyebrow={FOOTER_EYEBROW}
        headline={['One binary, no runtime.', 'One command to start.']}
        description="A quiet terminal surface in front of an agent that reads your repository, edits files, and runs commands, with an approval gate in front of every change."
        cta={{ label: 'Install Niki' }}
        onCtaClick={onInstall}
        linkGroups={FOOTER_LINK_GROUPS}
        statusLabels={['Working', 'Idle']}
        statusCaption="Agent Loop"
        defaultActive={true}
        count={11}
        countLabel="Providers Built In"
        rate={0}
        blurb="One installer line, one command. Eleven providers built in, any OpenAI-compatible endpoint in ~/.niki/config.toml, and sessions persisted in SQLite under ~/.niki."
        legal={FOOTER_LEGAL}
        background={isDark ? 'oklch(0 0 0)' : 'oklch(0.99 0.002 281.25)'}
        ink={isDark ? 'oklch(0.974 0.004 281.223)' : 'oklch(0.12 0.031 273.513)'}
        muted={isDark ? 'oklch(0.789 0.011 236.635)' : 'oklch(0.39 0.013 236.931)'}
        accent="oklch(0.473 0.283 265.93)"
        accentDeep="oklch(0.38 0.25 265.93)"
        accentInk="#ffffff"
        fontSans='"Geist", ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
        fontMono='"Geist Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace'
        serifWeight={500}
        halftone={true}
      />
    </div>
  );
}
