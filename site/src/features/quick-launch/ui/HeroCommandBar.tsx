import React, { useEffect, useRef, useState } from 'react';
import { Check, Copy, Terminal } from 'lucide-react';
import { HERO_QUICK_PRESETS } from '../../../entities/niki';
import { BTN, buildProviderInstallCommand, ProviderIcon } from '../../../shared';

interface HeroCommandBarProps {
  onLaunchInstall: (command: string) => void;
}

/**
 * Unified centered architectural command bar wired to live install commands.
 * Renders real full-color provider logos on a high-contrast neutral preset rail.
 */
export function HeroCommandBar({ onLaunchInstall }: HeroCommandBarProps) {
  const [providerQuery, setProviderQuery] = useState('');
  const [selectedPresetId, setSelectedPresetId] = useState<string>('all');
  const [copiedNotice, setCopiedNotice] = useState(false);
  const copyTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
    };
  }, []);

  const activePreset =
    HERO_QUICK_PRESETS.find((p) => p.id === selectedPresetId) || HERO_QUICK_PRESETS[0];

  const resolvedCommand = providerQuery.trim()
    ? buildProviderInstallCommand(providerQuery)
    : activePreset.command;

  const triggerCopy = () => {
    navigator.clipboard?.writeText(resolvedCommand).catch(() => {});
    setCopiedNotice(true);
    if (copyTimerRef.current !== null) {
      window.clearTimeout(copyTimerRef.current);
    }
    copyTimerRef.current = window.setTimeout(() => {
      setCopiedNotice(false);
      copyTimerRef.current = null;
    }, 1800);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    triggerCopy();
    onLaunchInstall(resolvedCommand);
  };

  return (
    <div className="w-full max-w-[610px] mx-auto">
      <form
        onSubmit={handleSubmit}
        aria-label="Quick launch Niki Agent"
        className="flex items-center gap-1.5 rounded-[2px] border border-grid-strong bg-card/95 backdrop-blur-md p-1.5 pl-4 shadow-lg transition-colors focus-within:border-ring"
      >
        <Terminal className="w-4 h-4 text-muted-foreground shrink-0" aria-hidden="true" />
        <label htmlFor="hero-quick-launch-input" className="sr-only">
          Install command or provider
        </label>
        <input
          id="hero-quick-launch-input"
          type="text"
          value={providerQuery}
          onChange={(e) => setProviderQuery(e.target.value)}
          placeholder={activePreset.command}
          className="w-full min-w-0 bg-transparent font-tech font-normal text-[13px] sm:text-[14px] text-foreground placeholder:text-muted-foreground focus:outline-none truncate"
        />
        <button
          type="button"
          onClick={triggerCopy}
          aria-label={copiedNotice ? 'Command copied' : 'Copy launch command'}
          title={copiedNotice ? 'Copied' : 'Copy command'}
          className={`${BTN.base} ${BTN.secondary} px-2.5 h-9 font-tech text-[12px]`}
        >
          {copiedNotice ? (
            <>
              <Check className="w-3.5 h-3.5 text-chart-1 dark:text-chart-2" aria-hidden="true" />
              <span className="hidden sm:inline">Copied</span>
            </>
          ) : (
            <>
              <Copy className="w-3.5 h-3.5" aria-hidden="true" />
              <span className="hidden sm:inline">Copy</span>
            </>
          )}
        </button>
        <button type="submit" className={`${BTN.base} ${BTN.primary}`}>
          <span>Install Niki</span>
        </button>
      </form>

      {/* Hairline-framed preset selector row with real full-color provider logos */}
      <div className="mt-4 flex justify-center">
        <div
          role="group"
          aria-label="Quick provider presets"
          className="inline-flex flex-wrap items-center justify-center rounded-[2px] bg-card/95 backdrop-blur-md border border-grid-strong divide-x divide-grid shadow-md font-tech text-[12px]"
        >
          {HERO_QUICK_PRESETS.map((preset) => {
            const isSelected = preset.id === selectedPresetId && !providerQuery.trim();
            const iconId = preset.id === 'install' ? 'niki' : preset.id;
            return (
              <button
                key={preset.id}
                type="button"
                aria-pressed={isSelected}
                onClick={() => {
                  setProviderQuery('');
                  setSelectedPresetId(preset.id);
                }}
                className={`relative inline-flex items-center gap-1.5 px-3.5 py-2 transition-colors duration-150 cursor-pointer whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring ${
                  isSelected
                    ? 'bg-muted text-foreground font-semibold'
                    : 'text-foreground/85 hover:text-foreground hover:bg-muted/65'
                }`}
              >
                <ProviderIcon id={iconId} className="w-3.5 h-3.5" />
                <span>{preset.label}</span>
                {isSelected && (
                  <span
                    aria-hidden="true"
                    className="pointer-events-none absolute inset-x-0 bottom-0 h-[2px] bg-primary"
                  />
                )}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
