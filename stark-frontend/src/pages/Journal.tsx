import { useState, useEffect } from 'react';
import {
  BookOpen,
  RefreshCw,
  AlertCircle,
  Folder,
  FolderOpen,
  FileText,
  ChevronRight,
  ChevronDown,
  Calendar,
} from 'lucide-react';
import {
  listJournal,
  readJournalFile,
  JournalEntry,
} from '@/lib/api';

interface TreeNode {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified?: string;
  children?: TreeNode[];
  expanded?: boolean;
  loaded?: boolean;
}

export default function Journal() {
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingFile, setIsLoadingFile] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fileError, setFileError] = useState<string | null>(null);

  const loadDirectory = async (path?: string): Promise<TreeNode[]> => {
    const response = await listJournal(path);
    if (!response.success) {
      throw new Error(response.error || 'Failed to load directory');
    }
    return response.entries.map((entry: JournalEntry) => ({
      ...entry,
      expanded: false,
      loaded: !entry.is_dir,
      children: entry.is_dir ? [] : undefined,
    }));
  };

  const loadRoot = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const nodes = await loadDirectory();
      setTree(nodes);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load journal');
    } finally {
      setIsLoading(false);
    }
  };

  const toggleDirectory = async (node: TreeNode) => {
    if (!node.is_dir) return;

    if (!node.loaded) {
      // Load children first
      try {
        const children = await loadDirectory(node.path);
        setTree((prevTree) =>
          updateNodeInTree(prevTree, node.path, {
            ...node,
            expanded: true,
            loaded: true,
            children,
          })
        );
      } catch (err) {
        console.error('Failed to load directory:', err);
      }
    } else {
      // Just toggle expanded state
      setTree((prevTree) =>
        updateNodeInTree(prevTree, node.path, {
          ...node,
          expanded: !node.expanded,
        })
      );
    }
  };

  const updateNodeInTree = (
    nodes: TreeNode[],
    targetPath: string,
    newNode: TreeNode
  ): TreeNode[] => {
    return nodes.map((n) => {
      if (n.path === targetPath) {
        return newNode;
      }
      // Check if targetPath is a child of this node (must start with path + separator)
      if (n.children && n.is_dir && (targetPath.startsWith(n.path + '/') || targetPath.startsWith(n.path + '\\'))) {
        return { ...n, children: updateNodeInTree(n.children, targetPath, newNode) };
      }
      return n;
    });
  };

  const loadFile = async (path: string) => {
    setIsLoadingFile(true);
    setFileError(null);
    setFileContent(null);
    setSelectedFile(path);
    try {
      const response = await readJournalFile(path);
      if (response.success && response.content !== undefined) {
        setFileContent(response.content);
      } else {
        setFileError(response.error || 'Failed to load file');
      }
    } catch (err) {
      setFileError('Failed to load file');
    } finally {
      setIsLoadingFile(false);
    }
  };

  useEffect(() => {
    loadRoot();
  }, []);

  const refresh = () => {
    loadRoot();
    setSelectedFile(null);
    setFileContent(null);
  };

  const renderTree = (nodes: TreeNode[], depth: number = 0): JSX.Element[] => {
    return nodes.map((node) => (
      <div key={node.path}>
        <button
          onClick={() => {
            if (node.is_dir) {
              toggleDirectory(node);
            } else {
              loadFile(node.path);
            }
          }}
          className={`w-full flex items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-slate-700/50 ${
            selectedFile === node.path
              ? 'bg-stark-500/20 text-stark-400'
              : 'text-slate-300'
          }`}
          style={{ paddingLeft: `${depth * 16 + 12}px` }}
        >
          {node.is_dir ? (
            <>
              {node.expanded ? (
                <ChevronDown className="w-4 h-4 flex-shrink-0 text-slate-500" />
              ) : (
                <ChevronRight className="w-4 h-4 flex-shrink-0 text-slate-500" />
              )}
              {node.expanded ? (
                <FolderOpen className="w-4 h-4 flex-shrink-0 text-amber-400" />
              ) : (
                <Folder className="w-4 h-4 flex-shrink-0 text-amber-400" />
              )}
            </>
          ) : (
            <>
              <span className="w-4" />
              <FileText className="w-4 h-4 flex-shrink-0 text-slate-400" />
            </>
          )}
          <span className="truncate text-sm">{node.name}</span>
          {node.modified && !node.is_dir && (
            <span className="ml-auto text-xs text-slate-500 flex-shrink-0">
              {node.modified.split(' ')[0]}
            </span>
          )}
        </button>
        {node.is_dir && node.expanded && node.children && (
          <div>{renderTree(node.children, depth + 1)}</div>
        )}
      </div>
    ));
  };

  // Extract date from filename for display (e.g., "2024-01-15.md" -> "January 15, 2024")
  const formatDateFromFilename = (filename: string): string | null => {
    const match = filename.match(/^(\d{4})-(\d{2})-(\d{2})\.md$/);
    if (match) {
      const date = new Date(parseInt(match[1]), parseInt(match[2]) - 1, parseInt(match[3]));
      return date.toLocaleDateString('en-US', {
        weekday: 'long',
        year: 'numeric',
        month: 'long',
        day: 'numeric',
      });
    }
    return null;
  };

  const selectedFilename = selectedFile?.split('/').pop() || '';
  const formattedDate = formatDateFromFilename(selectedFilename);

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-6 border-b border-slate-700">
        <div className="flex items-center justify-between mb-2">
          <div>
            <h1 className="text-2xl font-bold text-white">Journal</h1>
            <p className="text-slate-400 text-sm mt-1">
              Personal journal entries and notes
            </p>
          </div>
          <button
            onClick={refresh}
            className="p-2 text-slate-400 hover:text-white hover:bg-slate-700 rounded-lg transition-colors"
            title="Refresh"
          >
            <RefreshCw className="w-5 h-5" />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* File Tree */}
        <div className="w-80 border-r border-slate-700 overflow-y-auto">
          {isLoading ? (
            <div className="flex items-center justify-center h-32">
              <RefreshCw className="w-6 h-6 text-slate-400 animate-spin" />
            </div>
          ) : error ? (
            <div className="p-4">
              <div className="flex items-center gap-2 text-amber-400 bg-amber-500/10 px-4 py-3 rounded-lg">
                <AlertCircle className="w-5 h-5 flex-shrink-0" />
                <span className="text-sm">{error}</span>
              </div>
            </div>
          ) : tree.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-32 text-slate-400">
              <BookOpen className="w-8 h-8 mb-2" />
              <span className="text-sm">No journal entries yet</span>
              <span className="text-xs mt-1">Use the journal skill to create entries</span>
            </div>
          ) : (
            <div className="py-2">{renderTree(tree)}</div>
          )}
        </div>

        {/* File Preview */}
        <div className="flex-1 overflow-hidden flex flex-col bg-slate-900">
          {selectedFile ? (
            <>
              <div className="px-4 py-3 border-b border-slate-700 flex items-center gap-2">
                {formattedDate ? (
                  <>
                    <Calendar className="w-4 h-4 text-stark-400" />
                    <span className="text-sm text-slate-300">
                      {formattedDate}
                    </span>
                  </>
                ) : (
                  <>
                    <FileText className="w-4 h-4 text-slate-400" />
                    <span className="text-sm text-slate-300 font-mono">
                      {selectedFile}
                    </span>
                  </>
                )}
              </div>
              <div className="flex-1 overflow-auto">
                {isLoadingFile ? (
                  <div className="flex items-center justify-center h-32">
                    <RefreshCw className="w-6 h-6 text-slate-400 animate-spin" />
                  </div>
                ) : fileError ? (
                  <div className="p-4">
                    <div className="flex items-center gap-2 text-amber-400 bg-amber-500/10 px-4 py-3 rounded-lg">
                      <AlertCircle className="w-5 h-5 flex-shrink-0" />
                      <span className="text-sm">{fileError}</span>
                    </div>
                  </div>
                ) : fileContent !== null ? (
                  <div className="p-4">
                    <div className="prose prose-invert prose-sm max-w-none">
                      <pre className="whitespace-pre-wrap break-words text-slate-300 font-mono text-sm bg-transparent p-0 m-0">
                        {fileContent}
                      </pre>
                    </div>
                  </div>
                ) : null}
              </div>
            </>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center text-slate-500">
              <BookOpen className="w-12 h-12 mb-3 opacity-50" />
              <p>Select a journal entry to view</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
