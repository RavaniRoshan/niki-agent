import React, { useCallback, useEffect, useState } from 'react';
import { ArrowUpRight, BookOpen, Download, ScrollText } from 'lucide-react';
import { BTN, NikiMark } from '../../../shared';
import { ThemeToggleButton } from '../../../features/theme-switch';

const DOCS_URL = 'https://niki-docs.vercel.app';
const GITHUB_URL = 'https://github.com/RavaniRoshan/niki-agent';

interface PageShellProps {
  children: React.ReactNode;
  title: string;
  description: string;
}

export function PageShell({ children, title, description }: PageShellProps) {
  const [colorMode, setColorMode] = useState<'dark' | 'light'>(() => {
    if (typeof window === 'undefined') return 'dark';
    const saved = window.localStorage.getItem('niki-color-mode');
    return saved === 'light' ? 'light' : 'dark';
  });

  useEffect(() => {
    document.documentElement.classList.toggle('dark', colorMode === 'dark');
    try {
      window.localStorage.setItem('niki-color-mode', colorMode);
    } catch {
      // Ignore storage failures in restricted contexts.
    }
  }, [colorMode]);

  useEffect(() => {
    document.title = `${title} — Niki Agent`;
    const meta = document.querySelector('meta[name="description"]');
    if (meta) meta.setAttribute('content', description);
  }, [title, description]);

  const toggleColorMode = useCallback(() => {
    setColorMode((prev) => (prev === 'dark' ? 'light' : 'dark'));
  }, []);

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col">
      <header className="sticky top-0 z-40 border-b border-grid bg-background/85 backdrop-blur-md">
        <div className="mx-auto flex h-14 w-full max-w-[1100px] items-center gap-3 px-5 md:px-8">
          <a href="/" className="flex items-center gap-2 text-foreground hover:opacity-80">
            <NikiMark className="w-5 h-5" />
            <span className="font-tech text-[13px] font-medium tracking-[0.08em]">NIKI AGENT</span>
          </a>

          <nav className="ml-auto hidden items-center gap-1 md:flex" aria-label="Site">
            <a href="/" className={`${BTN.base} ${BTN.ghost}`}>
              Product
            </a>
            <a href="/download" className={`${BTN.base} ${BTN.ghost}`}>
              <Download className="w-3.5 h-3.5" aria-hidden="true" />
              Download
            </a>
            <a href="/changelog" className={`${BTN.base} ${BTN.ghost}`}>
              <ScrollText className="w-3.5 h-3.5" aria-hidden="true" />
              Changelog
            </a>
            <a href={DOCS_URL} className={`${BTN.base} ${BTN.ghost}`}>
              <BookOpen className="w-3.5 h-3.5" aria-hidden="true" />
              Docs
            </a>
            <a href={GITHUB_URL} className={`${BTN.base} ${BTN.ghost}`} rel="noreferrer">
              GitHub
              <ArrowUpRight className="w-3.5 h-3.5" aria-hidden="true" />
            </a>
          </nav>

          <div className="ml-auto flex items-center gap-2 md:ml-2">
            <ThemeToggleButton colorMode={colorMode} onToggle={toggleColorMode} compact />
            <a href={GITHUB_URL} className={`${BTN.base} ${BTN.secondary} md:hidden`} rel="noreferrer">
              GitHub
            </a>
          </div>
        </div>
      </header>

      <main className="flex-1">{children}</main>

      <footer className="border-t border-grid">
        <div className="mx-auto flex w-full max-w-[1100px] flex-col gap-3 px-5 py-6 md:flex-row md:items-center md:justify-between md:px-8">
          <p className="font-tech text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
            Niki Agent · one binary, no runtime
          </p>
          <nav className="flex flex-wrap items-center gap-4 text-[12px] text-muted-foreground" aria-label="Footer">
            <a href="/download" className="hover:text-foreground">
              Download
            </a>
            <a href="/changelog" className="hover:text-foreground">
              Changelog
            </a>
            <a href={`${DOCS_URL}/beta`} className="hover:text-foreground">
              Beta builds
            </a>
            <a href={`${DOCS_URL}/security`} className="hover:text-foreground">
              Security
            </a>
            <a href={GITHUB_URL} className="hover:text-foreground" rel="noreferrer">
              GitHub
            </a>
          </nav>
        </div>
      </footer>
    </div>
  );
}
