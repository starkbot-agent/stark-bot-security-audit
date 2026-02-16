import { useState, useEffect, useCallback, useRef } from 'react';
import { CheckCircle, Circle, Loader2, ListChecks, X } from 'lucide-react';
import clsx from 'clsx';
import { useGateway } from '@/hooks/useGateway';
import { deletePlannerTask, getPlannerTasks } from '@/lib/api';
import type { PlannerTask, TaskQueueUpdateEvent, TaskStatusChangeEvent } from '@/types';

// Web channel ID - must match backend WEB_CHANNEL_ID
const WEB_CHANNEL_ID = 0;

// Helper to check if an event is for the current session
// This filters out events from other browser tabs/sessions
function isCurrentSessionEvent(data: unknown, currentDbSessionId: number | null): boolean {
  if (typeof data !== 'object' || data === null) return true;
  const event = data as { channel_id?: number; session_id?: number };

  // First check channel_id (must be web channel or undefined)
  if (event.channel_id !== undefined && event.channel_id !== WEB_CHANNEL_ID) {
    return false;
  }

  // If no session_id in event (legacy) or no current session, allow the event
  if (event.session_id === undefined || currentDbSessionId === null) {
    return true;
  }

  // Check if session_id matches current session
  return event.session_id === currentDbSessionId;
}

interface TaskQueueProgressProps {
  className?: string;
  dbSessionId?: number | null;
}

