import {
  Home,
  MessageSquare,
  Monitor,
  Settings,
  Wrench,
  Zap,
  Calendar,
  FileText,
  Users,
  ScrollText,
  Bug,
  LogOut,
} from 'lucide-react';
import NavItem from './NavItem';
import { useAuth } from '@/hooks/useAuth';

export default function Sidebar() {
  const { logout } = useAuth();

  return (
    <aside className="w-64 h-screen sticky top-0 bg-slate-800 flex flex-col border-r border-slate-700">
      {/* Header */}
      <div className="p-6 border-b border-slate-700">
        <h1 className="text-2xl font-bold text-stark-400">StarkBot</h1>
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-4 space-y-1 overflow-y-auto">
        {/* Main Section */}
        <div className="space-y-1">
          <NavItem to="/dashboard" icon={Home} label="Dashboard" />
          <NavItem to="/agent-chat" icon={MessageSquare} label="Agent Chat" />
          <NavItem to="/channels" icon={Monitor} label="Channels" />
          <NavItem to="/agent-settings" icon={Settings} label="Agent Settings" />
          <NavItem to="/tools" icon={Wrench} label="Tools" />
          <NavItem to="/skills" icon={Zap} label="Skills" />
        </div>

        {/* Data Section */}
        <div className="pt-4 mt-4 border-t border-slate-700 space-y-1">
          <p className="px-4 py-2 text-xs font-semibold text-slate-500 uppercase tracking-wider">
            Data
          </p>
          <NavItem to="/sessions" icon={Calendar} label="Sessions" />
          <NavItem to="/memories" icon={FileText} label="Memories" />
          <NavItem to="/identities" icon={Users} label="Identities" />
        </div>

        {/* Developer Section */}
        <div className="pt-4 mt-4 border-t border-slate-700 space-y-1">
          <p className="px-4 py-2 text-xs font-semibold text-slate-500 uppercase tracking-wider">
            Developer
          </p>
          <NavItem to="/logs" icon={ScrollText} label="Logs" />
          <NavItem to="/debug" icon={Bug} label="Debug" />
        </div>
      </nav>

      {/* Footer */}
      <div className="p-4 border-t border-slate-700">
        <button
          onClick={logout}
          className="w-full flex items-center gap-3 px-4 py-3 rounded-lg font-medium text-slate-400 hover:text-white hover:bg-slate-700/50 transition-colors"
        >
          <LogOut className="w-5 h-5" />
          <span>Logout</span>
        </button>
      </div>
    </aside>
  );
}
