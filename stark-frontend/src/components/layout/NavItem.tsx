import { Link, useLocation } from 'react-router-dom';
import { LucideIcon } from 'lucide-react';
import clsx from 'clsx';

interface NavItemProps {
  to: string;
  icon: LucideIcon;
  label: string;
}

export default function NavItem({ to, icon: Icon, label }: NavItemProps) {
  const location = useLocation();
  const isActive = location.pathname === to;

  return (
    <Link
      to={to}
      className={clsx(
        'flex items-center gap-3 px-4 py-3 rounded-lg font-medium transition-colors',
        isActive
          ? 'bg-stark-500/20 text-stark-400'
          : 'text-slate-400 hover:text-white hover:bg-slate-700/50'
      )}
    >
      <Icon className="w-5 h-5" />
      <span>{label}</span>
    </Link>
  );
}