export default function TaskQueueProgress({ className, dbSessionId }: TaskQueueProgressProps) {
  const [tasks, setTasks] = useState<PlannerTask[]>([]);
  const [visible, setVisible] = useState(false);
  const [deletingTaskId, setDeletingTaskId] = useState<number | null>(null);
  const { on, off } = useGateway();
  const hasFetchedRef = useRef(false);

  // Fetch tasks on mount (for page refresh)
  useEffect(() => {
    if (hasFetchedRef.current) return;
    hasFetchedRef.current = true;

    const fetchTasks = async () => {
      try {
        const response = await getPlannerTasks();
        if (response.success && response.tasks.length > 0) {
          // Convert API response to PlannerTask format
          const plannerTasks: PlannerTask[] = response.tasks.map((t) => ({
            id: t.id,
            description: t.description,
            status: t.status as 'pending' | 'in_progress' | 'completed',
          }));
          setTasks(plannerTasks);
          setVisible(true);
          console.log('[TaskQueueProgress] Loaded tasks from API:', plannerTasks);
        }
      } catch (error) {
        console.error('[TaskQueueProgress] Failed to fetch tasks:', error);
      }
    };

    fetchTasks();
  }, []);

  // Handle task deletion
  const handleDeleteTask = useCallback(async (taskId: number, e: React.MouseEvent) => {
    e.stopPropagation();
    if (deletingTaskId !== null) return; // Already deleting

    setDeletingTaskId(taskId);
    try {
      const result = await deletePlannerTask(taskId);
      if (result.success) {
        // Optimistically remove the task from local state
        // The backend will also broadcast a task_queue_update event
        setTasks((prev) => prev.filter((t) => t.id !== taskId));
      } else {
        console.error('[TaskQueueProgress] Failed to delete task:', result.error);
      }
    } catch (error) {
      console.error('[TaskQueueProgress] Error deleting task:', error);
    } finally {
      setDeletingTaskId(null);
    }
  }, [deletingTaskId]);

  // Handle full task queue update
  const handleTaskQueueUpdate = useCallback((data: unknown) => {
    if (!isCurrentSessionEvent(data, dbSessionId ?? null)) return;

    const event = data as TaskQueueUpdateEvent;
    console.log('[TaskQueueProgress] Queue update:', event);

    if (event.tasks && event.tasks.length > 0) {
      setTasks(event.tasks);
      setVisible(true);
    } else {
      setTasks([]);
      setVisible(false);
    }
  }, [dbSessionId]);

  // Handle individual task status change
  const handleTaskStatusChange = useCallback((data: unknown) => {
    if (!isCurrentSessionEvent(data, dbSessionId ?? null)) return;

    const event = data as TaskStatusChangeEvent;
    console.log('[TaskQueueProgress] Status change:', event);

    setTasks((prev) =>
      prev.map((task) =>
        task.id === event.task_id
          ? { ...task, status: event.status, description: event.description }
          : task
      )
    );

    // Status is already reflected in the tasks array through the update above
  }, [dbSessionId]);

  // Handle session complete
  const handleSessionComplete = useCallback((data: unknown) => {
    if (!isCurrentSessionEvent(data, dbSessionId ?? null)) return;
    console.log('[TaskQueueProgress] Session complete');
    // Keep visible for a moment to show completion, then hide
    setTimeout(() => {
      setVisible(false);
      setTasks([]);
    }, 3000);
  }, [dbSessionId]);

  // Handle execution stopped (user clicked stop button)
  const handleExecutionStopped = useCallback((data: unknown) => {
    if (!isCurrentSessionEvent(data, dbSessionId ?? null)) return;
    console.log('[TaskQueueProgress] Execution stopped, clearing tasks');
    // Clear tasks immediately when execution is stopped
    setVisible(false);
    setTasks([]);
  }, [dbSessionId]);

  useEffect(() => {
    on('task.queue_update', handleTaskQueueUpdate);
    on('task.status_change', handleTaskStatusChange);
    on('session.complete', handleSessionComplete);
    on('execution.stopped', handleExecutionStopped);

    return () => {
      off('task.queue_update', handleTaskQueueUpdate);
      off('task.status_change', handleTaskStatusChange);
      off('session.complete', handleSessionComplete);
      off('execution.stopped', handleExecutionStopped);
    };
  }, [on, off, handleTaskQueueUpdate, handleTaskStatusChange, handleSessionComplete, handleExecutionStopped]);

  if (!visible || tasks.length === 0) {
    return null;
  }

  const completedCount = tasks.filter((t) => t.status === 'completed').length;
  const totalCount = tasks.length;
  const progressPercent = totalCount > 0 ? (completedCount / totalCount) * 100 : 0;

  const getStatusIcon = (task: PlannerTask) => {
    if (task.status === 'completed') {
      return <CheckCircle className="w-4 h-4 text-green-400" />;
    }
    if (task.status === 'in_progress') {
      return <Loader2 className="w-4 h-4 text-cyan-400 animate-spin" />;
    }
    return <Circle className="w-4 h-4 text-slate-500" />;
  };

  return (
    <div
      className={clsx(
        'bg-slate-800/80 backdrop-blur border border-slate-700 rounded-lg p-4 transition-opacity duration-300',
        visible ? 'opacity-100' : 'opacity-0',
        className
      )}
    >
      {/* Header with progress bar */}
      <div className="flex items-center gap-3 mb-3">
        <ListChecks className="w-5 h-5 text-stark-400" />
        <span className="text-sm font-medium text-white">
          Task Progress
        </span>
        <span className="text-xs text-slate-400">
          {completedCount}/{totalCount}
        </span>
        <div className="flex-1 bg-slate-700 rounded-full h-2 overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-stark-500 to-green-400 transition-all duration-500"
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      </div>

      {/* Task list */}
      <div className="space-y-2 max-h-[150px] overflow-y-auto">
        {tasks.map((task) => (
          <div
            key={task.id}
            className={clsx(
              'flex items-start gap-2 text-sm py-1 px-2 rounded group',
              task.status === 'in_progress' && 'bg-cyan-500/10 border border-cyan-500/30',
              task.status === 'completed' && 'opacity-60'
            )}
          >
            <div className="shrink-0 mt-0.5">
              {getStatusIcon(task)}
            </div>
            <div className="flex-1 min-w-0">
              <span
                className={clsx(
                  'block',
                  task.status === 'in_progress' && 'text-cyan-300 font-medium',
                  task.status === 'completed' && 'text-slate-400 line-through',
                  task.status === 'pending' && 'text-slate-300'
                )}
              >
                {task.id}. {task.description}
              </span>
            </div>
            {/* Delete button - always visible for non-completed tasks */}
            {task.status !== 'completed' && (
              <button
                onClick={(e) => handleDeleteTask(task.id, e)}
                disabled={deletingTaskId === task.id}
                className={clsx(
                  'shrink-0 p-1 rounded transition-colors',
                  'hover:bg-red-500/20',
                  'focus:outline-none focus:ring-1 focus:ring-red-500/50',
                  deletingTaskId === task.id && 'cursor-wait'
                )}
                title="Delete task"
              >
                {deletingTaskId === task.id ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin text-slate-400" />
                ) : (
                  <X className="w-3.5 h-3.5 text-slate-500 hover:text-red-400" />
                )}
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
