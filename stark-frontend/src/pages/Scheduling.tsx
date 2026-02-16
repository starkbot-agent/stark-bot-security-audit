import { useState, useEffect, useMemo } from 'react';
import {
  Clock,
  Plus,
  Trash2,
  Play,
  Pause,
  RefreshCw,
  Timer,
  ChevronDown,
  ChevronUp,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Activity,
  Calendar,
  Zap,
} from 'lucide-react';
import Card, { CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import Input from '@/components/ui/Input';
import {
  getCronJobs,
  createCronJob,
  deleteCronJob,
  runCronJobNow,
  pauseCronJob,
  resumeCronJob,
  getCronJobRuns,
  CronJobInfo,
  CronJobRunInfo,
} from '@/lib/api';

function timeAgo(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const diff = now - then;
  if (diff < 0) return timeUntil(dateStr);
  if (diff < 60_000) return 'just now';
  if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86400_000) return `${Math.floor(diff / 3600_000)}h ago`;
  return `${Math.floor(diff / 86400_000)}d ago`;
}

function timeUntil(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const diff = then - now;
  if (diff < 0) return 'overdue';
  if (diff < 60_000) return 'in <1m';
  if (diff < 3600_000) return `in ${Math.floor(diff / 60_000)}m`;
  if (diff < 86400_000) return `in ${Math.floor(diff / 3600_000)}h`;
  return `in ${Math.floor(diff / 86400_000)}d`;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

export default function Scheduling() {
  const [cronJobs, setCronJobs] = useState<CronJobInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);

  // Force re-render for relative times
  const [, setTick] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => setTick((t) => t + 1), 30_000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const jobs = await getCronJobs();
      setCronJobs(jobs);
    } catch (err) {
      setError('Failed to load cron jobs');
    } finally {
      setIsLoading(false);
    }
  };

  const stats = useMemo(() => {
    const active = cronJobs.filter((j) => j.status === 'active').length;
    const paused = cronJobs.filter((j) => j.status === 'paused').length;
    const totalRuns = cronJobs.reduce((sum, j) => sum + (j.run_count || 0), 0);
    const totalErrors = cronJobs.reduce((sum, j) => sum + (j.error_count || 0), 0);
    return { active, paused, totalRuns, totalErrors, total: cronJobs.length };
  }, [cronJobs]);

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading cron jobs...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 md:p-8 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Scheduling</h1>
          <p className="text-sm text-slate-400 mt-1">Manage automated cron jobs</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" size="sm" onClick={loadData}>
            <RefreshCw className="w-4 h-4" />
          </Button>
          {!showCreateForm && (
            <Button onClick={() => setShowCreateForm(true)} size="sm">
              <Plus className="w-4 h-4 mr-1" />
              New Job
            </Button>
          )}
        </div>
      </div>

      {/* Stats bar */}
      {cronJobs.length > 0 && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <div className="bg-slate-800/50 rounded-lg px-4 py-3 border border-slate-700/50">
            <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
              <Activity className="w-3.5 h-3.5" />
              Active
            </div>
            <p className="text-xl font-bold text-green-400">{stats.active}</p>
          </div>
          <div className="bg-slate-800/50 rounded-lg px-4 py-3 border border-slate-700/50">
            <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
              <Pause className="w-3.5 h-3.5" />
              Paused
            </div>
            <p className="text-xl font-bold text-yellow-400">{stats.paused}</p>
          </div>
          <div className="bg-slate-800/50 rounded-lg px-4 py-3 border border-slate-700/50">
            <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
              <Zap className="w-3.5 h-3.5" />
              Total Runs
            </div>
            <p className="text-xl font-bold text-white">{stats.totalRuns}</p>
          </div>
          <div className="bg-slate-800/50 rounded-lg px-4 py-3 border border-slate-700/50">
            <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
              <AlertTriangle className="w-3.5 h-3.5" />
              Errors
            </div>
            <p className={`text-xl font-bold ${stats.totalErrors > 0 ? 'text-red-400' : 'text-slate-500'}`}>
              {stats.totalErrors}
            </p>
          </div>
        </div>
      )}

      {error && (
        <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-3 rounded-lg text-sm flex items-center justify-between">
          <span>{error}</span>
          <button onClick={() => setError(null)} className="text-red-400/60 hover:text-red-400 ml-2">dismiss</button>
        </div>
      )}

      <CronJobsSection
        jobs={cronJobs}
        setJobs={setCronJobs}
        showCreateForm={showCreateForm}
        setShowCreateForm={setShowCreateForm}
        setError={setError}
      />
    </div>
  );
}

