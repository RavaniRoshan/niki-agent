import React from 'react';

/**
 * Stageshow primitive: quiet structural filler bar for peripheral terminal context.
 * Keeps real words focused strictly on the story (3–7 focal strings per scene).
 */
export function FillerBar({
  w = 'w-24',
  h = 'h-1.5',
  tone = 'bg-[#222736]',
  style,
}: {
  w?: string;
  h?: string;
  tone?: string;
  style?: React.CSSProperties;
}) {
  return <span aria-hidden="true" style={style} className={`block rounded-full ${w} ${h} ${tone}`} />;
}

/**
 * Stageshow primitive: subtle terminal window title bar with neutral dots and path label.
 */
export function TerminalTitleBar({
  title,
  rightLabel,
  borderColor = '#212536',
  bgColor = '#0c0e14',
  textColor = '#8f98ad',
}: {
  title: string;
  rightLabel?: string;
  borderColor?: string;
  bgColor?: string;
  textColor?: string;
}) {
  return (
    <div
      style={{ borderColor, backgroundColor: bgColor, color: textColor }}
      className="flex items-center justify-between gap-4 px-4 py-2.5 border-b font-tech text-[10.5px] select-none"
    >
      <div className="flex items-center gap-3 min-w-0">
        <span className="inline-flex items-center gap-1.5 shrink-0" aria-hidden="true">
          <span className="w-2 h-2 rounded-full bg-[#2e354d]" />
          <span className="w-2 h-2 rounded-full bg-[#2e354d]" />
          <span className="w-2 h-2 rounded-full bg-[#2e354d]" />
        </span>
        <span className="truncate">{title}</span>
      </div>
      {rightLabel && <span className="shrink-0 opacity-80 pr-8">{rightLabel}</span>}
    </div>
  );
}
