import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, Users, Wallet, Activity, AlertTriangle, ExternalLink } from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import { apiFetch } from '@/lib/api';

export default function ModuleDashboard() {
  const { name } = useParams<{ name: string }>();
  const [data, setData] = useState<any>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!name) return;
    loadDashboard();
  }, [name]);

  const loadDashboard = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await apiFetch(`/modules/${encodeURIComponent(name!)}/dashboard`);
      setData(result);
    } catch (err: any) {
      let msg = err.message || 'Failed to load dashboard';
      try {
        const parsed = JSON.parse(msg);
        msg = parsed.error || msg;
      } catch {}
      setError(msg);
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center min-h-[400px]">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading dashboard...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-8">
        <BackLink />
        <div className="bg-red-500/20 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg mt-4">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div className="p-8">
      <BackLink />
      <h1 className="text-2xl font-bold text-white mb-6 mt-4">
        {formatModuleName(name!)} Dashboard
      </h1>

      {name === 'discord_tipping' && data ? (
        <DiscordTippingDashboard data={data} />
      ) : name === 'wallet_monitor' && data ? (
        <WalletMonitorDashboard data={data} />
      ) : (
        <GenericDashboard data={data} />
      )}
    </div>
  );
}

function BackLink() {
  return (
    <Link to="/modules" className="inline-flex items-center gap-1.5 text-sm text-slate-400 hover:text-white transition-colors">
      <ArrowLeft className="w-4 h-4" />
      Back to Modules
    </Link>
  );
}

function formatModuleName(name: string): string {
  return name
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}

// ─── Discord Tipping Dashboard ────────────────────────────────────

