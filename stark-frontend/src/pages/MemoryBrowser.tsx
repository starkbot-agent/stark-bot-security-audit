import { useState, useEffect, useMemo } from 'react';
import {
  FileText,
  Search,
  Calendar,
  Brain,
  Clock,
  User,
  FolderOpen,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
  BarChart3,
  Plus,
  X,
} from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import { apiFetch } from '@/lib/api';

// ============================================================================
// API Types (matching backend)
// ============================================================================

interface MemoryFile {
  path: string;
  name: string;
  file_type: 'daily_log' | 'long_term' | 'unknown';
  date: string | null;
  identity_id: string | null;
  size: number;
  modified: string | null;
}

interface ListFilesResponse {
  success: boolean;
  files: MemoryFile[];
  error?: string;
}

interface ReadFileResponse {
  success: boolean;
  path: string;
  content?: string;
  file_type?: string;
  date?: string;
  identity_id?: string;
  error?: string;
}

interface SearchResult {
  file_path: string;
  snippet: string;
  score: number;
}

interface SearchResponse {
  success: boolean;
  query: string;
  results: SearchResult[];
  error?: string;
}

interface DateRange {
  oldest: string;
  newest: string;
}

interface MemoryStats {
  total_files: number;
  daily_log_count: number;
  long_term_count: number;
  identity_count: number;
  identities: string[];
  date_range: DateRange | null;
}

interface StatsResponse {
  success: boolean;
  stats: MemoryStats;
  error?: string;
}

// ============================================================================
// Helpers
// ============================================================================

/** Render a human-readable label for a memory identity */
function identityLabel(id: string | null): string {
  if (!id) return 'Standard';
  if (id === 'safemode') return 'Safe Mode';
  // UUID identity — truncate
  return id.length > 12 ? id.slice(0, 8) + '...' : id;
}

/** Mode badge component for file list items */
function ModeBadge({ identityId }: { identityId: string | null }) {
  if (identityId === 'safemode') {
    return (
      <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/20 text-amber-400 font-medium">
        Safe
      </span>
    );
  }
  return null;
}

// ============================================================================
// API Functions
// ============================================================================

async function getMemoryFiles(): Promise<ListFilesResponse> {
  return apiFetch('/memory/files');
}

async function readMemoryFile(path: string): Promise<ReadFileResponse> {
  return apiFetch(`/memory/file?path=${encodeURIComponent(path)}`);
}

async function searchMemory(query: string, limit = 20): Promise<SearchResponse> {
  return apiFetch(`/memory/search?query=${encodeURIComponent(query)}&limit=${limit}`);
}

async function getMemoryStats(): Promise<StatsResponse> {
  return apiFetch('/memory/stats');
}

async function getDailyLog(date?: string, identityId?: string): Promise<ReadFileResponse> {
  const params = new URLSearchParams();
  if (date) params.set('date', date);
  if (identityId) params.set('identity_id', identityId);
  const query = params.toString();
  return apiFetch(`/memory/daily${query ? `?${query}` : ''}`);
}

async function reindexMemory(): Promise<{ success: boolean; message?: string; error?: string }> {
  return apiFetch('/memory/reindex', { method: 'POST' });
}

async function appendToDailyLog(content: string, identityId?: string): Promise<{ success: boolean; error?: string }> {
  return apiFetch('/memory/daily', {
    method: 'POST',
    body: JSON.stringify({ content, identity_id: identityId }),
  });
}

async function appendToLongTerm(content: string, identityId?: string): Promise<{ success: boolean; error?: string }> {
  return apiFetch('/memory/long-term', {
    method: 'POST',
    body: JSON.stringify({ content, identity_id: identityId }),
  });
}

// ============================================================================
// Components
// ============================================================================

type ViewMode = 'files' | 'calendar' | 'search';

function FileTypeIcon({ type }: { type: string }) {
  if (type === 'long_term') {
    return <Brain className="w-4 h-4 text-purple-400" />;
  }
  if (type === 'daily_log') {
    return <Calendar className="w-4 h-4 text-blue-400" />;
  }
  return <FileText className="w-4 h-4 text-slate-400" />;
}