interface CronJobsSectionProps {
  jobs: CronJobInfo[];
  setJobs: React.Dispatch<React.SetStateAction<CronJobInfo[]>>;
  showCreateForm: boolean;
  setShowCreateForm: React.Dispatch<React.SetStateAction<boolean>>;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
}

function CronJobsSection({ jobs, setJobs, showCreateForm, setShowCreateForm, setError }: CronJobsSectionProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [expandedJob, setExpandedJob] = useState<number | null>(null);
  const [jobRuns, setJobRuns] = useState<Record<number, CronJobRunInfo[]>>({});

  // Form state
  const [formData, setFormData] = useState({
    name: '',
    description: '',
    schedule_type: 'every',
    schedule_value: '',
    session_mode: 'main',
    message: '',
    timeout_seconds: '',
    delete_after_run: false,
  });

  const [intervalValue, setIntervalValue] = useState(1);
  const [intervalUnit, setIntervalUnit] = useState<'seconds' | 'minutes' | 'hours'>('hours');
  const [showAdvanced, setShowAdvanced] = useState(false);

  const resetForm = () => {
    setFormData({
      name: '',
      description: '',
      schedule_type: 'every',
      schedule_value: '',
      session_mode: 'main',
      message: '',
      timeout_seconds: '',
      delete_after_run: false,
    });
    setIntervalValue(1);
    setIntervalUnit('hours');
    setShowAdvanced(false);
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsCreating(true);
    setError(null);

    let scheduleValue = formData.schedule_value;
    if (formData.schedule_type === 'every') {
      const multipliers = { seconds: 1000, minutes: 60000, hours: 3600000 };
      scheduleValue = String(intervalValue * multipliers[intervalUnit]);
    }

    try {
      const payload: Record<string, unknown> = {
        name: formData.name,
        description: formData.description || undefined,
        schedule_type: formData.schedule_type,
        schedule_value: scheduleValue,
        session_mode: formData.session_mode,
        message: formData.message,
        deliver: false,
        delete_after_run: formData.delete_after_run,
      };
      if (formData.timeout_seconds) payload.timeout_seconds = parseInt(formData.timeout_seconds);

      const newJob = await createCronJob(payload as Parameters<typeof createCronJob>[0]);
      setJobs((prev) => [...prev, newJob]);
      setShowCreateForm(false);
      resetForm();
    } catch (err) {
      setError('Failed to create cron job');
    } finally {
      setIsCreating(false);
    }
  };

  const handleDelete = async (id: number, name: string) => {
    if (!confirm(`Delete job "${name}"?`)) return;
    try {
      await deleteCronJob(id);
      setJobs((prev) => prev.filter((j) => j.id !== id));
    } catch (err) {
      setError('Failed to delete cron job');
    }
  };

  const handleRunNow = async (id: number) => {
    try {
      await runCronJobNow(id);
      const updatedJobs = await getCronJobs();
      setJobs(updatedJobs);
    } catch (err) {
      setError('Failed to run cron job');
    }
  };

  const handleTogglePause = async (job: CronJobInfo) => {
    try {
      const updatedJob = job.status === 'paused'
        ? await resumeCronJob(job.id)
        : await pauseCronJob(job.id);
      setJobs((prev) => prev.map((j) => (j.id === job.id ? updatedJob : j)));
    } catch (err) {
      setError('Failed to toggle job status');
    }
  };

  const handleExpand = async (id: number) => {
    if (expandedJob === id) {
      setExpandedJob(null);
    } else {
      setExpandedJob(id);
      if (!jobRuns[id]) {
        try {
          const runs = await getCronJobRuns(id, 10);
          setJobRuns((prev) => ({ ...prev, [id]: runs }));
        } catch (err) {
          console.error('Failed to load job runs');
        }
      }
    }
  };

  const getScheduleDisplay = (job: CronJobInfo) => {
    switch (job.schedule_type) {
      case 'at':
        return `Once at ${new Date(job.schedule_value).toLocaleString()}`;
      case 'every': {
        const ms = parseInt(job.schedule_value);
        if (ms >= 86400000) return `Every ${Math.round(ms / 86400000)}d`;
        if (ms >= 3600000) return `Every ${Math.round(ms / 3600000)}h`;
        if (ms >= 60000) return `Every ${Math.round(ms / 60000)}m`;
        return `Every ${Math.round(ms / 1000)}s`;
      }
      case 'cron':
        return `cron: ${job.schedule_value}`;
      default:
        return job.schedule_value;
    }
  };

  const getScheduleIcon = (type: string) => {
    switch (type) {
      case 'every':
        return <RefreshCw className="w-4 h-4" />;
      case 'cron':
        return <Clock className="w-4 h-4" />;
      case 'at':
        return <Calendar className="w-4 h-4" />;
      default:
        return <Timer className="w-4 h-4" />;
    }
  };

  return (
    <div className="space-y-4">
      {/* Create Form */}
      {showCreateForm && (
        <Card>
          <CardHeader>
            <CardTitle>New Cron Job</CardTitle>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleCreate} className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <Input
                  label="Name"
                  value={formData.name}
                  onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                  placeholder="e.g. daily-report"
                  required
                />
                <div>
                  <label className="block text-sm font-medium text-slate-300 mb-2">Schedule Type</label>
                  <select
                    value={formData.schedule_type}
                    onChange={(e) => setFormData({ ...formData, schedule_type: e.target.value })}
                    className="w-full px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-white text-sm focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                  >
                    <option value="every">Recurring Interval</option>
                    <option value="cron">Cron Expression</option>
                    <option value="at">One-time (at date)</option>
                  </select>
                </div>
              </div>

              {formData.schedule_type === 'every' ? (
                <div>
                  <label className="block text-sm font-medium text-slate-300 mb-2">Repeat Every</label>
                  <div className="flex gap-2">
                    <input
                      type="number"
                      min="1"
                      value={intervalValue}
                      onChange={(e) => setIntervalValue(parseInt(e.target.value) || 1)}
                      className="w-24 px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-white text-sm focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                      required
                    />
                    <select
                      value={intervalUnit}
                      onChange={(e) => setIntervalUnit(e.target.value as 'seconds' | 'minutes' | 'hours')}
                      className="px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-white text-sm focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                    >
                      <option value="seconds">Seconds</option>
                      <option value="minutes">Minutes</option>
                      <option value="hours">Hours</option>
                    </select>
                  </div>
                </div>
              ) : formData.schedule_type === 'cron' ? (
                <div>
                  <Input
                    label="Cron Expression"
                    value={formData.schedule_value}
                    onChange={(e) => setFormData({ ...formData, schedule_value: e.target.value })}
                    placeholder="0 */6 * * *"
                    required
                  />
                  <p className="mt-1 text-xs text-slate-500">
                    Format: minute hour day-of-month month day-of-week
                  </p>
                </div>
              ) : (
                <Input
                  label="Run At (ISO date)"
                  value={formData.schedule_value}
                  onChange={(e) => setFormData({ ...formData, schedule_value: e.target.value })}
                  placeholder="2025-12-31T12:00:00Z"
                  required
                />
              )}

              <div>
                <label className="block text-sm font-medium text-slate-300 mb-2">Task Message</label>
                <textarea
                  value={formData.message}
                  onChange={(e) => setFormData({ ...formData, message: e.target.value })}
                  placeholder="What should the agent do when this job runs?"
                  rows={2}
                  required
                  className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white text-sm
                    focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent resize-none"
                />
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <Input
                  label="Description (optional)"
                  value={formData.description}
                  onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                  placeholder="What this job does"
                />
                <div>
                  <label className="block text-sm font-medium text-slate-300 mb-2">Session Mode</label>
                  <select
                    value={formData.session_mode}
                    onChange={(e) => setFormData({ ...formData, session_mode: e.target.value })}
                    className="w-full px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-white text-sm focus:ring-2 focus:ring-stark-500 focus:border-transparent"
                  >
                    <option value="main">Main (shared context)</option>
                    <option value="isolated">Isolated (fresh session)</option>
                  </select>
                </div>
              </div>

              {/* Advanced options toggle */}
              <button
                type="button"
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="flex items-center gap-1 text-xs text-slate-400 hover:text-slate-300 transition-colors"
              >
                {showAdvanced ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                Advanced Options
              </button>

              {showAdvanced && (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 p-4 bg-slate-900/50 rounded-lg border border-slate-700/50">
                  <div>
                    <Input
                      label="Timeout (seconds)"
                      type="number"
                      value={formData.timeout_seconds}
                      onChange={(e) => setFormData({ ...formData, timeout_seconds: e.target.value })}
                      placeholder="600"
                    />
                  </div>
                  <div className="md:col-span-2">
                    <label className="flex items-center gap-2 text-sm text-slate-300 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={formData.delete_after_run}
                        onChange={(e) => setFormData({ ...formData, delete_after_run: e.target.checked })}
                        className="rounded border-slate-600 bg-slate-900 text-stark-500 focus:ring-stark-500"
                      />
                      Delete after successful run (one-shot job)
                    </label>
                  </div>
                </div>
              )}

              <div className="flex gap-2 pt-1">
                <Button type="submit" isLoading={isCreating} size="sm">
                  <Plus className="w-4 h-4 mr-1" />
                  Create Job
                </Button>
                <Button type="button" variant="secondary" size="sm" onClick={() => { setShowCreateForm(false); resetForm(); }}>
                  Cancel
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      )}

      {/* Job List */}
      {jobs.length > 0 ? (
        <div className="space-y-3">
          {jobs.map((job) => (
            <JobCard
              key={job.id}
              job={job}
              expanded={expandedJob === job.id}
              runs={jobRuns[job.id]}
              getScheduleDisplay={getScheduleDisplay}
              getScheduleIcon={getScheduleIcon}
              onExpand={() => handleExpand(job.id)}
              onRunNow={() => handleRunNow(job.id)}
              onTogglePause={() => handleTogglePause(job)}
              onDelete={() => handleDelete(job.id, job.name)}
            />
          ))}
        </div>
      ) : (
        !showCreateForm && (
          <div className="text-center py-16 bg-slate-800/30 rounded-xl border border-slate-700/50">
            <Clock className="w-12 h-12 text-slate-600 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-white mb-2">No cron jobs yet</h3>
            <p className="text-sm text-slate-400 mb-6 max-w-md mx-auto">
              Create automated jobs to have your agent perform tasks on a schedule
            </p>
            <Button onClick={() => setShowCreateForm(true)} size="sm">
              <Plus className="w-4 h-4 mr-1" />
              Create Your First Job
            </Button>
          </div>
        )
      )}
    </div>
  );
}

