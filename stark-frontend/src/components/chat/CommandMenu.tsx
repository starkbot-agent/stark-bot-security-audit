import { useState, useRef, useEffect } from 'react';
import { Menu } from 'lucide-react';
import clsx from 'clsx';
import {
  Command,
  CommandDefinition,
  getAllCommands,
  CATEGORY_LABELS,
  CATEGORY_ORDER,
} from '@/lib/commands';

interface CommandMenuProps {
  onCommandSelect: (command: Command) => void;
  className?: string;
}

export default function CommandMenu({ onCommandSelect, className }: CommandMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  // Group commands by category
  const commandsByCategory = CATEGORY_ORDER.reduce((acc, category) => {
    const commands = getAllCommands().filter((cmd) => cmd.category === category);
    if (commands.length > 0) {
      acc[category] = commands;
    }
    return acc;
  }, {} as Record<string, CommandDefinition[]>);

  // Close menu on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        menuRef.current &&
        buttonRef.current &&
        !menuRef.current.contains(e.target as Node) &&
        !buttonRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [isOpen]);

  // Close menu on escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('keydown', handleKeyDown);
      return () => document.removeEventListener('keydown', handleKeyDown);
    }
  }, [isOpen]);

  const handleCommandClick = (command: Command) => {
    onCommandSelect(command);
    setIsOpen(false);
  };

  return (
    <div className={clsx('relative', className)}>
      {/* Trigger Button */}
      <button
        ref={buttonRef}
        onClick={() => setIsOpen(!isOpen)}
        className={clsx(
          'flex items-center justify-center w-12 h-12 rounded-lg transition-all',
          'bg-slate-700 hover:bg-slate-600 text-slate-300 hover:text-white',
          'focus:outline-none focus:ring-2 focus:ring-stark-500 focus:ring-offset-2 focus:ring-offset-slate-900',
          isOpen && 'bg-slate-600 text-white ring-2 ring-stark-500'
        )}
        title="Commands menu"
        aria-expanded={isOpen}
        aria-haspopup="menu"
      >
        <Menu className="w-5 h-5" />
      </button>

      {/* Dropdown Menu */}
      {isOpen && (
        <div
          ref={menuRef}
          className="fixed sm:absolute bottom-20 sm:bottom-full right-4 sm:right-0 left-4 sm:left-auto sm:mb-2 w-auto sm:w-64 max-h-[70vh] sm:max-h-none bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden z-50"
          role="menu"
        >
          <div className="max-h-[calc(70vh-3rem)] sm:max-h-80 overflow-y-auto">
            {Object.entries(commandsByCategory).map(([category, commands], categoryIndex) => (
              <div key={category}>
                {/* Category Header */}
                <div className="px-3 py-2 bg-slate-900/50 border-b border-slate-700">
                  <span className="text-xs font-semibold text-slate-500 uppercase tracking-wider">
                    {CATEGORY_LABELS[category as keyof typeof CATEGORY_LABELS]}
                  </span>
                </div>

                {/* Commands */}
                {commands.map((cmd) => {
                  const Icon = cmd.icon;
                  return (
                    <button
                      key={cmd.command}
                      onClick={() => handleCommandClick(cmd.command)}
                      className="w-full px-3 py-2.5 flex items-center gap-3 text-left transition-colors hover:bg-slate-700/50 group"
                      role="menuitem"
                    >
                      <Icon className="w-4 h-4 text-slate-500 group-hover:text-stark-400 transition-colors" />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-sm text-stark-400">/{cmd.name}</span>
                        </div>
                        <span className="text-xs text-slate-500 group-hover:text-slate-400 truncate block">
                          {cmd.description}
                        </span>
                      </div>
                    </button>
                  );
                })}

                {/* Divider between categories (except last) */}
                {categoryIndex < Object.keys(commandsByCategory).length - 1 && (
                  <div className="border-b border-slate-700/50" />
                )}
              </div>
            ))}
          </div>

          {/* Footer hint */}
          <div className="px-3 py-2 bg-slate-900/30 border-t border-slate-700">
            <span className="text-xs text-slate-600">
              Tip: Type <code className="bg-slate-700 px-1 rounded text-slate-400">/</code> in chat for autocomplete
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
