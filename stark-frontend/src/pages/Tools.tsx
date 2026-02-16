import { useState, useEffect } from 'react';
import { Wrench, Check, X, Shield, Eye } from 'lucide-react';
import Card, { CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { getTools, getToolGroups, ToolGroupInfo } from '@/lib/api';

interface Tool {
  name: string;
  description: string;
  group: string;
  enabled: boolean;
  safety_level: string;
}

export default function Tools() {
  const [tools, setTools] = useState<Tool[]>([]);
  const [groups, setGroups] = useState<ToolGroupInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const [toolsData, groupsData] = await Promise.all([
        getTools(),
        getToolGroups(),
      ]);
      setTools(toolsData);
      setGroups(groupsData);
    } catch (err) {
      setError('Failed to load tools');
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="p-4 sm:p-8 flex items-center justify-center">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading tools...</span>
        </div>
      </div>
    );
  }

  // Group tools by their group
  const toolsByGroup = tools.reduce((acc, tool) => {
    const group = tool.group || 'other';
    if (!acc[group]) {
      acc[group] = [];
    }
    acc[group].push(tool);
    return acc;
  }, {} as Record<string, Tool[]>);

  // Build group labels from API response
  const groupLabels: Record<string, string> = groups.reduce((acc, g) => {
    acc[g.key] = g.label;
    return acc;
  }, { other: 'Other Tools' } as Record<string, string>);

  // Use API order for groups, with 'other' at the end
  const groupOrder = [...groups.map(g => g.key), 'other'];

  return (
    <div className="p-4 sm:p-8">
      <div className="mb-6 sm:mb-8">
        <h1 className="text-xl sm:text-2xl font-bold text-white mb-1 sm:mb-2">Tools</h1>
        <p className="text-sm sm:text-base text-slate-400">Available tools for your agent</p>
      </div>

      {error && (
        <div className="mb-6 bg-red-500/20 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      <div className="space-y-6">
        {groupOrder.map((groupKey) => {
          const groupTools = toolsByGroup[groupKey];
          if (!groupTools || groupTools.length === 0) return null;

          return (
            <Card key={groupKey}>
              <CardHeader>
                <CardTitle>{groupLabels[groupKey] || groupKey}</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-3">
                  {groupTools.map((tool) => (
                    <div
                      key={tool.name}
                      className="p-3 sm:p-4 rounded-lg bg-slate-700/50"
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="flex items-center gap-2 sm:gap-3 min-w-0">
                          <div className="p-1.5 sm:p-2 bg-slate-600 rounded-lg shrink-0">
                            <Wrench className="w-4 h-4 sm:w-5 sm:h-5 text-slate-300" />
                          </div>
                          <p className="font-medium text-white text-sm sm:text-base truncate">{tool.name}</p>
                          {tool.safety_level === 'safe_mode' && (
                            <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] sm:text-xs font-medium bg-green-500/20 text-green-400 border border-green-500/30 shrink-0">
                              <Shield className="w-3 h-3" />
                              Safe
                            </span>
                          )}
                          {tool.safety_level === 'read_only' && (
                            <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] sm:text-xs font-medium bg-blue-500/20 text-blue-400 border border-blue-500/30 shrink-0">
                              <Eye className="w-3 h-3" />
                              ReadOnly
                            </span>
                          )}
                        </div>
                        <div
                          className={`p-1.5 sm:p-2 rounded-lg shrink-0 ${
                            tool.enabled
                              ? 'bg-green-500/20 text-green-400'
                              : 'bg-slate-600 text-slate-400'
                          }`}
                        >
                          {tool.enabled ? (
                            <Check className="w-4 h-4 sm:w-5 sm:h-5" />
                          ) : (
                            <X className="w-4 h-4 sm:w-5 sm:h-5" />
                          )}
                        </div>
                      </div>
                      {tool.description && (
                        <p className="text-xs sm:text-sm text-slate-400 mt-2 pl-9 sm:pl-11">{tool.description}</p>
                      )}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          );
        })}

        {tools.length === 0 && (
          <Card>
            <CardContent className="text-center py-12">
              <Wrench className="w-12 h-12 text-slate-600 mx-auto mb-4" />
              <p className="text-slate-400">No tools available</p>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