interface JobCardProps {
  job: CronJobInfo;
  expanded: boolean;
  runs?: CronJobRunInfo[];
  getScheduleDisplay: (job: CronJobInfo) => string;
  getScheduleIcon: (type: string) => React.ReactNode;
  onExpand: () => void;
  onRunNow: () => void;
  onTogglePause: () => void;
  onDelete: () => void;
}

function JobCard({ job, expanded, runs, getScheduleDisplay, getScheduleIcon, onExpand, onRunNow, onTogglePause, onDelete }: JobCardProps) {
  const hasErrors = (job.error_count || 0) > 0;
  const isInBackoff = hasErrors && job.status === 'active' && job.last_error;

  const statusConfig: Record<string, { bg: string; text: string; dot: string; label: string }> = {
    active: { bg: 'bg-green-500/10', text: 'text-green-400', dot: 'bg-green-400', label: 'Active' },
    paused: { bg: 'bg-yellow-500/10', text: 'text-yellow-400', dot: 'bg-yellow-400', label: 'Paused' },
    completed: { bg: 'bg-blue-500/10', text: 'text-blue-400', dot: 'bg-blue-400', label: 'Completed' },
    failed: { bg: 'bg-red-500/10', text: 'text-red-400', dot: 'bg-red-400', label: 'Failed' },
  };

  const st = statusConfig[job.status] || { bg: 'bg-slate-700', text: 'text-slate-400', dot: 'bg-slate-400', label: job.status };

  return (
    <div className={`rounded-xl border transition-colors ${
      isInBackoff
        ? 'border-red-500/30 bg-red-500/5'
        : job.status === 'paused'
        ? 'border-yellow-500/20 bg-slate-800/50'
        : 'border-slate-700/50 bg-slate-800/50'
    }`}>
      {/* Main row */}
      <div className="p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-3 min-w-0 flex-1">
            {/* Icon */}
            <div className={`p-2.5 rounded-lg shrink-0 ${
              isInBackoff ? 'bg-red-500/20 text-red-400' :
              job.status === 'paused' ? 'bg-yellow-500/20 text-yellow-400' :
              'bg-stark-500/15 text-stark-400'
            }`}>
              {isInBackoff ? <AlertTriangle className="w-5 h-5" /> : getScheduleIcon(job.schedule_type)}
            </div>

            <div className="min-w-0 flex-1">
              {/* Title row */}
              <div className="flex items-center gap-2 flex-wrap">
                <h3 className="font-semibold text-white truncate">{job.name}</h3>
                <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium ${st.bg} ${st.text}`}>
                  <span className={`w-1.5 h-1.5 rounded-full ${st.dot}`} />
                  {st.label}
                </span>
                <span className="text-xs text-slate-500 bg-slate-800 px-2 py-0.5 rounded-md font-mono">
                  {getScheduleDisplay(job)}
                </span>
                {job.delete_after_run && (
                  <span className="text-[10px] text-slate-500 bg-slate-800 px-1.5 py-0.5 rounded">one-shot</span>
                )}
              </div>

              {/* Description */}
              {job.description && (
                <p className="text-sm text-slate-400 mt-1 truncate">{job.description}</p>
              )}

              {/* Meta row */}
              <div className="flex items-center gap-4 mt-2 text-xs text-slate-500 flex-wrap">
                {job.next_run_at && job.status === 'active' && (
                  <span className="flex items-center gap-1" title={new Date(job.next_run_at).toLocaleString()}>
                    <Clock className="w-3 h-3" />
                    Next: {timeUntil(job.next_run_at)}
                  </span>
                )}
                {job.last_run_at && (
                  <span title={new Date(job.last_run_at).toLocaleString()}>
                    Last: {timeAgo(job.last_run_at)}
                  </span>
                )}
                {(job.run_count || 0) > 0 && (
                  <span className="flex items-center gap-1">
                    <Zap className="w-3 h-3" />
                    {job.run_count} run{job.run_count !== 1 ? 's' : ''}
                  </span>
                )}
                {hasErrors && (
                  <span className="flex items-center gap-1 text-red-400/80">
                    <XCircle className="w-3 h-3" />
                    {job.error_count} error{job.error_count !== 1 ? 's' : ''}
                  </span>
                )}
                {job.session_mode === 'isolated' && (
                  <span className="text-slate-600">isolated</span>
                )}
              </div>

              {/* Error banner */}
              {isInBackoff && (
                <div className="mt-2 flex items-start gap-2 px-3 py-2 bg-red-500/10 rounded-lg border border-red-500/20">
                  <AlertTriangle className="w-3.5 h-3.5 text-red-400 mt-0.5 shrink-0" />
                  <p className="text-xs text-red-400/90 break-all">{job.last_error}</p>
                </div>
              )}
            </div>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-1 shrink-0">
            <button
              onClick={onRunNow}
              title="Run now"
              className="p-2 text-slate-400 hover:text-green-400 hover:bg-green-500/10 rounded-lg transition-colors"
            >
              <Play className="w-4 h-4" />
            </button>
            <button
              onClick={onTogglePause}
              title={job.status === 'paused' ? 'Resume' : 'Pause'}
              className="p-2 text-slate-400 hover:text-yellow-400 hover:bg-yellow-500/10 rounded-lg transition-colors"
            >
              {job.status === 'paused' ? <Play className="w-4 h-4" /> : <Pause className="w-4 h-4" />}
            </button>
            <button
              onClick={onExpand}
              title="Details"
              className="p-2 text-slate-400 hover:text-white hover:bg-slate-700/50 rounded-lg transition-colors"
            >
              {expanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
            </button>
            <button
              onClick={onDelete}
              title="Delete"
              className="p-2 text-slate-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Expanded details */}
      {expanded && (
        <div className="px-4 pb-4 border-t border-slate-700/50">
          <div className="pt-4 space-y-4">
            {/* Info grid */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
              <DetailField label="Message" value={job.message || '(none)'} mono />
              <DetailField label="Session Mode" value={job.session_mode === 'main' ? 'Main (shared)' : 'Isolated'} />
              <DetailField label="Schedule Type" value={job.schedule_type} />
              {job.timeout_seconds && <DetailField label="Timeout" value={`${job.timeout_seconds}s`} />}
              <DetailField label="Created" value={new Date(job.created_at).toLocaleString()} />
              {job.next_run_at && (
                <DetailField label="Next Run" value={new Date(job.next_run_at).toLocaleString()} />
              )}
            </div>

            {/* Run history */}
            {runs && runs.length > 0 && (
              <div>
                <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">
                  Recent Runs ({runs.length})
                </h4>
                <div className="space-y-1.5">
                  {runs.map((run) => (
                    <div
                      key={run.id}
                      className={`flex items-center gap-3 px-3 py-2 rounded-lg text-sm ${
                        run.success
                          ? 'bg-green-500/5 border border-green-500/10'
                          : 'bg-red-500/5 border border-red-500/10'
                      }`}
                    >
                      {run.success ? (
                        <CheckCircle className="w-4 h-4 text-green-400 shrink-0" />
                      ) : (
                        <XCircle className="w-4 h-4 text-red-400 shrink-0" />
                      )}
                      <span className="text-xs text-slate-400 shrink-0">
                        {timeAgo(run.started_at)}
                      </span>
                      {run.duration_ms != null && (
                        <span className="text-xs text-slate-500 shrink-0">
                          {formatDuration(run.duration_ms)}
                        </span>
                      )}
                      {run.error && (
                        <span className="text-xs text-red-400/80 truncate flex-1" title={run.error}>
                          {run.error}
                        </span>
                      )}
                      {run.success && !run.error && (
                        <span className="text-xs text-green-400/60 flex-1">Success</span>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {runs && runs.length === 0 && (
              <p className="text-xs text-slate-500 italic">No runs recorded yet</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function DetailField({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <p className="text-[11px] text-slate-500 uppercase tracking-wider mb-0.5">{label}</p>
      <p className={`text-sm text-slate-300 ${mono ? 'font-mono text-xs' : ''} break-all`}>{value}</p>
    </div>
  );
}
