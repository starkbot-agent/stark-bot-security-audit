import { useEffect, useRef } from 'react';
import clsx from 'clsx';
import type { SlashCommand } from '@/types';

interface CommandAutocompleteProps {
  commands: SlashCommand[];
  filter: string;
  selectedIndex: number;
  onSelect: (command: SlashCommand) => void;
  onClose: () => void;
}

export default function CommandAutocomplete({
  commands,
  filter,
  selectedIndex,
  onSelect,
  onClose,
}: CommandAutocompleteProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const selectedRef = useRef<HTMLButtonElement>(null);

  // Filter commands by prefix (excluding the leading /)
  const prefix = filter.startsWith('/') ? filter.slice(1).toLowerCase() : filter.toLowerCase();
  const filteredCommands = commands.filter((cmd) =>
    cmd.name.toLowerCase().startsWith(prefix)
  );

  // Scroll selected item into view
  useEffect(() => {
    if (selectedRef.current && containerRef.current) {
      selectedRef.current.scrollIntoView({
        block: 'nearest',
        behavior: 'smooth',
      });
    }
  }, [selectedIndex]);

  // Handle click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  if (filteredCommands.length === 0) {
    return null;
  }

  return (
    <div
      ref={containerRef}
      className="absolute bottom-full left-0 right-0 mb-2 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden max-h-64 overflow-y-auto"
    >
      {filteredCommands.map((command, index) => (
        <button
          key={command.name}
          ref={index === selectedIndex ? selectedRef : undefined}
          onClick={() => onSelect(command)}
          className={clsx(
            'w-full px-4 py-3 text-left flex items-center gap-3 transition-colors',
            index === selectedIndex
              ? 'bg-stark-500/20 text-stark-400'
              : 'text-slate-300 hover:bg-slate-700/50'
          )}
        >
          <span className="font-mono text-stark-400">/{command.name}</span>
          <span className="text-sm text-slate-500">{command.description}</span>
        </button>
      ))}
    </div>
  );
}
