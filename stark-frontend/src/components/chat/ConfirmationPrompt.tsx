import { useState } from 'react';
import { AlertTriangle, Check, X, Loader2 } from 'lucide-react';
import Button from '../ui/Button';

export interface PendingConfirmation {
  confirmation_id: string;
  channel_id: number;
  tool_name: string;
  description: string;
  parameters: Record<string, unknown>;
  timestamp: string;
}

interface ConfirmationPromptProps {
  confirmation: PendingConfirmation;
  onConfirm: (confirmationId: string) => Promise<void>;
  onCancel: (confirmationId: string) => Promise<void>;
}

export function ConfirmationPrompt({
  confirmation,
  onConfirm,
  onCancel
}: ConfirmationPromptProps) {
  const [isLoading, setIsLoading] = useState<'confirm' | 'cancel' | null>(null);
  const [resolved, setResolved] = useState<'confirmed' | 'cancelled' | null>(null);

  const handleConfirm = async () => {
    setIsLoading('confirm');
    try {
      await onConfirm(confirmation.confirmation_id);
      setResolved('confirmed');
    } catch (error) {
      console.error('Failed to confirm:', error);
      setIsLoading(null);
    }
  };

  const handleCancel = async () => {
    setIsLoading('cancel');
    try {
      await onCancel(confirmation.confirmation_id);
      setResolved('cancelled');
    } catch (error) {
      console.error('Failed to cancel:', error);
      setIsLoading(null);
    }
  };

  // Extract useful info from parameters
  const params = confirmation.parameters;
  const network = (params.network as string) || 'base';
  const to = params.to as string;
  const value = params.value as string;
  const data = params.data as string;

  // Format value for display
  const formatValue = (wei: string | undefined) => {
    if (!wei || wei === '0') return null;
    try {
      const num = BigInt(wei);
      const eth = Number(num) / 1e18;
      return eth < 0.0001 ? eth.toFixed(8) : eth.toFixed(6);
    } catch {
      return wei;
    }
  };

  const ethValue = formatValue(value);
  const isContractCall = data && data !== '0x' && data.length > 2;

  if (resolved) {
    return (
      <div className={`rounded-lg p-4 border ${
        resolved === 'confirmed'
          ? 'bg-green-500/10 border-green-500/30'
          : 'bg-slate-700/50 border-slate-600'
      }`}>
        <div className="flex items-center gap-2">
          {resolved === 'confirmed' ? (
            <>
              <Check className="w-5 h-5 text-green-400" />
              <span className="text-green-400 font-medium">Transaction confirmed</span>
            </>
          ) : (
            <>
              <X className="w-5 h-5 text-slate-400" />
              <span className="text-slate-400 font-medium">Transaction cancelled</span>
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-lg bg-amber-500/10 border border-amber-500/30 p-4 space-y-3">
      {/* Header */}
      <div className="flex items-start gap-3">
        <AlertTriangle className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <h4 className="text-amber-400 font-medium">Transaction Confirmation Required</h4>
          <p className="text-slate-300 text-sm mt-1">{confirmation.description}</p>
        </div>
      </div>

      {/* Transaction Details */}
      <div className="bg-slate-800/50 rounded-md p-3 space-y-2 text-sm">
        <div className="flex justify-between">
          <span className="text-slate-400">Network:</span>
          <span className="text-slate-200 font-mono">{network}</span>
        </div>
        {to && (
          <div className="flex justify-between">
            <span className="text-slate-400">To:</span>
            <span className="text-slate-200 font-mono text-xs">
              {to.slice(0, 10)}...{to.slice(-8)}
            </span>
          </div>
        )}
        {ethValue && (
          <div className="flex justify-between">
            <span className="text-slate-400">Value:</span>
            <span className="text-slate-200">{ethValue} ETH</span>
          </div>
        )}
        {isContractCall && (
          <div className="flex justify-between">
            <span className="text-slate-400">Type:</span>
            <span className="text-slate-200">Contract Call</span>
          </div>
        )}
      </div>

      {/* Action Buttons */}
      <div className="flex gap-3 pt-1">
        <Button
          onClick={handleConfirm}
          disabled={isLoading !== null}
          className="flex-1 bg-green-600 hover:bg-green-700 text-white"
        >
          {isLoading === 'confirm' ? (
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
          ) : (
            <Check className="w-4 h-4 mr-2" />
          )}
          Confirm
        </Button>
        <Button
          onClick={handleCancel}
          disabled={isLoading !== null}
          variant="secondary"
          className="flex-1"
        >
          {isLoading === 'cancel' ? (
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
          ) : (
            <X className="w-4 h-4 mr-2" />
          )}
          Cancel
        </Button>
      </div>

      {/* Expiry notice */}
      <p className="text-xs text-slate-500 text-center">
        This confirmation will expire in 5 minutes
      </p>
    </div>
  );
}
