import { useState } from 'react';
import { Skull, Users } from 'lucide-react';
import Card, { CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { updateBotSettings } from '@/lib/api';

interface OperatingModeCardProps {
  rogueModeEnabled: boolean;
  onModeChange: (newValue: boolean) => void;
  onMessage?: (message: { type: 'success' | 'error'; text: string }) => void;
}

export default function OperatingModeCard({
  rogueModeEnabled,
  onModeChange,
  onMessage,
}: OperatingModeCardProps) {
  const [isSaving, setIsSaving] = useState(false);

  const handleToggle = async () => {
    const newValue = !rogueModeEnabled;
    setIsSaving(true);
    onMessage?.(null as any);
    try {
      await updateBotSettings({
        rogue_mode_enabled: newValue,
      });
      onModeChange(newValue);
      onMessage?.({ type: 'success', text: `Switched to ${newValue ? 'Rogue' : 'Partner'} mode` });
    } catch (err) {
      onMessage?.({ type: 'error', text: 'Failed to update operating mode' });
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {rogueModeEnabled ? (
            <Skull className="w-5 h-5 text-red-400" />
          ) : (
            <Users className="w-5 h-5 text-stark-400" />
          )}
          Operating Mode
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between p-4 bg-slate-800/50 rounded-lg">
          <div className="flex items-center gap-3">
            <Users className={`w-5 h-5 ${!rogueModeEnabled ? 'text-stark-400' : 'text-slate-500'}`} />
            <span className={`font-medium ${!rogueModeEnabled ? 'text-white' : 'text-slate-500'}`}>
              Partner
            </span>
          </div>

          <button
            onClick={handleToggle}
            disabled={isSaving}
            className={`relative w-14 h-7 rounded-full transition-colors duration-200 ${
              rogueModeEnabled
                ? 'bg-red-500'
                : 'bg-stark-500'
            } ${isSaving ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}
          >
            <div
              className={`absolute top-1 w-5 h-5 rounded-full bg-white transition-transform duration-200 ${
                rogueModeEnabled ? 'translate-x-8' : 'translate-x-1'
              }`}
            />
          </button>

          <div className="flex items-center gap-3">
            <span className={`font-medium ${rogueModeEnabled ? 'text-white' : 'text-slate-500'}`}>
              Rogue
            </span>
            <Skull className={`w-5 h-5 ${rogueModeEnabled ? 'text-red-400' : 'text-slate-500'}`} />
          </div>
        </div>
        <div className="flex justify-between mt-3 text-xs text-slate-500">
          <div className="text-left">
            <p>Collaborative assistant</p>
            <p>Transactions queued for approval</p>
          </div>
          <div className="text-right">
            <p>Autonomous agent</p>
            <p>Transactions auto-broadcast</p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