function CalendarView({
  files,
  selectedDate,
  onSelectDate,
}: {
  files: MemoryFile[];
  selectedDate: string | null;
  onSelectDate: (date: string) => void;
}) {
  const [currentMonth, setCurrentMonth] = useState(() => {
    const now = new Date();
    return new Date(now.getFullYear(), now.getMonth(), 1);
  });

  // Get dates with entries
  const datesWithEntries = useMemo(() => {
    const dates = new Set<string>();
    files.forEach((f) => {
      if (f.date) dates.add(f.date);
    });
    return dates;
  }, [files]);

  // Generate calendar days
  const calendarDays = useMemo(() => {
    const year = currentMonth.getFullYear();
    const month = currentMonth.getMonth();

    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const startPadding = firstDay.getDay();
    const totalDays = lastDay.getDate();

    const days: { date: string | null; day: number | null; hasEntry: boolean }[] = [];

    // Padding for days before the 1st
    for (let i = 0; i < startPadding; i++) {
      days.push({ date: null, day: null, hasEntry: false });
    }

    // Actual days
    for (let d = 1; d <= totalDays; d++) {
      const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
      days.push({
        date: dateStr,
        day: d,
        hasEntry: datesWithEntries.has(dateStr),
      });
    }

    return days;
  }, [currentMonth, datesWithEntries]);

  const monthName = currentMonth.toLocaleString('default', { month: 'long', year: 'numeric' });

  const prevMonth = () => {
    setCurrentMonth(new Date(currentMonth.getFullYear(), currentMonth.getMonth() - 1, 1));
  };

  const nextMonth = () => {
    setCurrentMonth(new Date(currentMonth.getFullYear(), currentMonth.getMonth() + 1, 1));
  };

  const today = new Date().toISOString().split('T')[0];

  return (
    <div className="bg-slate-800/50 rounded-lg p-4">
      {/* Month navigation */}
      <div className="flex items-center justify-between mb-4">
        <Button variant="ghost" size="sm" onClick={prevMonth}>
          <ChevronLeft className="w-4 h-4" />
        </Button>
        <span className="text-white font-medium">{monthName}</span>
        <Button variant="ghost" size="sm" onClick={nextMonth}>
          <ChevronRight className="w-4 h-4" />
        </Button>
      </div>

      {/* Day headers */}
      <div className="grid grid-cols-7 gap-1 mb-2">
        {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map((day) => (
          <div key={day} className="text-center text-xs text-slate-500 py-1">
            {day}
          </div>
        ))}
      </div>

      {/* Calendar grid */}
      <div className="grid grid-cols-7 gap-1">
        {calendarDays.map((d, i) => (
          <button
            key={i}
            disabled={!d.date}
            onClick={() => d.date && onSelectDate(d.date)}
            className={`
              aspect-square flex items-center justify-center text-sm rounded transition-colors
              ${!d.date ? 'cursor-default' : 'cursor-pointer'}
              ${d.date === selectedDate ? 'bg-stark-500 text-white' : ''}
              ${d.date === today && d.date !== selectedDate ? 'ring-1 ring-stark-400' : ''}
              ${d.hasEntry && d.date !== selectedDate ? 'bg-blue-500/20 text-blue-300 hover:bg-blue-500/30' : ''}
              ${!d.hasEntry && d.date && d.date !== selectedDate ? 'text-slate-500 hover:bg-slate-700' : ''}
            `}
          >
            {d.day}
            {d.hasEntry && d.date !== selectedDate && (
              <span className="absolute bottom-1 w-1 h-1 rounded-full bg-blue-400" />
            )}
          </button>
        ))}
      </div>
    </div>
  );
}

