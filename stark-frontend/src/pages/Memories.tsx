import { useState, useEffect } from 'react';
import { FileText, Trash2 } from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import { getMemories, deleteMemory } from '@/lib/api';

interface Memory {
  id: number;
  content: string;
  importance?: number;
  created_at: string;
}

export default function Memories() {
  const [memories, setMemories] = useState<Memory[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadMemories();
  }, []);

  const loadMemories = async () => {
    try {
      const data = await getMemories();
      setMemories(data);
    } catch (err) {
      setError('Failed to load memories');
    } finally {
      setIsLoading(false);
    }
  };

  const handleDelete = async (id: number) => {
    if (!confirm('Are you sure you want to delete this memory?')) return;

    try {
      await deleteMemory(String(id));
      setMemories((prev) => prev.filter((m) => m.id !== id));
    } catch (err) {
      setError('Failed to delete memory');
    }
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString();
  };

  if (isLoading) {
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
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white mb-2">Memories</h1>
        <p className="text-slate-400">View stored agent memories</p>
      </div>

      {error && (
        <div className="mb-6 bg-red-500/20 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      {memories.length > 0 ? (
        <div className="space-y-4">
          {memories.map((memory) => (
            <Card key={memory.id}>
              <CardContent>
                <div className="flex items-start justify-between gap-4">
                  <div className="flex items-start gap-4 flex-1">
                    <div className="p-3 bg-green-500/20 rounded-lg shrink-0">
                      <FileText className="w-6 h-6 text-green-400" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-white whitespace-pre-wrap break-words">
                        {memory.content}
                      </p>
                      <div className="flex items-center gap-4 mt-2 text-sm text-slate-400">
                        <span>{formatDate(memory.created_at)}</span>
                        {memory.importance !== undefined && (
                          <span className="px-2 py-0.5 bg-slate-700 rounded">
                            Importance: {memory.importance}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDelete(memory.id)}
                    className="text-red-400 hover:text-red-300 hover:bg-red-500/20 shrink-0"
                  >
                    <Trash2 className="w-4 h-4" />
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <Card>
          <CardContent className="text-center py-12">
            <FileText className="w-12 h-12 text-slate-600 mx-auto mb-4" />
            <p className="text-slate-400">No memories stored</p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
