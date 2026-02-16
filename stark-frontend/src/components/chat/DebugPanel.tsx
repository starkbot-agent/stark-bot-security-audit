import { useState, useEffect, useCallback, useRef } from 'react';
import { ChevronDown, ChevronRight, DollarSign, Cpu, Clock, Globe, Terminal, Wrench, Brain, CheckCircle, XCircle, Loader2, Zap, Database, ListTodo, FileJson } from 'lucide-react';
import clsx from 'clsx';
import { useGateway } from '@/hooks/useGateway';
import type { ExecutionEvent, X402PaymentEvent } from '@/types';

interface DebugTask {
  id: string;
  parentId?: string;
  name: string;
  description?: string;
  activeForm?: string;
  taskType?: string;
  status: 'pending' | 'in_progress' | 'completed' | 'error';
  startTime?: number;
  endTime?: number;
  duration?: number;
  toolsCount?: number;
  tokensUsed?: number;
  linesRead?: number;
  children: DebugTask[];
  // New fields for richer info
  toolName?: string;
  toolParams?: Record<string, unknown>;
  result?: string;
  success?: boolean;
}

interface DebugPanelProps {
  className?: string;
}

// Extract tool name from description for better display
const extractToolName = (desc: string): string | undefined => {
  if (!desc) return undefined;

  // Try various patterns from the new descriptive format
  const patterns = [
    /^Reading\s+(.+)$/i,      // "Reading readme.md"
    /^Writing\s+(.+)$/i,      // "Writing config.json"
    /^Listing\s+(.+)$/i,      // "Listing /path"
    /^Patching\s+(.+)$/i,     // "Patching file.rs"
    /^Fetching\s+(.+)$/i,     // "Fetching github.com"
    /^Searching:\s+(.+)$/i,   // "Searching: query"
    /^Running:\s+(.+)$/i,     // "Running: git status"
    /^Using skill:\s+(.+)$/i, // "Using skill: weather"
    /^Agent:\s+(.+)$/i,       // "Agent: explore codebase"
    /^Using\s+(.+)$/i,        // "Using web_fetch" (fallback)
    /^Using tool:\s+(\w+)/i,  // Legacy: "Using tool: name"
  ];

  for (const pattern of patterns) {
    const match = desc.match(pattern);
    if (match) {
      return match[1].trim();
    }
  }

  return undefined;
};

// Get a short label for the tool badge based on description
const getToolBadgeLabel = (desc: string, toolName?: string): string => {
  if (!desc) return toolName || 'tool';

  const lowerDesc = desc.toLowerCase();

  if (lowerDesc.startsWith('reading')) return 'read';
  if (lowerDesc.startsWith('writing')) return 'write';
  if (lowerDesc.startsWith('listing')) return 'list';
  if (lowerDesc.startsWith('patching')) return 'patch';
  if (lowerDesc.startsWith('fetching')) return 'fetch';
  if (lowerDesc.startsWith('searching')) return 'search';
  if (lowerDesc.startsWith('running')) return 'exec';
  if (lowerDesc.startsWith('using skill')) return 'skill';
  if (lowerDesc.startsWith('agent')) return 'agent';
  if (lowerDesc.startsWith('storing memory')) return 'memory';
  if (lowerDesc.startsWith('recalling')) return 'recall';
  if (lowerDesc.startsWith('sending')) return 'send';

  return toolName || 'tool';
};

// Tool icons mapping
const getToolIcon = (toolName?: string, desc?: string) => {
  const text = (toolName || desc || '').toLowerCase();
  if (text.includes('fetch') || text.includes('web')) return <Globe className="w-3 h-3" />;
  if (text.includes('exec') || text.includes('shell') || text.includes('bash') || text.includes('running')) return <Terminal className="w-3 h-3" />;
  if (text.includes('skill')) return <Zap className="w-3 h-3" />;
  if (text.includes('read') || text.includes('write') || text.includes('file') || text.includes('list') || text.includes('patch')) return <Cpu className="w-3 h-3" />;
  if (text.includes('search')) return <Globe className="w-3 h-3" />;
  if (text.includes('agent')) return <Brain className="w-3 h-3" />;
  if (text.includes('memory') || text.includes('recall')) return <Brain className="w-3 h-3" />;
  return <Wrench className="w-3 h-3" />;
};

interface RegisterEntry {
  value: unknown;
  source: string;
  age_secs: number;
}

interface AgentTaskNote {
  content: string;
  timestamp: string;
}

interface AgentTask {
  id: string;
  subject: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  notes: AgentTaskNote[];
  result?: string;
  error?: string;
  created_at: string;
  updated_at: string;
}

interface AgentTasksState {
  mode: string;
  modeLabel: string;
  tasks: AgentTask[];
  stats: {
    total: number;
    pending: number;
    in_progress: number;
    completed: number;
    failed: number;
  };
}

interface AgentToolInfo {
  name: string;
  description: string;
  group: string;
}

interface AgentToolsetState {
  mode: string;
  subtype: string;
  tools: AgentToolInfo[];
  count: number;
  timestamp: string;
}

interface ContextBankItem {
  value: string;
  item_type: string;
  label?: string;
}

interface ContextBankState {
  items: ContextBankItem[];
  count: number;
  formatted?: string;
}

interface AgentContextMessage {
  role: string;
  content: string;
}

interface AgentContextTool {
  name: string;
  description: string;
  group: string;
}