function SearchView({
  onSelectFile,
}: {
  onSelectFile: (path: string) => void;
}) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async () => {
    if (!query.trim()) return;

    setIsSearching(true);
    setError(null);

    try {
      const response = await searchMemory(query, 30);
      if (response.success) {
        setResults(response.results);
      } else {
        setError(response.error || 'Search failed');
      }
    } catch (err) {
      setError('Search request failed');
    } finally {
      setIsSearching(false);
    }
  };

  // Format snippet with highlights
  const formatSnippet = (snippet: string) => {
    // The backend wraps matches in >>> and <<<
    const parts = snippet.split(/(>>>.*?<<<)/g);
    return parts.map((part, i) => {
      if (part.startsWith('>>>') && part.endsWith('<<<')) {
        return (
          <mark key={i} className="bg-yellow-500/30 text-yellow-200 px-0.5 rounded">
            {part.slice(3, -3)}
          </mark>
        );
      }
      return part;
    });
  };

  return (
    <div className="space-y-4">
      {/* Search input */}
      <div className="flex gap-2">
        <div className="flex-1 relative">
          <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            placeholder="Search memories (BM25 full-text)..."
            className="w-full pl-10 pr-4 py-2 bg-slate-800 border border-slate-700 rounded-lg text-white text-sm focus:outline-none focus:border-stark-500"
          />
        </div>
        <Button onClick={handleSearch} disabled={isSearching || !query.trim()}>
          {isSearching ? 'Searching...' : 'Search'}
        </Button>
      </div>

      {error && (
        <div className="text-red-400 text-sm bg-red-500/10 px-3 py-2 rounded">
          {error}
        </div>
      )}

      {/* Results */}
      {results.length > 0 ? (
        <div className="space-y-2">
          <div className="text-sm text-slate-400">{results.length} results found</div>
          {results.map((result, i) => (
            <button
              key={i}
              onClick={() => onSelectFile(result.file_path)}
              className="w-full text-left p-3 bg-slate-800/50 hover:bg-slate-700/50 rounded-lg transition-colors"
            >
              <div className="flex items-center gap-2 mb-1">
                <FileText className="w-4 h-4 text-slate-400" />
                <span className="text-sm text-stark-400 font-mono">{result.file_path}</span>
                <span className="text-xs text-slate-500 ml-auto">
                  score: {Math.abs(result.score).toFixed(2)}
                </span>
              </div>
              <p className="text-sm text-slate-300 line-clamp-2">{formatSnippet(result.snippet)}</p>
            </button>
          ))}
        </div>
      ) : query && !isSearching ? (
        <div className="text-center text-slate-500 py-8">
          No results found for "{query}"
        </div>
      ) : null}
    </div>
  );
}

function MarkdownViewer({ content }: { content: string }) {
  // Simple markdown rendering - just handle headers and basic formatting
  const lines = content.split('\n');

  return (
    <div className="prose prose-invert prose-sm max-w-none">
      {lines.map((line, i) => {
        if (line.startsWith('## ')) {
          return (
            <h2 key={i} className="text-lg font-semibold text-stark-400 mt-4 mb-2">
              {line.slice(3)}
            </h2>
          );
        }
        if (line.startsWith('# ')) {
          return (
            <h1 key={i} className="text-xl font-bold text-white mt-4 mb-2">
              {line.slice(2)}
            </h1>
          );
        }
        if (line.startsWith('- ')) {
          return (
            <li key={i} className="text-slate-300 ml-4">
              {line.slice(2)}
            </li>
          );
        }
        if (line.trim() === '') {
          return <div key={i} className="h-2" />;
        }
        return (
          <p key={i} className="text-slate-300">
            {line}
          </p>
        );
      })}
    </div>
  );
}

