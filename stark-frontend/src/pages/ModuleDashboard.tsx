import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, ExternalLink, AlertCircle } from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import { apiFetch } from '@/lib/api';

interface ModuleInfo {
  name: string;
  description: string;
  version: string;
  service_url: string;
  service_port: number;
  has_dashboard: boolean;
}

export default function ModuleDashboard() {
  const { name } = useParams<{ name: string }>();
  const [module, setModule] = useState<ModuleInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [iframeError, setIframeError] = useState(false);

  useEffect(() => {
    if (!name) return;
    loadModule();
  }, [name]);

  const loadModule = async () => {
    try {
      const modules = await apiFetch<ModuleInfo[]>('/modules');
      const found = modules.find((m) => m.name === name);
      setModule(found || null);
    } catch {
      // ignore
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center min-h-[400px]">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 flex flex-col" style={{ height: 'calc(100vh - 64px)' }}>
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-4">
          <Link
            to="/modules"
            className="inline-flex items-center gap-1.5 text-sm text-slate-400 hover:text-white transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            Back to Modules
          </Link>
          <h1 className="text-2xl font-bold text-white">
            {formatModuleName(name!)} Dashboard
          </h1>
        </div>
        {module?.has_dashboard && (
          <a
            href={`/api/modules/${encodeURIComponent(name!)}/proxy/`}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-slate-300 bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors"
          >
            <ExternalLink className="w-4 h-4" />
            Open in new tab
          </a>
        )}
      </div>

      {module?.has_dashboard ? (
        <div className="flex-1 rounded-lg overflow-hidden border border-slate-700 bg-white">
          {iframeError ? (
            <div className="flex flex-col items-center justify-center h-full bg-slate-900 text-slate-400 gap-3">
              <AlertCircle className="w-8 h-8" />
              <p>Unable to load the dashboard. The service may be offline.</p>
              <a
                href={`/api/modules/${encodeURIComponent(name!)}/proxy/`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-stark-400 hover:text-stark-300 underline text-sm"
              >
                Try opening via proxy
              </a>
            </div>
          ) : (
            <iframe
              src={`/api/modules/${encodeURIComponent(name!)}/proxy/`}
              className="w-full h-full border-0"
              title={`${formatModuleName(name!)} Dashboard`}
              onError={() => setIframeError(true)}
            />
          )}
        </div>
      ) : (
        <Card>
          <CardContent>
            <p className="text-slate-400 text-center py-8">
              {module
                ? 'This module does not have a dashboard.'
                : `Module "${name}" not found.`}
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function formatModuleName(name: string): string {
  return name
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}