interface AgentContextToolHistory {
  tool_name: string;
  tool_input: Record<string, unknown>;
  tool_result: string;
  tool_use_id: string;
}

interface AgentContextState {
  messages: AgentContextMessage[];
  messages_count: number;
  tools: AgentContextTool[];
  tools_count: number;
  tool_history: AgentContextToolHistory[];
  tool_history_count: number;
  timestamp: string;
}

export default function DebugPanel({ className }: DebugPanelProps) {
  const [executions, setExecutions] = useState<Map<string, DebugTask>>(new Map());
  const [payments, setPayments] = useState<X402PaymentEvent[]>([]);
  const [registers, setRegisters] = useState<Record<string, RegisterEntry>>({});
  const [contextBank, setContextBank] = useState<ContextBankState | null>(null);
  const [agentTasks, setAgentTasks] = useState<AgentTasksState | null>(null);
  const [agentToolset, setAgentToolset] = useState<AgentToolsetState | null>(null);
  const [agentContext, setAgentContext] = useState<AgentContextState | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [toolsCollapsed, setToolsCollapsed] = useState(true);
  const [activeTab, setActiveTab] = useState<'tasks' | 'payments' | 'registry' | 'agent' | 'context'>('tasks');
  const [, forceUpdate] = useState(0);
  const { on, off } = useGateway();

  // Store tool execution data to match with tasks
  const toolDataRef = useRef<Map<string, { params: Record<string, unknown>; result?: string; success?: boolean; duration?: number; content?: string }>>(new Map());

  // Force re-render every second to update elapsed times
  useEffect(() => {
    const interval = setInterval(() => {
      forceUpdate(n => n + 1);
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const updateExecution = useCallback((executionId: string, updater: (task: DebugTask) => DebugTask) => {
    setExecutions((prev) => {
      const newMap = new Map(prev);
      const execution = newMap.get(executionId);
      if (execution) {
        newMap.set(executionId, updater(execution));
      }
      return newMap;
    });
  }, []);

  const handleExecutionStarted = useCallback((data: unknown) => {
    const event = data as ExecutionEvent & {
      description?: string;
      active_form?: string;
    };
    const mode = (data as Record<string, unknown>).mode as string || 'execute';

    // Use description from event, fallback to generic text
    const description = event.description ||
      (mode === 'plan' ? 'Creating execution plan...' : 'Processing request...');
    const activeForm = event.active_form || description;
    const name = mode === 'plan' ? 'Planning' : 'Processing';

    const newExecution: DebugTask = {
      id: event.execution_id,
      name,
      description,
      activeForm,
      taskType: 'execution',
      status: 'in_progress',
      startTime: Date.now(),
      children: [],
    };

    // Clear previous executions and start fresh with the new one
    setExecutions(() => {
      const newMap = new Map<string, DebugTask>();
      newMap.set(event.execution_id, newExecution);
      return newMap;
    });
  }, []);

  const handleExecutionThinking = useCallback((data: unknown) => {
    const event = data as ExecutionEvent;
    updateExecution(event.execution_id, (execution) => ({
      ...execution,
      activeForm: event.active_form || (data as Record<string, unknown>).text as string || 'Thinking...',
    }));
  }, [updateExecution]);

  const handleTaskStarted = useCallback((data: unknown) => {
    const event = data as ExecutionEvent & {
      id?: string;
      type?: string;
      name?: string;
      description?: string;
      active_form?: string;
      parent_id?: string;
    };

    // Extract tool name and context from description
    const desc = event.description || event.name || '';
    const toolName = extractToolName(desc);

    const newTask: DebugTask = {
      id: event.id || event.task_id || crypto.randomUUID(),
      parentId: event.parent_task_id || event.parent_id,
      name: event.name || desc || 'Task',
      description: desc,
      taskType: event.type || (data as Record<string, unknown>).type as string,
      activeForm: event.active_form,
      status: 'in_progress',
      startTime: Date.now(),
      children: [],
      toolName,
    };

    // If no execution_id, try to find it from channel executions
    const execId = event.execution_id;
    if (!execId) {
      console.warn('Task started without execution_id:', event);
      return;
    }

    updateExecution(execId, (execution) => {
      const addToParent = (tasks: DebugTask[]): DebugTask[] => {
        return tasks.map((task) => {
          if (task.id === newTask.parentId) {
            return { ...task, children: [...task.children, newTask] };
          }
          return { ...task, children: addToParent(task.children) };
        });
      };

      if (!newTask.parentId || newTask.parentId === execution.id) {
        return { ...execution, children: [...execution.children, newTask] };
      }
      return { ...execution, children: addToParent(execution.children) };
    });
  }, [updateExecution]);

  // Listen for tool.execution to capture parameters
  const handleToolExecution = useCallback((data: unknown) => {
    const event = data as { tool_name: string; parameters: Record<string, unknown> };
    // Store params keyed by tool name (we'll match by timing/order)
    toolDataRef.current.set(`pending_${event.tool_name}`, { params: event.parameters });
  }, []);

  // Listen for tool.result to capture success/duration/content
  const handleToolResult = useCallback((data: unknown) => {
    const event = data as { tool_name: string; success: boolean; duration_ms: number; content: string };
    const key = `pending_${event.tool_name}`;
    const existing = toolDataRef.current.get(key);
    if (existing) {
      existing.success = event.success;
      existing.duration = event.duration_ms;
      existing.content = event.content;
    }
  }, []);

  const handleTaskUpdated = useCallback((data: unknown) => {
    const event = data as ExecutionEvent & { metrics?: { tool_uses?: number; tokens_used?: number; lines_read?: number; duration_ms?: number } };
    if (!event.task_id) return;

    const metrics = event.metrics || {};

    updateExecution(event.execution_id, (execution) => {
      const updateTask = (tasks: DebugTask[]): DebugTask[] => {
        return tasks.map((task) => {
          if (task.id === event.task_id) {
            return {
              ...task,
              toolsCount: metrics.tool_uses ?? event.tools_count ?? task.toolsCount,
              tokensUsed: metrics.tokens_used ?? event.tokens_used ?? task.tokensUsed,
              linesRead: metrics.lines_read ?? task.linesRead,
              activeForm: event.active_form ?? task.activeForm,
            };
          }
          return { ...task, children: updateTask(task.children) };
        });
      };

      if (execution.id === event.task_id) {
        return {
          ...execution,
          toolsCount: metrics.tool_uses ?? event.tools_count ?? execution.toolsCount,
          tokensUsed: metrics.tokens_used ?? event.tokens_used ?? execution.tokensUsed,
          linesRead: metrics.lines_read ?? execution.linesRead,
          activeForm: event.active_form ?? execution.activeForm,
        };
      }
      return { ...execution, children: updateTask(execution.children) };
    });
  }, [updateExecution]);

  const handleTaskCompleted = useCallback((data: unknown) => {
    const event = data as ExecutionEvent & {
      status?: string;
      metrics?: { tool_uses?: number; tokens_used?: number; lines_read?: number; duration_ms?: number }
    };
    if (!event.task_id) return;

    const metrics = event.metrics || {};
    const statusStr = event.status || 'completed';
    const isError = statusStr.toLowerCase().includes('error');

    updateExecution(event.execution_id, (execution) => {
      const completeTask = (tasks: DebugTask[]): DebugTask[] => {
        return tasks.map((task) => {
          if (task.id === event.task_id) {
            return {
              ...task,
              status: isError ? 'error' : 'completed',
              success: !isError,
              result: statusStr !== 'completed' ? statusStr : undefined,
              endTime: Date.now(),
              duration: metrics.duration_ms ?? event.duration_ms ?? (Date.now() - (task.startTime || Date.now())),
              toolsCount: metrics.tool_uses ?? task.toolsCount,
              tokensUsed: metrics.tokens_used ?? task.tokensUsed,
              linesRead: metrics.lines_read ?? task.linesRead,
            };
          }
          return { ...task, children: completeTask(task.children) };
        });
      };

      return { ...execution, children: completeTask(execution.children) };
    });
  }, [updateExecution]);

  const handleExecutionCompleted = useCallback((data: unknown) => {
    const event = data as ExecutionEvent & { metrics?: { tool_uses?: number; tokens_used?: number; duration_ms?: number } };
    const metrics = event.metrics || {};

    // Helper to recursively complete all in-progress child tasks
    const completeAllChildren = (tasks: DebugTask[]): DebugTask[] => {
      return tasks.map((task) => {
        const updatedChildren = completeAllChildren(task.children);
        if (task.status === 'in_progress') {
          return {
            ...task,
            status: 'completed' as const,
            endTime: Date.now(),
            duration: Date.now() - (task.startTime || Date.now()),
            children: updatedChildren,
          };
        }
        return { ...task, children: updatedChildren };
      });
    };

    updateExecution(event.execution_id, (execution) => ({
      ...execution,
      status: 'completed',
      endTime: Date.now(),
      duration: metrics.duration_ms ?? event.duration_ms ?? (Date.now() - (execution.startTime || Date.now())),
      toolsCount: metrics.tool_uses ?? execution.toolsCount,
      tokensUsed: metrics.tokens_used ?? execution.tokensUsed,
      children: completeAllChildren(execution.children),
    }));
  }, [updateExecution]);

  const handleX402Payment = useCallback((data: unknown) => {
    const event = data as X402PaymentEvent;
    setPayments((prev) => [...prev, event]);
  }, []);

  const handleRegisterUpdate = useCallback((data: unknown) => {
    const event = data as { registers: Record<string, RegisterEntry> };
    setRegisters(event.registers || {});
  }, []);

  const handleContextBankUpdate = useCallback((data: unknown) => {
    const event = data as { context_bank: ContextBankState };
    setContextBank(event.context_bank || null);
  }, []);

  const handleAgentTasksUpdate = useCallback((data: unknown) => {
    const event = data as {
      mode: string;
      mode_label: string;
      tasks: AgentTask[];
      stats: AgentTasksState['stats'];
    };
    setAgentTasks({
      mode: event.mode,
      modeLabel: event.mode_label,
      tasks: event.tasks || [],
      stats: event.stats || { total: 0, pending: 0, in_progress: 0, completed: 0, failed: 0 },
    });
  }, []);

  const handleAgentToolsetUpdate = useCallback((data: unknown) => {
    const event = data as {
      mode: string;
      subtype: string;
      tools: AgentToolInfo[];
      count: number;
      timestamp: string;
    };
    setAgentToolset({
      mode: event.mode,
      subtype: event.subtype,
      tools: event.tools || [],
      count: event.count || 0,
      timestamp: event.timestamp,
    });
  }, []);

  const handleAgentContextUpdate = useCallback((data: unknown) => {
    const event = data as AgentContextState;
    setAgentContext({
      messages: event.messages || [],
      messages_count: event.messages_count || 0,
      tools: event.tools || [],
      tools_count: event.tools_count || 0,
      tool_history: event.tool_history || [],
      tool_history_count: event.tool_history_count || 0,
      timestamp: event.timestamp,
    });
  }, []);

  useEffect(() => {
    on('execution.started', handleExecutionStarted);
    on('execution.thinking', handleExecutionThinking);
    on('execution.task_started', handleTaskStarted);
    on('execution.task_updated', handleTaskUpdated);
    on('execution.task_completed', handleTaskCompleted);
    on('execution.completed', handleExecutionCompleted);
    on('tool.execution', handleToolExecution);
    on('tool.result', handleToolResult);
    on('x402.payment', handleX402Payment);
    on('register.update', handleRegisterUpdate);
    on('context_bank.update', handleContextBankUpdate);
    on('agent.tasks_update', handleAgentTasksUpdate);
    on('agent.toolset_update', handleAgentToolsetUpdate);
    on('agent.context_update', handleAgentContextUpdate);

    return () => {
      off('execution.started', handleExecutionStarted);
      off('execution.thinking', handleExecutionThinking);
      off('execution.task_started', handleTaskStarted);
      off('execution.task_updated', handleTaskUpdated);
      off('execution.task_completed', handleTaskCompleted);
      off('execution.completed', handleExecutionCompleted);
      off('tool.execution', handleToolExecution);
      off('tool.result', handleToolResult);
      off('x402.payment', handleX402Payment);
      off('register.update', handleRegisterUpdate);
      off('context_bank.update', handleContextBankUpdate);
      off('agent.tasks_update', handleAgentTasksUpdate);
      off('agent.toolset_update', handleAgentToolsetUpdate);
      off('agent.context_update', handleAgentContextUpdate);
    };
  }, [on, off, handleExecutionStarted, handleExecutionThinking, handleTaskStarted, handleTaskUpdated, handleTaskCompleted, handleExecutionCompleted, handleToolExecution, handleToolResult, handleX402Payment, handleRegisterUpdate, handleContextBankUpdate, handleAgentTasksUpdate, handleAgentToolsetUpdate, handleAgentContextUpdate]);

  const toggleCollapse = (taskId: string) => {
    setCollapsed((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(taskId)) {
        newSet.delete(taskId);
      } else {
        newSet.add(taskId);
      }
      return newSet;
    });
  };

  const formatDuration = (ms?: number): string => {
    if (!ms) return '';
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
  };

  const formatElapsed = (startTime?: number): string => {
    if (!startTime) return '';
    const elapsed = Date.now() - startTime;
    return formatDuration(elapsed);
  };

  const formatTimestamp = (ts: string): string => {
    const date = new Date(ts);
    return date.toLocaleTimeString();
  };

  const formatTime = (timestamp?: number): string => {
    if (!timestamp) return '';
    return new Date(timestamp).toLocaleTimeString('en-US', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  };

  const totalPayments = payments.reduce((sum, p) => {
    const amount = parseFloat(p.amount_formatted || '0');
    return sum + amount;
  }, 0);

  const renderTask = (task: DebugTask, depth: number = 0): JSX.Element => {
    const hasChildren = task.children.length > 0;
    const isCollapsed = collapsed.has(task.id);
    const isInProgress = task.status === 'in_progress';

    // Status indicators
    const StatusIcon = () => {
      switch (task.status) {
        case 'pending':
          return <span className="text-slate-500">○</span>;
        case 'in_progress':
          return <Loader2 className="w-3.5 h-3.5 text-cyan-400 animate-spin" />;
        case 'completed':
          return <CheckCircle className="w-3.5 h-3.5 text-green-400" />;
        case 'error':
          return <XCircle className="w-3.5 h-3.5 text-red-400" />;
        default:
          return null;
      }
    };

    // Get icon based on both toolName and description for better matching
    const toolIcon = getToolIcon(task.toolName, task.description);

    const typeConfig: Record<string, { color: string; bg: string; icon: JSX.Element }> = {
      tool: { color: 'text-purple-400', bg: 'bg-purple-500/20', icon: toolIcon },
      thinking: { color: 'text-yellow-400', bg: 'bg-yellow-500/20', icon: <Brain className="w-3 h-3" /> },
      agent: { color: 'text-blue-400', bg: 'bg-blue-500/20', icon: <Cpu className="w-3 h-3" /> },
      execution: { color: 'text-cyan-400', bg: 'bg-cyan-500/20', icon: <Zap className="w-3 h-3" /> },
      plan: { color: 'text-orange-400', bg: 'bg-orange-500/20', icon: <Brain className="w-3 h-3" /> },
    };

    const config = typeConfig[task.taskType || ''] || { color: 'text-slate-400', bg: 'bg-slate-500/20', icon: <Cpu className="w-3 h-3" /> };

    // Get short badge label and display text
    const badgeLabel = getToolBadgeLabel(task.description || '', task.toolName);
    const taskText = isInProgress && task.activeForm
      ? task.activeForm
      : task.description || task.name;

    return (
      <div key={task.id} className={clsx('border-l-2', task.status === 'error' ? 'border-red-500/50' : 'border-slate-700')}>
        <div
          className={clsx(
            'py-2 px-3 hover:bg-slate-800/50 text-sm transition-colors',
            isInProgress && 'bg-slate-800/30'
          )}
        >
          {/* Header row */}
          <div className="flex items-center gap-2">
            {/* Collapse toggle */}
            {hasChildren ? (
              <button
                onClick={() => toggleCollapse(task.id)}
                className="p-0.5 hover:bg-slate-700 rounded shrink-0"
              >
                {isCollapsed ? (
                  <ChevronRight className="w-3 h-3 text-slate-500" />
                ) : (
                  <ChevronDown className="w-3 h-3 text-slate-500" />
                )}
              </button>
            ) : (
              <div className="w-4 shrink-0" />
            )}

            {/* Status icon */}
            <div className="shrink-0">
              <StatusIcon />
            </div>

            {/* Task type badge with icon */}
            {task.taskType && (
              <span className={clsx(
                'flex items-center gap-1 text-xs px-1.5 py-0.5 rounded shrink-0',
                config.color,
                config.bg
              )}>
                {config.icon}
                <span>{task.taskType === 'tool' ? badgeLabel : task.taskType}</span>
              </span>
            )}

            {/* Timestamp */}
            <span className="text-[10px] text-slate-600 shrink-0">
              {formatTime(task.startTime)}
            </span>

            {/* Metrics on the right */}
            <div className="flex items-center gap-3 text-xs text-slate-500 ml-auto shrink-0">
              {/* Duration or elapsed time */}
              {(task.duration || isInProgress) && (
                <span className={clsx(
                  'flex items-center gap-1',
                  isInProgress && 'text-cyan-400'
                )}>
                  <Clock className="w-3 h-3" />
                  {task.duration ? formatDuration(task.duration) : formatElapsed(task.startTime)}
                </span>
              )}

              {/* Tool count */}
              {task.toolsCount !== undefined && task.toolsCount > 0 && (
                <span className="flex items-center gap-1" title="Tool calls">
                  <Wrench className="w-3 h-3" />
                  {task.toolsCount}
                </span>
              )}

              {/* Lines read */}
              {task.linesRead !== undefined && task.linesRead > 0 && (
                <span className="text-slate-600" title="Lines read">
                  {task.linesRead} lines
                </span>
              )}

              {/* Tokens */}
              {task.tokensUsed !== undefined && task.tokensUsed > 0 && (
                <span title="Tokens used">
                  {task.tokensUsed >= 1000
                    ? `${(task.tokensUsed / 1000).toFixed(1)}k tok`
                    : `${task.tokensUsed} tok`}
                </span>
              )}
            </div>
          </div>

          {/* Task description */}
          <div
            className={clsx(
              'ml-6 mt-1 font-mono text-xs',
              task.status === 'completed' && 'text-slate-400',
              task.status === 'error' && 'text-red-400',
              isInProgress && 'text-cyan-300',
              task.status === 'pending' && 'text-slate-300'
            )}
          >
            {taskText}
          </div>

          {/* Error/result message if present */}
          {task.result && task.status === 'error' && (
            <div className="ml-6 mt-1 text-xs text-red-400/80 bg-red-500/10 px-2 py-1 rounded font-mono">
              {task.result}
            </div>
          )}

          {/* Children count indicator when collapsed */}
          {hasChildren && isCollapsed && (
            <div className="ml-6 mt-1 text-[10px] text-slate-600">
              {task.children.length} subtask{task.children.length !== 1 ? 's' : ''} hidden
            </div>
          )}
        </div>

        {/* Children */}
        {hasChildren && !isCollapsed && (
          <div className="ml-3">
            {task.children.map((child) => renderTask(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  // Calculate stats
  const allTasks = Array.from(executions.values());
  const completedCount = allTasks.filter(t => t.status === 'completed').length;
  const errorCount = allTasks.filter(t => t.status === 'error').length;
  const inProgressCount = allTasks.filter(t => t.status === 'in_progress').length;

  return (
    <div className={clsx(
      'bg-slate-900 border border-slate-700 rounded-lg overflow-hidden',
      className
    )}>
      {/* Tab headers with stats */}
      <div className="flex border-b border-slate-700 sticky top-0 bg-slate-900 z-10">
        <button
          onClick={() => setActiveTab('tasks')}
          className={clsx(
            'flex-1 px-4 py-2 text-sm font-medium transition-colors',
            activeTab === 'tasks'
              ? 'bg-slate-800 text-white border-b-2 border-cyan-500'
              : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
          )}
        >
          <Cpu className="w-4 h-4 inline mr-2" />
          Tasks
          {executions.size > 0 && (
            <span className="ml-2 text-xs">
              {inProgressCount > 0 && <span className="text-cyan-400">{inProgressCount} running</span>}
              {completedCount > 0 && <span className="text-green-400 ml-1">✓{completedCount}</span>}
              {errorCount > 0 && <span className="text-red-400 ml-1">✗{errorCount}</span>}
            </span>
          )}
        </button>
        <button
          onClick={() => setActiveTab('payments')}
          className={clsx(
            'flex-1 px-4 py-2 text-sm font-medium transition-colors',
            activeTab === 'payments'
              ? 'bg-slate-800 text-white border-b-2 border-green-500'
              : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
          )}
        >
          <DollarSign className="w-4 h-4 inline mr-2" />
          x402
          {payments.length > 0 && (
            <span className="ml-2 text-xs text-green-400">
              ${totalPayments.toFixed(4)}
            </span>
          )}
        </button>
        <button
          onClick={() => setActiveTab('registry')}
          className={clsx(
            'flex-1 px-4 py-2 text-sm font-medium transition-colors',
            activeTab === 'registry'
              ? 'bg-slate-800 text-white border-b-2 border-purple-500'
              : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
          )}
        >
          <Database className="w-4 h-4 inline mr-2" />
          Registry
          {Object.keys(registers).length > 0 && (
            <span className="ml-2 text-xs text-purple-400">
              {Object.keys(registers).length}
            </span>
          )}
        </button>
        <button
          onClick={() => setActiveTab('agent')}
          className={clsx(
            'flex-1 px-4 py-2 text-sm font-medium transition-colors',
            activeTab === 'agent'
              ? 'bg-slate-800 text-white border-b-2 border-orange-500'
              : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
          )}
        >
          <ListTodo className="w-4 h-4 inline mr-2" />
          Agent
          <span className="ml-2 text-xs">
            {agentToolset && agentToolset.count > 0 && (
              <span className="text-purple-400">{agentToolset.count} tools</span>
            )}
            {agentTasks && agentTasks.stats.total > 0 && (
              <>
                {agentTasks.stats.in_progress > 0 && <span className="text-cyan-400 ml-1">{agentTasks.stats.in_progress}</span>}
                {agentTasks.stats.completed > 0 && <span className="text-green-400 ml-1">✓{agentTasks.stats.completed}</span>}
                {agentTasks.stats.failed > 0 && <span className="text-red-400 ml-1">✗{agentTasks.stats.failed}</span>}
              </>
            )}
          </span>
        </button>
        <button
          onClick={() => setActiveTab('context')}
          className={clsx(
            'flex-1 px-4 py-2 text-sm font-medium transition-colors',
            activeTab === 'context'
              ? 'bg-slate-800 text-white border-b-2 border-pink-500'
              : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
          )}
        >
          <FileJson className="w-4 h-4 inline mr-2" />
          Context
          {agentContext && (
            <span className="ml-2 text-xs text-pink-400">
              {agentContext.messages_count} msgs
            </span>
          )}
        </button>
      </div>

      {/* Tab content */}
      <div className="max-h-[400px] overflow-y-auto overflow-x-hidden">
        {activeTab === 'tasks' && (
          <div className="p-2">
            {executions.size === 0 ? (
              <div className="text-center text-slate-500 py-8">
                <Cpu className="w-8 h-8 mx-auto mb-2 opacity-50" />
                <p>No executions yet</p>
                <p className="text-xs mt-1">Send a message to see activity</p>
              </div>
            ) : (
              Array.from(executions.values()).reverse().map((execution) => (
                <div key={execution.id} className="mb-3 last:mb-0">
                  {renderTask(execution)}
                </div>
              ))
            )}
          </div>
        )}

        {activeTab === 'payments' && (
          <div className="p-2">
            {payments.length > 0 && (
              <div className="mb-4 p-3 bg-gradient-to-r from-green-500/10 to-emerald-500/10 rounded-lg border border-green-500/20">
                <div className="text-xs text-slate-400 uppercase tracking-wide">Total Spent</div>
                <div className="text-2xl font-bold text-green-400">
                  ${totalPayments.toFixed(6)}
                  <span className="text-sm font-normal text-slate-500 ml-2">USDC</span>
                </div>
                <div className="text-xs text-slate-500 mt-1">
                  {payments.length} transaction{payments.length !== 1 ? 's' : ''}
                </div>
              </div>
            )}

            {payments.length === 0 ? (
              <div className="text-center text-slate-500 py-8">
                <DollarSign className="w-8 h-8 mx-auto mb-2 opacity-50" />
                <p>No x402 payments yet</p>
                <p className="text-xs mt-1">Payments appear when using pay-per-use AI</p>
              </div>
            ) : (
              <div className="space-y-2">
                {payments.slice().reverse().map((payment, idx) => (
                  <div
                    key={idx}
                    className="p-3 bg-slate-800 rounded-lg border border-slate-700 hover:border-slate-600 transition-colors"
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <DollarSign className="w-4 h-4 text-green-400" />
                        <span className="text-lg font-semibold text-green-400">
                          ${payment.amount_formatted}
                        </span>
                        <span className="text-xs px-1.5 py-0.5 bg-blue-500/20 text-blue-400 rounded">
                          {payment.asset}
                        </span>
                      </div>
                      <span className="text-xs text-slate-500">
                        {formatTimestamp(payment.timestamp)}
                      </span>
                    </div>

                    <div className="text-xs text-slate-400 space-y-1 font-mono" style={{ wordBreak: 'break-all' }}>
                      <div className="flex">
                        <span className="text-slate-500 w-16 shrink-0 font-sans">To:</span>
                        <span className="text-slate-400">{payment.pay_to}</span>
                      </div>
                      {payment.resource && (
                        <div className="flex">
                          <span className="text-slate-500 w-16 shrink-0 font-sans">Resource:</span>
                          <span className="text-slate-400">{payment.resource}</span>
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === 'registry' && (
          <div className="p-2 space-y-4">
            {/* Context Bank Section */}
            {contextBank && contextBank.items.length > 0 && (
              <div className="p-3 bg-gradient-to-r from-amber-500/10 to-orange-500/10 rounded-lg border border-amber-500/20">
                <div className="flex items-center gap-2 mb-2">
                  <span className="text-amber-400 text-sm font-semibold">Context Bank</span>
                  <span className="text-xs text-slate-500">
                    {contextBank.count} term{contextBank.count !== 1 ? 's' : ''} detected
                  </span>
                </div>
                <div className="flex flex-wrap gap-2">
                  {contextBank.items.map((item, idx) => (
                    <div
                      key={idx}
                      className="px-2 py-1 bg-slate-800 rounded text-xs font-mono flex items-center gap-1"
                    >
                      <span className={
                        item.item_type === 'eth_address'
                          ? 'text-cyan-400'
                          : item.item_type === 'network'
                          ? 'text-green-400'
                          : 'text-amber-400'
                      }>
                        {item.item_type === 'eth_address'
                          ? `${item.value.slice(0, 6)}...${item.value.slice(-4)}`
                          : item.value
                        }
                      </span>
                      {item.label && (
                        <span className="text-slate-500">({item.label})</span>
                      )}
                      <span className="text-slate-600 text-[10px]">
                        {item.item_type === 'eth_address' ? '📍' : item.item_type === 'network' ? '🌐' : '🪙'}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Registers Section */}
            {Object.keys(registers).length === 0 && (!contextBank || contextBank.items.length === 0) ? (
              <div className="text-center text-slate-500 py-8">
                <Database className="w-8 h-8 mx-auto mb-2 opacity-50" />
                <p>No registers set</p>
                <p className="text-xs mt-1">Registers store data passed between tools</p>
              </div>
            ) : Object.keys(registers).length > 0 && (
              <div className="space-y-2">
                {Object.entries(registers).map(([key, entry]) => (
                  <div
                    key={key}
                    className="p-3 bg-slate-800 rounded-lg border border-slate-700 hover:border-slate-600 transition-colors"
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <Database className="w-4 h-4 text-purple-400" />
                        <span className="text-sm font-semibold text-purple-400 font-mono">
                          {key}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 text-xs text-slate-500">
                        <span className="px-1.5 py-0.5 bg-slate-700 rounded">
                          {entry.source}
                        </span>
                        {entry.age_secs > 0 && (
                          <span>{entry.age_secs}s ago</span>
                        )}
                      </div>
                    </div>
                    <pre className="text-xs text-slate-300 bg-slate-900 p-2 rounded overflow-x-auto font-mono">
                      {typeof entry.value === 'object'
                        ? JSON.stringify(entry.value, null, 2)
                        : String(entry.value)}
                    </pre>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === 'agent' && (
          <div className="p-2">
            {/* Toolset section - always show if we have toolset data */}
            {agentToolset && agentToolset.tools.length > 0 && (
              <div className="mb-4">
                <button
                  onClick={() => setToolsCollapsed(!toolsCollapsed)}
                  className="w-full p-3 bg-gradient-to-r from-purple-500/10 to-indigo-500/10 rounded-lg border border-purple-500/20 hover:border-purple-500/40 transition-colors"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Wrench className="w-4 h-4 text-purple-400" />
                      <span className="text-sm font-semibold text-purple-400">
                        Available Tools
                      </span>
                      <span className="text-xs px-1.5 py-0.5 bg-purple-500/20 text-purple-300 rounded">
                        {agentToolset.count}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-slate-500">
                        {agentToolset.mode} · {agentToolset.subtype}
                      </span>
                      {toolsCollapsed ? (
                        <ChevronRight className="w-4 h-4 text-slate-500" />
                      ) : (
                        <ChevronDown className="w-4 h-4 text-slate-500" />
                      )}
                    </div>
                  </div>
                </button>
                {!toolsCollapsed && (
                  <div className="mt-2 max-h-48 overflow-y-auto space-y-1 px-1">
                    {agentToolset.tools.map((tool, idx) => (
                      <div
                        key={idx}
                        className="p-2 bg-slate-800/50 rounded border border-slate-700/50 hover:border-slate-600/50 transition-colors"
                      >
                        <div className="flex items-center gap-2 mb-1">
                          <span className="text-xs font-mono text-purple-300">{tool.name}</span>
                          <span className="text-[10px] px-1 py-0.5 bg-slate-700/50 text-slate-400 rounded">
                            {tool.group}
                          </span>
                        </div>
                        <div className="text-[11px] text-slate-500 line-clamp-2">
                          {tool.description}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {!agentTasks || agentTasks.tasks.length === 0 ? (
              !agentToolset || agentToolset.tools.length === 0 ? (
                <div className="text-center text-slate-500 py-8">
                  <ListTodo className="w-8 h-8 mx-auto mb-2 opacity-50" />
                  <p>No agent data</p>
                  <p className="text-xs mt-1">Agent info appears during execution</p>
                </div>
              ) : null
            ) : (
              <>
                {/* Mode indicator and stats */}
                <div className="mb-4 p-3 bg-gradient-to-r from-orange-500/10 to-amber-500/10 rounded-lg border border-orange-500/20">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <Brain className="w-4 h-4 text-orange-400" />
                      <span className="text-sm font-semibold text-orange-400">
                        {agentTasks.modeLabel}
                      </span>
                      <span className="text-xs px-1.5 py-0.5 bg-orange-500/20 text-orange-300 rounded uppercase">
                        {agentTasks.mode}
                      </span>
                    </div>
                  </div>
                  <div className="text-xs text-slate-400">
                    {agentTasks.stats.total} task{agentTasks.stats.total !== 1 ? 's' : ''} · {' '}
                    {agentTasks.stats.pending > 0 && <span className="text-slate-300">{agentTasks.stats.pending} pending</span>}
                    {agentTasks.stats.in_progress > 0 && <span className="text-cyan-400 ml-2">{agentTasks.stats.in_progress} in progress</span>}
                    {agentTasks.stats.completed > 0 && <span className="text-green-400 ml-2">{agentTasks.stats.completed} done</span>}
                    {agentTasks.stats.failed > 0 && <span className="text-red-400 ml-2">{agentTasks.stats.failed} failed</span>}
                  </div>
                </div>

                {/* Task list */}
                <div className="space-y-2">
                  {agentTasks.tasks.map((task) => {
                    const StatusIcon = () => {
                      switch (task.status) {
                        case 'pending':
                          return <span className="text-slate-500">⏳</span>;
                        case 'in_progress':
                          return <Loader2 className="w-4 h-4 text-cyan-400 animate-spin" />;
                        case 'completed':
                          return <CheckCircle className="w-4 h-4 text-green-400" />;
                        case 'failed':
                          return <XCircle className="w-4 h-4 text-red-400" />;
                        default:
                          return null;
                      }
                    };

                    return (
                      <div
                        key={task.id}
                        className={clsx(
                          'p-3 bg-slate-800 rounded-lg border transition-colors',
                          task.status === 'in_progress' && 'border-cyan-500/50 bg-cyan-500/5',
                          task.status === 'completed' && 'border-green-500/30',
                          task.status === 'failed' && 'border-red-500/30',
                          task.status === 'pending' && 'border-slate-700'
                        )}
                      >
                        <div className="flex items-start gap-2">
                          <div className="mt-0.5 shrink-0">
                            <StatusIcon />
                          </div>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2 mb-1">
                              <span className={clsx(
                                'text-sm font-medium',
                                task.status === 'completed' && 'text-slate-400',
                                task.status === 'failed' && 'text-red-400',
                                task.status === 'in_progress' && 'text-cyan-300',
                                task.status === 'pending' && 'text-slate-300'
                              )}>
                                {task.subject}
                              </span>
                              <span className="text-[10px] text-slate-600 font-mono">
                                [{task.id.slice(0, 8)}]
                              </span>
                            </div>

                            {/* Result or error */}
                            {task.result && (
                              <div className="text-xs text-green-400/80 bg-green-500/10 px-2 py-1 rounded mb-2">
                                → {task.result}
                              </div>
                            )}
                            {task.error && (
                              <div className="text-xs text-red-400/80 bg-red-500/10 px-2 py-1 rounded mb-2">
                                ✗ {task.error}
                              </div>
                            )}

                            {/* Notes */}
                            {task.notes.length > 0 && (
                              <div className="mt-2 space-y-1">
                                {task.notes.map((note, idx) => (
                                  <div key={idx} className="text-xs text-slate-500 flex gap-2">
                                    <span className="text-slate-600 shrink-0">•</span>
                                    <span>{note.content}</span>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </>
            )}
          </div>
        )}

        {activeTab === 'context' && (
          <div className="p-2">
            {!agentContext ? (
              <div className="text-center text-slate-500 py-8">
                <FileJson className="w-8 h-8 mx-auto mb-2 opacity-50" />
                <p>No context yet</p>
                <p className="text-xs mt-1">Send a message to see the AI context</p>
              </div>
            ) : (
              <div className="space-y-4">
                {/* Timestamp */}
                <div className="text-xs text-slate-500 text-right">
                  Last updated: {formatTimestamp(agentContext.timestamp)}
                </div>

                {/* Summary Stats */}
                <div className="grid grid-cols-3 gap-2 text-center">
                  <div className="p-2 bg-slate-800 rounded">
                    <div className="text-lg font-bold text-pink-400">{agentContext.messages_count}</div>
                    <div className="text-xs text-slate-500">Messages</div>
                  </div>
                  <div className="p-2 bg-slate-800 rounded">
                    <div className="text-lg font-bold text-purple-400">{agentContext.tools_count}</div>
                    <div className="text-xs text-slate-500">Tools</div>
                  </div>
                  <div className="p-2 bg-slate-800 rounded">
                    <div className="text-lg font-bold text-cyan-400">{agentContext.tool_history_count}</div>
                    <div className="text-xs text-slate-500">History</div>
                  </div>
                </div>

                {/* Full JSON Context */}
                <div className="p-3 bg-slate-800 rounded-lg border border-slate-700">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-semibold text-pink-400">Full Context (JSON)</span>
                    <button
                      onClick={() => {
                        navigator.clipboard.writeText(JSON.stringify(agentContext, null, 2));
                      }}
                      className="text-xs px-2 py-1 bg-slate-700 hover:bg-slate-600 rounded text-slate-300 transition-colors"
                    >
                      Copy
                    </button>
                  </div>
                  <pre className="text-xs text-slate-300 bg-slate-900 p-3 rounded overflow-auto max-h-[500px] font-mono whitespace-pre-wrap">
                    {JSON.stringify(agentContext, null, 2)}
                  </pre>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
