import { useState, useEffect, useCallback, DragEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { Plus, Trash2, GripVertical, ExternalLink } from 'lucide-react';
import { useGateway } from '@/hooks/useGateway';
import {
  KanbanItem,
  getKanbanItems,
  createKanbanItem,
  updateKanbanItem,
  deleteKanbanItem,
} from '@/lib/api';
import Modal from '@/components/ui/Modal';
import Button from '@/components/ui/Button';
import Input from '@/components/ui/Input';

type KanbanStatus = 'ready' | 'in_progress' | 'complete';

const COLUMNS: { status: KanbanStatus; label: string; color: string; bg: string; border: string }[] = [
  { status: 'ready', label: 'Ready', color: 'text-blue-400', bg: 'bg-blue-500/10', border: 'border-blue-500/30' },
  { status: 'in_progress', label: 'In Progress', color: 'text-yellow-400', bg: 'bg-yellow-500/10', border: 'border-yellow-500/30' },
  { status: 'complete', label: 'Complete', color: 'text-green-400', bg: 'bg-green-500/10', border: 'border-green-500/30' },
];

const PRIORITY_LABELS: Record<number, { label: string; class: string }> = {
  0: { label: 'Normal', class: 'bg-slate-600 text-slate-300' },
  1: { label: 'High', class: 'bg-orange-600/80 text-orange-100' },
  2: { label: 'Urgent', class: 'bg-red-600/80 text-red-100' },
};

export default function KanbanBoard() {
  const navigate = useNavigate();
  const { gateway } = useGateway();
  const [items, setItems] = useState<KanbanItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Create modal
  const [createOpen, setCreateOpen] = useState(false);
  const [createTitle, setCreateTitle] = useState('');
  const [createDesc, setCreateDesc] = useState('');
  const [createPriority, setCreatePriority] = useState(0);
  const [creating, setCreating] = useState(false);

  // Detail modal
  const [detailItem, setDetailItem] = useState<KanbanItem | null>(null);

  // Drag state
  const [dragItemId, setDragItemId] = useState<number | null>(null);

  const fetchItems = useCallback(async () => {
    try {
      const data = await getKanbanItems();
      setItems(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load kanban items');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchItems();
  }, [fetchItems]);

  // Listen for real-time updates
  useEffect(() => {
    if (!gateway) return;

    const handleUpdate = () => {
      fetchItems();
    };

    gateway.on('kanban_item_updated', handleUpdate);
    return () => {
      gateway.off('kanban_item_updated', handleUpdate);
    };
  }, [gateway, fetchItems]);

  const handleCreate = async () => {
    if (!createTitle.trim()) return;
    setCreating(true);
    try {
      await createKanbanItem({
        title: createTitle.trim(),
        description: createDesc.trim() || undefined,
        priority: createPriority,
      });
      setCreateOpen(false);
      setCreateTitle('');
      setCreateDesc('');
      setCreatePriority(0);
      fetchItems();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to create item');
    } finally {
      setCreating(false);
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await deleteKanbanItem(id);
      setDetailItem(null);
      fetchItems();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete item');
    }
  };

  const handleStatusChange = async (id: number, status: KanbanStatus) => {
    try {
      await updateKanbanItem(id, { status });
      fetchItems();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to update item');
    }
  };

  // Drag and drop handlers
  const onDragStart = (e: DragEvent, id: number) => {
    setDragItemId(id);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', String(id));
  };

  const onDragOver = (e: DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  };

  const onDrop = (e: DragEvent, targetStatus: KanbanStatus) => {
    e.preventDefault();
    const id = dragItemId;
    setDragItemId(null);
    if (id === null) return;

    const item = items.find((i) => i.id === id);
    if (item && item.status !== targetStatus) {
      handleStatusChange(id, targetStatus);
    }
  };

  const onDragEnd = () => {
    setDragItemId(null);
  };

  const itemsByStatus = (status: KanbanStatus) =>
    items.filter((i) => i.status === status);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-stark-500" />
      </div>
    );
  }

  return (
    <div className="p-6 h-full flex flex-col">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Kanban Board</h1>
        <Button variant="primary" size="sm" onClick={() => setCreateOpen(true)}>
          <Plus className="w-4 h-4 mr-1" />
          Add Task
        </Button>
      </div>

      {error && (
        <div className="mb-4 p-3 rounded-lg bg-red-500/10 border border-red-500/30 text-red-400 text-sm">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">dismiss</button>
        </div>
      )}

      {/* Three-column board */}
      <div className="flex-1 grid grid-cols-1 md:grid-cols-3 gap-4 min-h-0">
        {COLUMNS.map((col) => (
          <div
            key={col.status}
            className={`flex flex-col rounded-xl border ${col.border} ${col.bg} overflow-hidden`}
            onDragOver={onDragOver}
            onDrop={(e) => onDrop(e, col.status)}
          >
            {/* Column header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-slate-700/50">
              <div className="flex items-center gap-2">
                <h2 className={`font-semibold ${col.color}`}>{col.label}</h2>
                <span className="text-xs text-slate-500 bg-slate-800 rounded-full px-2 py-0.5">
                  {itemsByStatus(col.status).length}
                </span>
              </div>
            </div>

            {/* Cards */}
            <div className="flex-1 overflow-y-auto p-3 space-y-2">
              {itemsByStatus(col.status).map((item) => (
                <div
                  key={item.id}
                  draggable
                  onDragStart={(e) => onDragStart(e, item.id)}
                  onDragEnd={onDragEnd}
                  onClick={() => setDetailItem(item)}
                  className={`group bg-slate-800/80 rounded-lg p-3 border border-slate-700/50 cursor-pointer
                    hover:border-slate-600 hover:bg-slate-800 transition-colors
                    ${dragItemId === item.id ? 'opacity-50' : ''}`}
                >
                  <div className="flex items-start gap-2">
                    <GripVertical className="w-4 h-4 text-slate-600 mt-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity cursor-grab" />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="text-sm font-medium text-white truncate">{item.title}</span>
                        {item.priority > 0 && (
                          <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium shrink-0 ${PRIORITY_LABELS[item.priority]?.class}`}>
                            {PRIORITY_LABELS[item.priority]?.label}
                          </span>
                        )}
                      </div>
                      {item.description && (
                        <p className="text-xs text-slate-400 line-clamp-2">{item.description}</p>
                      )}
                      {item.session_id && (
                        <div className="mt-1.5 flex items-center gap-1 text-[10px] text-slate-500">
                          <ExternalLink className="w-3 h-3" />
                          Session #{item.session_id}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))}

              {itemsByStatus(col.status).length === 0 && (
                <div className="text-center text-sm text-slate-600 py-8">
                  No tasks
                </div>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Create Modal */}
      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)} title="New Kanban Task" size="md">
        <div className="space-y-4">
          <Input
            label="Title"
            value={createTitle}
            onChange={(e) => setCreateTitle(e.target.value)}
            placeholder="What needs to be done?"
            autoFocus
          />
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-1">Description</label>
            <textarea
              value={createDesc}
              onChange={(e) => setCreateDesc(e.target.value)}
              placeholder="Optional details..."
              rows={3}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white text-sm
                focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent resize-none"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-slate-300 mb-1">Priority</label>
            <select
              value={createPriority}
              onChange={(e) => setCreatePriority(Number(e.target.value))}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white text-sm
                focus:outline-none focus:ring-2 focus:ring-stark-500 focus:border-transparent"
            >
              <option value={0}>Normal</option>
              <option value={1}>High</option>
              <option value={2}>Urgent</option>
            </select>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button variant="secondary" size="sm" onClick={() => setCreateOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={handleCreate}
              isLoading={creating}
              disabled={!createTitle.trim()}
            >
              Create
            </Button>
          </div>
        </div>
      </Modal>

      {/* Detail Modal */}
      <Modal
        isOpen={!!detailItem}
        onClose={() => setDetailItem(null)}
        title={detailItem?.title || ''}
        size="lg"
      >
        {detailItem && (
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <span className={`text-xs px-2 py-1 rounded-full font-medium ${
                detailItem.status === 'ready' ? 'bg-blue-500/20 text-blue-400' :
                detailItem.status === 'in_progress' ? 'bg-yellow-500/20 text-yellow-400' :
                'bg-green-500/20 text-green-400'
              }`}>
                {detailItem.status === 'in_progress' ? 'In Progress' :
                 detailItem.status.charAt(0).toUpperCase() + detailItem.status.slice(1)}
              </span>
              {detailItem.priority > 0 && (
                <span className={`text-xs px-2 py-1 rounded-full font-medium ${PRIORITY_LABELS[detailItem.priority]?.class}`}>
                  {PRIORITY_LABELS[detailItem.priority]?.label}
                </span>
              )}
            </div>

            {detailItem.description && (
              <div>
                <h3 className="text-sm font-medium text-slate-400 mb-1">Description</h3>
                <p className="text-sm text-white whitespace-pre-wrap">{detailItem.description}</p>
              </div>
            )}

            {detailItem.result && (
              <div>
                <h3 className="text-sm font-medium text-slate-400 mb-1">Agent Notes</h3>
                <pre className="text-sm text-slate-300 whitespace-pre-wrap bg-slate-900 rounded-lg p-3 border border-slate-700">
                  {detailItem.result}
                </pre>
              </div>
            )}

            {detailItem.session_id && (
              <div>
                <h3 className="text-sm font-medium text-slate-400 mb-1">Session</h3>
                <button
                  onClick={() => navigate(`/sessions/${detailItem.session_id}`)}
                  className="text-sm text-stark-400 hover:text-stark-300 flex items-center gap-1"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  View Session #{detailItem.session_id}
                </button>
              </div>
            )}

            <div className="text-xs text-slate-500 space-y-1">
              <p>Created: {new Date(detailItem.created_at).toLocaleString()}</p>
              <p>Updated: {new Date(detailItem.updated_at).toLocaleString()}</p>
            </div>

            {/* Actions */}
            <div className="flex items-center justify-between pt-2 border-t border-slate-700">
              <Button
                variant="danger"
                size="sm"
                onClick={() => handleDelete(detailItem.id)}
              >
                <Trash2 className="w-3.5 h-3.5 mr-1" />
                Delete
              </Button>

              <div className="flex gap-2">
                {detailItem.status !== 'ready' && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      handleStatusChange(detailItem.id, 'ready');
                      setDetailItem(null);
                    }}
                  >
                    Move to Ready
                  </Button>
                )}
                {detailItem.status !== 'in_progress' && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      handleStatusChange(detailItem.id, 'in_progress');
                      setDetailItem(null);
                    }}
                  >
                    Move to In Progress
                  </Button>
                )}
                {detailItem.status !== 'complete' && (
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => {
                      handleStatusChange(detailItem.id, 'complete');
                      setDetailItem(null);
                    }}
                  >
                    Mark Complete
                  </Button>
                )}
              </div>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
