import React, { useCallback, useEffect, useState } from 'react';
import { Check, Copy } from 'lucide-react';

interface CopyButtonProps {
  value: string;
  label?: string;
  className?: string;
}

export function CopyButton({ value, label = 'Copy', className = '' }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
    } catch {
      // Clipboard access can be denied; the command stays selectable.
    }
  }, [value]);

  return (
    <button
      type="button"
      onClick={copy}
      aria-label={copied ? `${label}ed` : label}
      title={copied ? 'Copied' : label}
      className={`inline-flex items-center gap-1.5 h-8 px-2.5 rounded-[2px] border border-grid bg-background/80 text-muted-foreground hover:text-foreground hover:border-grid-strong transition-colors font-tech text-[11px] ${className}`}
    >
      {copied ? <Check className="w-3.5 h-3.5" aria-hidden="true" /> : <Copy className="w-3.5 h-3.5" aria-hidden="true" />}
      <span>{copied ? 'Copied' : label}</span>
    </button>
  );
}
