import React from 'react';
import { Moon, Sun } from 'lucide-react';
import { BTN } from '../../../shared';

interface ThemeToggleButtonProps {
  colorMode: 'dark' | 'light';
  onToggle: () => void;
  compact?: boolean;
}

export function ThemeToggleButton({
  colorMode,
  onToggle,
  compact = false,
}: ThemeToggleButtonProps) {
  const label = colorMode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode';

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-label={label}
      title={label}
      className={`${BTN.base} ${compact ? BTN.icon : BTN.iconLg}`}
    >
      {colorMode === 'dark' ? (
        <Sun className="w-4 h-4" aria-hidden="true" />
      ) : (
        <Moon className="w-4 h-4" aria-hidden="true" />
      )}
    </button>
  );
}
