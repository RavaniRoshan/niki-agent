import React from 'react';

/**
 * Normalized Architectural UI Primitives
 *
 * Single source of truth for Niki Agent's symmetrical hairline-ruled grid frame,
 * eyebrow headers, normalized button variants, and hairline card surfaces —
 * matching the TicketStubFooter architectural grammar across the entire landing page.
 */

export const BTN = {
  base:
    'inline-flex items-center justify-center gap-2 font-sans font-medium text-[14px] leading-none whitespace-nowrap shrink-0 cursor-pointer select-none transition-[background-color,border-color,color,opacity,transform] duration-150 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background',
  primary:
    'h-10 px-5 rounded-[2px] bg-primary text-primary-foreground hover:opacity-92 shadow-xs',
  secondary:
    'h-10 px-4 rounded-[2px] border border-grid-strong bg-card/70 text-foreground hover:bg-muted hover:border-foreground/40',
  subtle:
    'h-9 px-3.5 rounded-[2px] border border-grid bg-card/50 text-foreground hover:bg-muted hover:border-grid-strong font-tech text-[12px]',
  ghost:
    'h-9 px-3 rounded-[2px] border border-transparent text-foreground/80 hover:text-foreground hover:bg-muted/70',
  icon:
    'w-9 h-9 rounded-[2px] border border-grid bg-card/70 text-muted-foreground hover:text-foreground hover:bg-muted hover:border-grid-strong',
  iconLg:
    'w-10 h-10 rounded-[2px] border border-grid bg-card/70 text-foreground hover:bg-muted hover:border-grid-strong',
} as const;

export const CARD = {
  cell: 'border-grid bg-card/45 transition-[background-color,border-color] duration-150',
  cellInteractive:
    'border-grid bg-card/45 hover:bg-muted/55 transition-[background-color,border-color] duration-150 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring',
  stageFrame:
    'dark relative bg-[#06070a] border-grid overflow-hidden select-none',
  modal:
    'relative w-full max-w-[600px] max-h-[90vh] overflow-y-auto rounded-[2px] border border-grid-strong bg-card text-card-foreground shadow-2xl animate-modal-enter',
} as const;

interface GridBandProps {
  id?: string;
  ariaLabel?: string;
  children: React.ReactNode;
  className?: string;
  innerClassName?: string;
  topBorder?: boolean;
  bottomBorder?: boolean;
  showCornerTicks?: boolean;
  dataTestId?: string;
  sectionRef?: React.Ref<HTMLElement>;
  dataAttrs?: Record<string, string | number>;
}

/**
 * Symmetrical 3-track architectural band:
 * [ left outer rail | hairline-framed main canvas | right outer rail ]
 * Keeps exact vertical rail continuity from Navbar down into TicketStubFooter.
 */
export function GridBand({
  id,
  ariaLabel,
  children,
  className = '',
  innerClassName = '',
  topBorder = true,
  bottomBorder = false,
  showCornerTicks = true,
  dataTestId,
  sectionRef,
  dataAttrs,
}: GridBandProps) {
  return (
    <section
      id={id}
      ref={sectionRef}
      aria-label={ariaLabel}
      data-testid={dataTestId}
      {...dataAttrs}
      className={`relative w-full flex flex-col bg-background ${topBorder ? 'border-t border-grid' : ''} ${
        bottomBorder ? 'border-b border-grid' : ''
      } ${className}`}
    >
      <div className="arch-grid-band flex-1">
        {/* Left symmetrical rail */}
        <div aria-hidden="true" className="relative border-r border-grid">
          {showCornerTicks && (
            <span className="pointer-events-none absolute -top-[4px] -right-[4px] w-[7px] h-[7px] rotate-45 border border-grid-strong bg-background" />
          )}
        </div>

        {/* Main symmetrical center column */}
        <div className={`min-w-0 relative ${innerClassName}`}>{children}</div>

        {/* Right symmetrical rail */}
        <div aria-hidden="true" className="relative border-l border-grid">
          {showCornerTicks && (
            <span className="pointer-events-none absolute -top-[4px] -left-[4px] w-[7px] h-[7px] rotate-45 border border-grid-strong bg-background" />
          )}
        </div>
      </div>
    </section>
  );
}

interface SectionEyebrowProps {
  index: string;
  label: string;
  rightMeta?: React.ReactNode;
  className?: string;
}

/**
 * Unified hairline-ruled eyebrow bar matching TicketStubFooter's `◇ LABEL ·······` rhythm.
 */
export function SectionEyebrow({
  index,
  label,
  rightMeta,
  className = '',
}: SectionEyebrowProps) {
  return (
    <div
      className={`flex items-center gap-3 h-9 px-4 sm:px-6 border-b border-grid font-tech font-medium text-[11.5px] tracking-[0.06em] uppercase text-foreground/85 ${className}`}
    >
      <svg
        viewBox="0 0 12 12"
        className="w-3 h-3 shrink-0 text-chart-1 dark:text-chart-2"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M6 1 11 6 6 11 1 6Z" fill="none" stroke="currentColor" strokeWidth="1.25" />
      </svg>
      <span className="text-chart-1 dark:text-chart-2 tabular-nums shrink-0">{index}</span>
      <span className="opacity-35 select-none shrink-0" aria-hidden="true">
        ·
      </span>
      <span className="whitespace-nowrap shrink-0">{label}</span>
      <span className="arch-dots text-foreground" aria-hidden="true" />
      {rightMeta && (
        <span className="text-[11px] text-muted-foreground tabular-nums whitespace-nowrap shrink-0">
          {rightMeta}
        </span>
      )}
    </div>
  );
}

export function Keycap({
  children,
  inverted = false,
  className = '',
}: {
  children: React.ReactNode;
  inverted?: boolean;
  className?: string;
}) {
  return (
    <kbd
      aria-hidden="true"
      className={`inline-flex items-center justify-center min-w-[20px] h-[20px] px-1 rounded-[2px] border font-tech font-medium text-[11px] leading-none ${
        inverted
          ? 'bg-primary-foreground/15 border-primary-foreground/30 text-primary-foreground'
          : 'bg-background border-grid-strong text-foreground'
      } ${className}`}
    >
      {children}
    </kbd>
  );
}
