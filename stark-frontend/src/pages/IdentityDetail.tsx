import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ChevronLeft, Users, MessageSquare, Wrench, CheckCircle, XCircle, Clock, Activity } from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import { getIdentityLogs, IdentityLogs, IdentitySession, ToolExecution } from '@/lib/api';

type CompletionStatus = 'active' | 'complete' | 'cancelled' | 'failed';

const statusConfig: Record<CompletionStatus, { bg: string; text: string; label: string }> = {
  active: { bg: 'bg-blue-500/20', text: 'text-blue-400', label: 'Active' },
  complete: { bg: 'bg-green-500/20', text: 'text-green-400', label: 'Complete' },
  cancelled: { bg: 'bg-yellow-500/20', text: 'text-yellow-400', label: 'Cancelled' },
  failed: { bg: 'bg-red-500/20', text: 'text-red-400', label: 'Failed' },
};

function isValidStatus(status: string | undefined): status is CompletionStatus {
  return status !== undefined && ['active', 'complete', 'cancelled', 'failed'].includes(status);
}

export default function IdentityDetail() {
  const { identityId } = useParams<{ identityId: string }>();
  const navigate = useNavigate();
  const [logs, setLogs] = useState<IdentityLogs | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'sessions' | 'tools'>('sessions');

  useEffect(() => {
    if (identityId) {
      loadLogs();
    }
  }, [identityId]);

  const loadLogs = async () => {
    if (!identityId) return;
    try {
      setIsLoading(true);
      const data = await getIdentityLogs(identityId);
      setLogs(data);
    } catch (err) {
      setError('Failed to load identity logs');
    } finally {
      setIsLoading(false);
    }
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString();
  };

  const formatDuration = (ms?: number) => {
    if (!ms) return '-';
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading identity logs...</span>
        </div>
      </div>
    );
  }

  if (error || !logs) {
    return (
      <div className="p-8">
        <Button variant="ghost" onClick={() => navigate('/identities')} className="mb-4">
          <ChevronLeft className="w-4 h-4 mr-2" />
          Back to Identities
        </Button>
        <div className="bg-red-500/20 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg">
          {error || 'Identity not found'}
        </div>
      </div>
    );
  }

  const primaryAccount = logs.linked_accounts[0];
  const totalToolCalls = logs.tool_stats.reduce((sum, stat) => sum + stat.total_calls, 0);
  const successfulToolCalls = logs.tool_stats.reduce((sum, stat) => sum + stat.successful_calls, 0);

  return (
    <div className="p-8">
      {/* Header */}
      <div className="mb-6">
        <Button variant="ghost" onClick={() => navigate('/identities')} className="mb-4">
          <ChevronLeft className="w-4 h-4 mr-2" />
          Back to Identities
        </Button>
        <div className="flex items-start gap-4">
          <div className="p-4 bg-purple-500/20 rounded-lg">
            <Users className="w-8 h-8 text-purple-400" />
          </div>
          <div>
            <h1 className="text-2xl font-bold text-white">
              {primaryAccount?.platform_user_name || primaryAccount?.platform_user_id || 'Unknown'}
            </h1>
            <p className="text-slate-400 text-sm mt-1">
              Identity ID: {logs.identity_id}
            </p>
          </div>
        </div>
      </div>

      {/* Linked Accounts */}
      <Card className="mb-6">
        <CardContent>
          <h2 className="text-lg font-semibold text-white mb-4">Linked Accounts</h2>
          <div className="flex flex-wrap gap-3">
            {logs.linked_accounts.map((account, idx) => (
              <div
                key={idx}
                className="flex items-center gap-2 px-3 py-2 bg-slate-700/50 rounded-lg"
              >
                <span className="px-2 py-0.5 bg-slate-600 rounded text-xs text-slate-300">
                  {account.channel_type}
                </span>
                <span className="text-white">
                  {account.platform_user_name || account.platform_user_id}
                </span>
                {account.is_verified && (
                  <CheckCircle className="w-4 h-4 text-green-400" />
                )}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        <Card>
          <CardContent className="flex items-center gap-4">
            <div className="p-3 bg-blue-500/20 rounded-lg">
              <MessageSquare className="w-6 h-6 text-blue-400" />
            </div>
            <div>
              <p className="text-2xl font-bold text-white">{logs.session_count}</p>
              <p className="text-slate-400 text-sm">Sessions</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="flex items-center gap-4">
            <div className="p-3 bg-green-500/20 rounded-lg">
              <Wrench className="w-6 h-6 text-green-400" />
            </div>
            <div>
              <p className="text-2xl font-bold text-white">{totalToolCalls}</p>
              <p className="text-slate-400 text-sm">Tool Calls</p>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="flex items-center gap-4">
            <div className="p-3 bg-purple-500/20 rounded-lg">
              <Activity className="w-6 h-6 text-purple-400" />
            </div>
            <div>
              <p className="text-2xl font-bold text-white">
                {totalToolCalls > 0 ? Math.round((successfulToolCalls / totalToolCalls) * 100) : 0}%
              </p>
              <p className="text-slate-400 text-sm">Success Rate</p>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 mb-4">
        <Button
          variant={activeTab === 'sessions' ? 'primary' : 'ghost'}
          onClick={() => setActiveTab('sessions')}
        >
          <MessageSquare className="w-4 h-4 mr-2" />
          Sessions ({logs.sessions.length})
        </Button>
        <Button
          variant={activeTab === 'tools' ? 'primary' : 'ghost'}
          onClick={() => setActiveTab('tools')}
        >
          <Wrench className="w-4 h-4 mr-2" />
          Tool Activity
        </Button>
      </div>

      {/* Sessions Tab */}
      {activeTab === 'sessions' && (
        <div className="space-y-3">
          {logs.sessions.length === 0 ? (
            <Card>
              <CardContent className="text-center py-8">
                <MessageSquare className="w-12 h-12 text-slate-600 mx-auto mb-4" />
                <p className="text-slate-400">No sessions found for this identity</p>
              </CardContent>
            </Card>
          ) : (
            logs.sessions.map((session: IdentitySession) => {
              const status = isValidStatus(session.completion_status)
                ? session.completion_status
                : 'active';
              const config = statusConfig[status];

              return (
                <Card
                  key={session.id}
                  className="cursor-pointer hover:border-stark-500/50 transition-colors"
                  onClick={() => navigate(`/sessions?id=${session.id}`)}
                >
                  <CardContent>
                    <div className="flex items-start justify-between">
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-2">
                          <span className={`px-2 py-0.5 rounded text-xs ${config.bg} ${config.text}`}>
                            {config.label}
                          </span>
                          <span className="px-2 py-0.5 bg-slate-700 rounded text-xs text-slate-300">
                            {session.channel_type}
                          </span>
                          <span className="text-slate-500 text-xs">
                            #{session.id}
                          </span>
                        </div>
                        {session.initial_query && (
                          <p className="text-white truncate mb-2">
                            {session.initial_query}
                          </p>
                        )}
                        <div className="flex items-center gap-4 text-sm text-slate-400">
                          <span className="flex items-center gap-1">
                            <MessageSquare className="w-4 h-4" />
                            {session.message_count} messages
                          </span>
                          <span className="flex items-center gap-1">
                            <Clock className="w-4 h-4" />
                            {formatDate(session.last_activity_at)}
                          </span>
                        </div>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              );
            })
          )}
        </div>
      )}

      {/* Tools Tab */}
      {activeTab === 'tools' && (
        <div className="space-y-6">
          {/* Tool Stats */}
          {logs.tool_stats.length > 0 && (
            <Card>
              <CardContent>
                <h3 className="text-lg font-semibold text-white mb-4">Tool Usage Summary</h3>
                <div className="space-y-3">
                  {logs.tool_stats.map((stat, idx) => (
                    <div key={idx} className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className="p-2 bg-slate-700 rounded">
                          <Wrench className="w-4 h-4 text-slate-400" />
                        </div>
                        <span className="text-white font-medium">{stat.tool_name}</span>
                      </div>
                      <div className="flex items-center gap-4">
                        <span className="text-slate-400 text-sm">
                          {stat.successful_calls}/{stat.total_calls} successful
                        </span>
                        <div className="w-24 h-2 bg-slate-700 rounded-full overflow-hidden">
                          <div
                            className="h-full bg-green-500"
                            style={{
                              width: `${stat.total_calls > 0 ? (stat.successful_calls / stat.total_calls) * 100 : 0}%`,
                            }}
                          />
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          {/* Recent Executions */}
          <Card>
            <CardContent>
              <h3 className="text-lg font-semibold text-white mb-4">Recent Tool Executions</h3>
              {logs.recent_tool_executions.length === 0 ? (
                <p className="text-slate-400 text-center py-4">No tool executions recorded</p>
              ) : (
                <div className="space-y-2">
                  {logs.recent_tool_executions.map((exec: ToolExecution) => (
                    <div
                      key={exec.id}
                      className="flex items-center justify-between p-3 bg-slate-800/50 rounded-lg"
                    >
                      <div className="flex items-center gap-3">
                        {exec.success ? (
                          <CheckCircle className="w-5 h-5 text-green-400" />
                        ) : (
                          <XCircle className="w-5 h-5 text-red-400" />
                        )}
                        <div>
                          <span className="text-white font-medium">{exec.tool_name}</span>
                          {exec.result && (
                            <p className="text-slate-400 text-sm truncate max-w-md">
                              {exec.result.substring(0, 100)}
                              {exec.result.length > 100 ? '...' : ''}
                            </p>
                          )}
                        </div>
                      </div>
                      <div className="flex items-center gap-4 text-sm text-slate-400">
                        <span>{formatDuration(exec.duration_ms)}</span>
                        <span>{formatDate(exec.executed_at)}</span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
