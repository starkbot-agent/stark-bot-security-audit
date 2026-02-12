import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Package,
  Check,
  X,
  Trash2,
  Play,
  Pause,
  Key,
  Database,
  Wrench,
  Activity,
  RefreshCw,
  LayoutDashboard,
} from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import { apiFetch } from '@/lib/api';

interface ModuleInfo {
  name: string;
  description: string;
  version: string;
  installed: boolean;
  enabled: boolean;
  has_db_tables: boolean;
  has_tools: boolean;
  has_worker: boolean;
  has_dashboard: boolean;
  required_api_keys: string[];
  api_keys_met: boolean;
  installed_at: string | null;
}

export default function Modules() {
  const navigate = useNavigate();
  const [modules, setModules] = useState<ModuleInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [reloadLoading, setReloadLoading] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  useEffect(() => {
    loadModules();
  }, []);

  const loadModules = async () => {
    try {
      const data = await apiFetch<ModuleInfo[]>('/modules');
      setModules(data);
    } catch (err) {
      setMessage({ type: 'error', text: 'Failed to load modules' });
    } finally {
      setIsLoading(false);
    }
  };

  const performAction = async (name: string, action: string) => {
    setActionLoading(`${name}-${action}`);
    setMessage(null);
    try {
      const result = await apiFetch<{ status: string; message: string; error?: string }>(
        `/modules/${encodeURIComponent(name)}`,
        { method: 'POST', body: JSON.stringify({ action }) }
      );
      setMessage({ type: 'success', text: result.message || `Module ${action}ed successfully` });
      await loadModules();
    } catch (err: any) {
      let errorMsg = err.message || `Failed to ${action} module`;
      try {
        const parsed = JSON.parse(errorMsg);
        errorMsg = parsed.error || errorMsg;
      } catch {}
      setMessage({ type: 'error', text: errorMsg });
    } finally {
      setActionLoading(null);
    }
  };

  const reloadModules = async () => {
    setReloadLoading(true);
    setMessage(null);
    try {
      const result = await apiFetch<{ status: string; message: string; activated: string[] }>(
        '/modules/reload',
        { method: 'POST' }
      );
      setMessage({ type: 'success', text: result.message || 'Modules reloaded' });
      await loadModules();
    } catch (err: any) {
      let errorMsg = err.message || 'Failed to reload modules';
      try {
        const parsed = JSON.parse(errorMsg);
        errorMsg = parsed.error || errorMsg;
      } catch {}
      setMessage({ type: 'error', text: errorMsg });
    } finally {
      setReloadLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center min-h-[400px]">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading modules...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8">
      {/* Header */}
      <div className="mb-8 flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white mb-2">Modules</h1>
          <p className="text-slate-400">
            Enable and manage optional modules. Modules add features like wallet monitoring, copy trading, and more.
          </p>
        </div>
        <Button
          size="sm"
          variant="secondary"
          disabled={reloadLoading || actionLoading !== null}
          onClick={reloadModules}
        >
          <RefreshCw className={`w-4 h-4 mr-1.5 ${reloadLoading ? 'animate-spin' : ''}`} />
          Reload Modules
        </Button>
      </div>

      {/* Messages */}
      {message && (
        <div
          className={`mb-6 px-4 py-3 rounded-lg ${
            message.type === 'success'
              ? 'bg-green-500/20 border border-green-500/50 text-green-400'
              : 'bg-red-500/20 border border-red-500/50 text-red-400'
          }`}
        >
          {message.text}
        </div>
      )}

      {/* Module Cards */}
      <div className="space-y-4">
        {modules.length === 0 ? (
          <Card>
            <CardContent>
              <p className="text-slate-400 text-center py-8">No modules available.</p>
            </CardContent>
          </Card>
        ) : (
          modules.map((module) => (
            <Card key={module.name} variant="elevated">
              <CardContent>
                <div className="flex items-start justify-between gap-4 py-2">
                  {/* Left: Module info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-3 mb-2">
                      <Package className="w-5 h-5 text-stark-400 flex-shrink-0" />
                      <h3 className="text-lg font-semibold text-white">{module.name}</h3>
                      <span className="text-xs text-slate-500 bg-slate-700 px-2 py-0.5 rounded">
                        v{module.version}
                      </span>
                      {module.installed && module.enabled ? (
                        <span className="text-xs text-green-400 bg-green-500/20 px-2 py-0.5 rounded flex items-center gap-1">
                          <Check className="w-3 h-3" /> Active
                        </span>
                      ) : (
                        <span className="text-xs text-slate-400 bg-slate-700 px-2 py-0.5 rounded flex items-center gap-1">
                          <Pause className="w-3 h-3" /> Disabled
                        </span>
                      )}
                    </div>

                    <p className="text-slate-400 text-sm mb-3">{module.description}</p>

                    {/* Features badges */}
                    <div className="flex flex-wrap gap-2 mb-3">
                      {module.has_db_tables && (
                        <span className="text-xs text-slate-300 bg-slate-700/50 px-2 py-1 rounded flex items-center gap-1">
                          <Database className="w-3 h-3" /> Database Tables
                        </span>
                      )}
                      {module.has_tools && (
                        <span className="text-xs text-slate-300 bg-slate-700/50 px-2 py-1 rounded flex items-center gap-1">
                          <Wrench className="w-3 h-3" /> AI Tools
                        </span>
                      )}
                      {module.has_worker && (
                        <span className="text-xs text-slate-300 bg-slate-700/50 px-2 py-1 rounded flex items-center gap-1">
                          <Activity className="w-3 h-3" /> Background Worker
                        </span>
                      )}
                    </div>

                    {/* API Keys status */}
                    {module.required_api_keys.length > 0 && (
                      <div className="flex items-center gap-2 text-sm">
                        <Key className="w-3.5 h-3.5 text-slate-500" />
                        <span className="text-slate-500">Required keys:</span>
                        {module.required_api_keys.map((key) => (
                          <span
                            key={key}
                            className={`text-xs px-1.5 py-0.5 rounded ${
                              module.api_keys_met
                                ? 'bg-green-500/20 text-green-400'
                                : 'bg-red-500/20 text-red-400'
                            }`}
                          >
                            {key}
                          </span>
                        ))}
                        {module.api_keys_met ? (
                          <Check className="w-3.5 h-3.5 text-green-400" />
                        ) : (
                          <X className="w-3.5 h-3.5 text-red-400" />
                        )}
                      </div>
                    )}

                    {module.installed_at && (
                      <p className="text-xs text-slate-500 mt-2">
                        Installed: {new Date(module.installed_at).toLocaleDateString()}
                      </p>
                    )}
                  </div>

                  {/* Right: Actions */}
                  <div className="flex flex-col gap-2 flex-shrink-0">
                    {module.enabled && module.has_dashboard && (
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => navigate(`/modules/${encodeURIComponent(module.name)}`)}
                      >
                        <LayoutDashboard className="w-4 h-4 mr-1" />
                        Dashboard
                      </Button>
                    )}
                    {module.enabled ? (
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={actionLoading !== null}
                        isLoading={actionLoading === `${module.name}-disable`}
                        onClick={() => performAction(module.name, 'disable')}
                      >
                        <Pause className="w-4 h-4 mr-1" />
                        Disable
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        variant="primary"
                        disabled={!module.api_keys_met || actionLoading !== null}
                        isLoading={actionLoading === `${module.name}-enable`}
                        onClick={() => performAction(module.name, 'enable')}
                      >
                        <Play className="w-4 h-4 mr-1" />
                        Enable
                      </Button>
                    )}
                    {module.installed && (
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={actionLoading !== null}
                        isLoading={actionLoading === `${module.name}-uninstall`}
                        onClick={() => performAction(module.name, 'uninstall')}
                      >
                        <Trash2 className="w-4 h-4 mr-1" />
                        Uninstall
                      </Button>
                    )}
                  </div>
                </div>
              </CardContent>
            </Card>
          ))
        )}
      </div>

      {/* Help text */}
      <div className="mt-8 p-4 bg-slate-800/50 rounded-lg border border-slate-700">
        <p className="text-sm text-slate-400">
          Modules activate immediately when enabled. Use <strong className="text-slate-300">Reload Modules</strong> to
          re-sync all module tools and workers. You can also manage modules via AI chat:
          <code className="text-stark-400 bg-slate-700 px-1.5 py-0.5 rounded mx-1">
            manage_modules(action="enable", name="wallet_monitor")
          </code>
        </p>
      </div>
    </div>
  );
}