function DiscordTippingDashboard({ data }: { data: any }) {
  const profiles = data.profiles || [];
  const registered = profiles.filter((p: any) => p.registration_status === 'registered');
  const unregistered = profiles.filter((p: any) => p.registration_status !== 'registered');

  return (
    <div className="space-y-6">
      {/* Stats row */}
      <div className="grid grid-cols-3 gap-4">
        <StatCard icon={<Users className="w-5 h-5" />} label="Total Profiles" value={data.total_profiles ?? 0} />
        <StatCard icon={<Wallet className="w-5 h-5" />} label="Registered" value={data.registered_count ?? 0} color="green" />
        <StatCard icon={<AlertTriangle className="w-5 h-5" />} label="Unregistered" value={data.unregistered_count ?? 0} color="yellow" />
      </div>

      {/* Registered profiles table */}
      {registered.length > 0 && (
        <Card variant="elevated">
          <CardContent>
            <h3 className="text-lg font-semibold text-white mb-4">Registered Users</h3>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-slate-400 border-b border-slate-700">
                    <th className="pb-2 pr-4">Username</th>
                    <th className="pb-2 pr-4">Discord ID</th>
                    <th className="pb-2 pr-4">Wallet Address</th>
                    <th className="pb-2">Registered</th>
                  </tr>
                </thead>
                <tbody>
                  {registered.map((p: any, i: number) => (
                    <tr key={i} className="border-b border-slate-700/50">
                      <td className="py-2 pr-4 text-white">{p.discord_username || '—'}</td>
                      <td className="py-2 pr-4 text-slate-400 font-mono text-xs">{p.discord_user_id}</td>
                      <td className="py-2 pr-4">
                        <code className="text-stark-400 text-xs">{truncateAddress(p.public_address)}</code>
                      </td>
                      <td className="py-2 text-slate-500 text-xs">{p.registered_at || '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Unregistered profiles */}
      {unregistered.length > 0 && (
        <Card>
          <CardContent>
            <h3 className="text-lg font-semibold text-white mb-4">
              Unregistered Users ({unregistered.length})
            </h3>
            <div className="flex flex-wrap gap-2">
              {unregistered.map((p: any, i: number) => (
                <span key={i} className="text-xs text-slate-400 bg-slate-700/50 px-2 py-1 rounded">
                  {p.discord_username || p.discord_user_id}
                </span>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

// ─── Wallet Monitor Dashboard ─────────────────────────────────────

function WalletMonitorDashboard({ data }: { data: any }) {
  const watchlist = data.watchlist || [];
  const recentActivity = data.recent_activity || [];

  return (
    <div className="space-y-6">
      {/* Stats row */}
      <div className="grid grid-cols-4 gap-4">
        <StatCard icon={<Wallet className="w-5 h-5" />} label="Watched Wallets" value={data.watched_wallets ?? 0} />
        <StatCard icon={<Activity className="w-5 h-5" />} label="Active Wallets" value={data.active_wallets ?? 0} color="green" />
        <StatCard icon={<ExternalLink className="w-5 h-5" />} label="Total Transactions" value={data.total_transactions ?? 0} />
        <StatCard icon={<AlertTriangle className="w-5 h-5" />} label="Large Trades" value={data.large_trades ?? 0} color="yellow" />
      </div>

      {/* Watchlist table */}
      {watchlist.length > 0 && (
        <Card variant="elevated">
          <CardContent>
            <h3 className="text-lg font-semibold text-white mb-4">Watchlist</h3>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-slate-400 border-b border-slate-700">
                    <th className="pb-2 pr-4">Label</th>
                    <th className="pb-2 pr-4">Address</th>
                    <th className="pb-2 pr-4">Chain</th>
                    <th className="pb-2 pr-4">Threshold</th>
                    <th className="pb-2">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {watchlist.map((w: any, i: number) => (
                    <tr key={i} className="border-b border-slate-700/50">
                      <td className="py-2 pr-4 text-white">{w.label || '—'}</td>
                      <td className="py-2 pr-4">
                        <code className="text-stark-400 text-xs">{truncateAddress(w.address)}</code>
                      </td>
                      <td className="py-2 pr-4 text-slate-400">{w.chain}</td>
                      <td className="py-2 pr-4 text-slate-400">${w.large_trade_threshold_usd?.toLocaleString()}</td>
                      <td className="py-2">
                        {w.monitor_enabled ? (
                          <span className="text-xs text-green-400 bg-green-500/20 px-2 py-0.5 rounded">Active</span>
                        ) : (
                          <span className="text-xs text-slate-400 bg-slate-700 px-2 py-0.5 rounded">Paused</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Recent Activity */}
      {recentActivity.length > 0 && (
        <Card variant="elevated">
          <CardContent>
            <h3 className="text-lg font-semibold text-white mb-4">Recent Activity</h3>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-slate-400 border-b border-slate-700">
                    <th className="pb-2 pr-4">Type</th>
                    <th className="pb-2 pr-4">Chain</th>
                    <th className="pb-2 pr-4">Amount</th>
                    <th className="pb-2 pr-4">USD Value</th>
                    <th className="pb-2">Tx Hash</th>
                  </tr>
                </thead>
                <tbody>
                  {recentActivity.map((a: any, i: number) => (
                    <tr key={i} className="border-b border-slate-700/50">
                      <td className="py-2 pr-4">
                        <span className={`text-xs px-2 py-0.5 rounded ${
                          a.is_large_trade ? 'bg-yellow-500/20 text-yellow-400' : 'bg-slate-700 text-slate-300'
                        }`}>
                          {a.activity_type}
                        </span>
                      </td>
                      <td className="py-2 pr-4 text-slate-400">{a.chain}</td>
                      <td className="py-2 pr-4 text-white">
                        {a.amount_formatted ? `${a.amount_formatted} ${a.asset_symbol || ''}` : '—'}
                      </td>
                      <td className="py-2 pr-4 text-slate-400">
                        {a.usd_value != null ? `$${a.usd_value.toLocaleString()}` : '—'}
                      </td>
                      <td className="py-2">
                        <code className="text-stark-400 text-xs">{truncateAddress(a.tx_hash)}</code>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

// ─── Generic Dashboard (fallback) ────────────────────────────────

function GenericDashboard({ data }: { data: any }) {
  return (
    <Card variant="elevated">
      <CardContent>
        <h3 className="text-lg font-semibold text-white mb-4">Module Data</h3>
        <pre className="text-xs text-slate-300 bg-slate-800 p-4 rounded-lg overflow-auto max-h-96">
          {JSON.stringify(data, null, 2)}
        </pre>
      </CardContent>
    </Card>
  );
}

// ─── Shared Components ────────────────────────────────────────────

function StatCard({
  icon,
  label,
  value,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  color?: 'green' | 'yellow';
}) {
  const colorClass =
    color === 'green'
      ? 'text-green-400'
      : color === 'yellow'
      ? 'text-yellow-400'
      : 'text-stark-400';

  return (
    <Card>
      <CardContent>
        <div className="flex items-center gap-3 py-1">
          <div className={`${colorClass}`}>{icon}</div>
          <div>
            <p className="text-2xl font-bold text-white">{value}</p>
            <p className="text-xs text-slate-400">{label}</p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function truncateAddress(addr: string | null | undefined): string {
  if (!addr) return '—';
  if (addr.length <= 14) return addr;
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`;
}