function AddEntryModal({
  type,
  identityId,
  onClose,
  onSuccess,
}: {
  type: 'daily' | 'long_term';
  identityId: string | null;
  onClose: () => void;
  onSuccess: () => void;
}) {
  const [content, setContent] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!content.trim()) return;

    setIsSubmitting(true);
    setError(null);

    try {
      const response = type === 'daily'
        ? await appendToDailyLog(content, identityId || undefined)
        : await appendToLongTerm(content, identityId || undefined);

      if (response.success) {
        onSuccess();
        onClose();
      } else {
        setError(response.error || 'Failed to add entry');
      }
    } catch {
      setError('Request failed');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-slate-900 rounded-lg p-6 max-w-lg w-full mx-4">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">
            Add to {type === 'daily' ? 'Daily Log' : 'Long-term Memory'}
          </h2>
          <button onClick={onClose} className="text-slate-400 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        {identityId && (
          <div className="text-sm text-slate-400 mb-3">
            Identity: <span className="text-stark-400">{identityId}</span>
          </div>
        )}

        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={type === 'daily' ? 'What happened today?' : 'Add a fact, preference, or important information...'}
          className="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-white text-sm resize-none h-32 mb-4"
        />

        {error && (
          <div className="text-red-400 text-sm mb-4">{error}</div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={isSubmitting || !content.trim()}>
            {isSubmitting ? 'Adding...' : 'Add Entry'}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

export default function MemoryBrowser() {
  const [files, setFiles] = useState<MemoryFile[]>([]);
  const [stats, setStats] = useState<MemoryStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // View state
  const [viewMode, setViewMode] = useState<ViewMode>('files');
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [loadingContent, setLoadingContent] = useState(false);

  // Filter state: "all" | "standard" | "safemode"
  const [modeFilter, setModeFilter] = useState<string>('all');
  const [selectedDate, setSelectedDate] = useState<string | null>(null);

  // Add entry modal
  const [addEntryType, setAddEntryType] = useState<'daily' | 'long_term' | null>(null);

  // Load files and stats
  const loadData = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const [filesRes, statsRes] = await Promise.all([getMemoryFiles(), getMemoryStats()]);

      if (filesRes.success) {
        setFiles(filesRes.files);
      } else {
        setError(filesRes.error || 'Failed to load files');
      }

      if (statsRes.success) {
        setStats(statsRes.stats);
      }
    } catch (err) {
      setError('Failed to load memory data');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  // Load file content when selected
  const loadFileContent = async (path: string) => {
    setLoadingContent(true);
    try {
      const response = await readMemoryFile(path);
      if (response.success && response.content !== undefined) {
        setFileContent(response.content);
        setSelectedFile(path);
      } else {
        setError(response.error || 'Failed to read file');
      }
    } catch {
      setError('Failed to load file');
    } finally {
      setLoadingContent(false);
    }
  };

  // Load daily log by date
  const loadDailyLogByDate = async (date: string) => {
    setLoadingContent(true);
    setSelectedDate(date);
    try {
      const response = await getDailyLog(date, modeFilter === 'safemode' ? 'safemode' : undefined);
      if (response.success) {
        setFileContent(response.content || '');
        setSelectedFile(response.path);
      } else {
        setFileContent('');
        setSelectedFile(`${date}.md`);
      }
    } catch {
      setFileContent('');
      setSelectedFile(`${date}.md`);
    } finally {
      setLoadingContent(false);
    }
  };

  // Handle reindex
  const handleReindex = async () => {
    try {
      const response = await reindexMemory();
      if (response.success) {
        await loadData();
      } else {
        setError(response.error || 'Reindex failed');
      }
    } catch {
      setError('Reindex request failed');
    }
  };

  // Filter files by mode
  const filteredFiles = useMemo(() => {
    if (modeFilter === 'all') return files;
    if (modeFilter === 'safemode') return files.filter((f) => f.identity_id === 'safemode');
    // "standard" = everything that's NOT safemode
    return files.filter((f) => f.identity_id !== 'safemode');
  }, [files, modeFilter]);

  // Group files by type
  const groupedFiles = useMemo(() => {
    const longTerm = filteredFiles.filter((f) => f.file_type === 'long_term');
    const dailyLogs = filteredFiles.filter((f) => f.file_type === 'daily_log');
    const other = filteredFiles.filter((f) => f.file_type === 'unknown');
    return { longTerm, dailyLogs, other };
  }, [filteredFiles]);

  if (isLoading && files.length === 0) {
    return (
      <div className="p-8 flex items-center justify-center">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading memories...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8">
      {/* Header */}
      <div className="mb-6">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h1 className="text-2xl font-bold text-white mb-1">Memory Browser</h1>
            <p className="text-slate-400 text-sm">
              {stats ? (
                <>
                  {stats.total_files} files | {stats.daily_log_count} daily logs | {stats.long_term_count} long-term
                  {stats.date_range && (
                    <span className="ml-2 text-slate-500">
                      ({stats.date_range.oldest} to {stats.date_range.newest})
                    </span>
                  )}
                </>
              ) : (
                'QMD Markdown-based memory system'
              )}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={handleReindex}
              className="flex items-center gap-2"
            >
              <RefreshCw className="w-4 h-4" />
              Reindex
            </Button>
          </div>
        </div>

        {/* View mode tabs */}
        <div className="flex items-center gap-4 border-b border-slate-700 pb-3">
          <div className="flex gap-1">
            <button
              onClick={() => setViewMode('files')}
              className={`px-3 py-1.5 rounded text-sm transition-colors ${
                viewMode === 'files'
                  ? 'bg-stark-500 text-white'
                  : 'text-slate-400 hover:text-white hover:bg-slate-700'
              }`}
            >
              <FolderOpen className="w-4 h-4 inline mr-1.5" />
              Memories
            </button>
            <button
              onClick={() => setViewMode('calendar')}
              className={`px-3 py-1.5 rounded text-sm transition-colors ${
                viewMode === 'calendar'
                  ? 'bg-stark-500 text-white'
                  : 'text-slate-400 hover:text-white hover:bg-slate-700'
              }`}
            >
              <Calendar className="w-4 h-4 inline mr-1.5" />
              Calendar
            </button>
            <button
              onClick={() => setViewMode('search')}
              className={`px-3 py-1.5 rounded text-sm transition-colors ${
                viewMode === 'search'
                  ? 'bg-stark-500 text-white'
                  : 'text-slate-400 hover:text-white hover:bg-slate-700'
              }`}
            >
              <Search className="w-4 h-4 inline mr-1.5" />
              Search
            </button>
          </div>

          {/* Mode filter */}
          <div className="flex items-center gap-2 ml-auto">
            <User className="w-4 h-4 text-slate-500" />
            <select
              value={modeFilter}
              onChange={(e) => setModeFilter(e.target.value)}
              className="bg-slate-800 border border-slate-600 rounded px-2 py-1 text-sm text-white"
            >
              <option value="all">All Memories</option>
              <option value="standard">Standard</option>
              <option value="safemode">Safe Mode</option>
            </select>
          </div>
        </div>
      </div>

      {error && (
        <div className="mb-6 bg-red-500/20 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">
            Dismiss
          </button>
        </div>
      )}

      {/* Main content area */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left panel: File list / Calendar / Search */}
        <div className="lg:col-span-1">
          {viewMode === 'files' && (
            <div className="space-y-4">
              {/* Long-term memories */}
              {groupedFiles.longTerm.length > 0 && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <Brain className="w-4 h-4 text-purple-400" />
                    <span className="text-sm font-medium text-purple-400">Long-term Memory</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setAddEntryType('long_term')}
                      className="ml-auto text-slate-400 hover:text-white"
                    >
                      <Plus className="w-3 h-3" />
                    </Button>
                  </div>
                  <div className="space-y-1">
                    {groupedFiles.longTerm.map((file) => (
                      <button
                        key={file.path}
                        onClick={() => loadFileContent(file.path)}
                        className={`w-full text-left px-3 py-2 rounded text-sm transition-colors ${
                          selectedFile === file.path
                            ? 'bg-purple-500/20 text-purple-300'
                            : 'text-slate-300 hover:bg-slate-700'
                        }`}
                      >
                        <div className="flex items-center gap-2">
                          <FileTypeIcon type={file.file_type} />
                          <span className="truncate">{file.name}</span>
                          <ModeBadge identityId={file.identity_id} />
                        </div>
                        {file.identity_id && file.identity_id !== 'safemode' && (
                          <span className="text-xs text-slate-500 ml-6">{identityLabel(file.identity_id)}</span>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {/* Daily logs */}
              {groupedFiles.dailyLogs.length > 0 && (
                <div>
                  <div className="flex items-center gap-2 mb-2">
                    <Calendar className="w-4 h-4 text-blue-400" />
                    <span className="text-sm font-medium text-blue-400">Daily Logs</span>
                    <span className="text-xs text-slate-500">({groupedFiles.dailyLogs.length})</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setAddEntryType('daily')}
                      className="ml-auto text-slate-400 hover:text-white"
                    >
                      <Plus className="w-3 h-3" />
                    </Button>
                  </div>
                  <div className="space-y-1 max-h-96 overflow-y-auto">
                    {groupedFiles.dailyLogs.map((file) => (
                      <button
                        key={file.path}
                        onClick={() => loadFileContent(file.path)}
                        className={`w-full text-left px-3 py-2 rounded text-sm transition-colors ${
                          selectedFile === file.path
                            ? 'bg-blue-500/20 text-blue-300'
                            : 'text-slate-300 hover:bg-slate-700'
                        }`}
                      >
                        <div className="flex items-center gap-2">
                          <FileTypeIcon type={file.file_type} />
                          <span>{file.date}</span>
                          <ModeBadge identityId={file.identity_id} />
                          <span className="text-xs text-slate-500 ml-auto">
                            {file.size > 1024
                              ? `${(file.size / 1024).toFixed(1)}KB`
                              : `${file.size}B`}
                          </span>
                        </div>
                        {file.identity_id && file.identity_id !== 'safemode' && (
                          <span className="text-xs text-slate-500 ml-6">{identityLabel(file.identity_id)}</span>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {filteredFiles.length === 0 && (
                <Card>
                  <CardContent className="text-center py-8">
                    <FileText className="w-10 h-10 text-slate-600 mx-auto mb-3" />
                    <p className="text-slate-400">No memory files yet</p>
                    <p className="text-slate-500 text-sm mt-1">
                      Memories will appear here as they are created
                    </p>
                  </CardContent>
                </Card>
              )}
            </div>
          )}

          {viewMode === 'calendar' && (
            <div className="space-y-4">
              <CalendarView
                files={filteredFiles}
                selectedDate={selectedDate}
                onSelectDate={loadDailyLogByDate}
              />
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setAddEntryType('daily')}
                className="w-full flex items-center justify-center gap-2"
              >
                <Plus className="w-4 h-4" />
                Add to Today's Log
              </Button>
            </div>
          )}

          {viewMode === 'search' && (
            <SearchView onSelectFile={loadFileContent} />
          )}

          {/* Stats summary */}
          {stats && (
            <Card className="mt-4">
              <CardContent className="py-3">
                <div className="flex items-center gap-2 text-sm text-slate-400 mb-2">
                  <BarChart3 className="w-4 h-4" />
                  <span>Memory Stats</span>
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs">
                  <div>
                    <span className="text-slate-500">Files:</span>{' '}
                    <span className="text-white">{stats.total_files}</span>
                  </div>
                  <div>
                    <span className="text-slate-500">Daily:</span>{' '}
                    <span className="text-blue-400">{stats.daily_log_count}</span>
                  </div>
                  <div>
                    <span className="text-slate-500">Long-term:</span>{' '}
                    <span className="text-purple-400">{stats.long_term_count}</span>
                  </div>
                  <div>
                    <span className="text-slate-500">Identities:</span>{' '}
                    <span className="text-white">{stats.identity_count}</span>
                  </div>
                </div>
              </CardContent>
            </Card>
          )}
        </div>

        {/* Right panel: Content viewer */}
        <div className="lg:col-span-2">
          <Card className="h-full min-h-[400px]">
            <CardContent>
              {loadingContent ? (
                <div className="flex items-center justify-center h-64">
                  <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
                </div>
              ) : selectedFile && fileContent !== null ? (
                <div>
                  {/* File header */}
                  <div className="flex items-center gap-3 mb-4 pb-3 border-b border-slate-700">
                    <FileTypeIcon
                      type={
                        selectedFile.includes('MEMORY.md') ? 'long_term' : 'daily_log'
                      }
                    />
                    <div>
                      <h2 className="text-lg font-medium text-white">{selectedFile}</h2>
                      {selectedDate && (
                        <span className="text-sm text-slate-400 flex items-center gap-1">
                          <Clock className="w-3 h-3" />
                          {new Date(selectedDate).toLocaleDateString('en-US', {
                            weekday: 'long',
                            year: 'numeric',
                            month: 'long',
                            day: 'numeric',
                          })}
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Content */}
                  {fileContent ? (
                    <div className="max-h-[600px] overflow-y-auto">
                      <MarkdownViewer content={fileContent} />
                    </div>
                  ) : (
                    <div className="text-center py-12 text-slate-500">
                      <FileText className="w-10 h-10 mx-auto mb-3 opacity-50" />
                      <p>No content yet</p>
                    </div>
                  )}
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center h-64 text-slate-500">
                  <FileText className="w-12 h-12 mb-3 opacity-50" />
                  <p>Select a file to view its contents</p>
                  <p className="text-sm mt-1">
                    Or use the calendar to browse daily logs
                  </p>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>

      {/* Add entry modal */}
      {addEntryType && (
        <AddEntryModal
          type={addEntryType}
          identityId={modeFilter === 'safemode' ? 'safemode' : null}
          onClose={() => setAddEntryType(null)}
          onSuccess={() => {
            loadData();
            // Reload current file if it's the same type
            if (selectedFile) {
              loadFileContent(selectedFile);
            }
          }}
        />
      )}
    </div>
  );
}
