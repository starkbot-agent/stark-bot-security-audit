import { useState } from 'react';
import Modal from '../ui/Modal';
import Button from '../ui/Button';
import { AlertTriangle, Check, X, Loader2 } from 'lucide-react';
import { getGateway } from '@/lib/gateway-client';

export interface TxQueueTransaction {
  uuid: string;
  network: string;
  to: string;
  value: string;
  value_formatted: string;
}

interface TxQueueConfirmationModalProps {
  isOpen: boolean;
  onClose: () => void;
  channelId: number;
  transaction: TxQueueTransaction | null;
}

export default function TxQueueConfirmationModal({
  isOpen,
  onClose,
  channelId,
  transaction
}: TxQueueConfirmationModalProps) {
  const [isLoading, setIsLoading] = useState<'confirm' | 'deny' | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleConfirm = async () => {
    if (!transaction) return;
    setIsLoading('confirm');
    setError(null);
    try {
      await getGateway().call('tx_queue.confirm', {
        uuid: transaction.uuid,
        channel_id: channelId
      });
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to confirm transaction');
    } finally {
      setIsLoading(null);
    }
  };

  const handleDeny = async () => {
    if (!transaction) return;
    setIsLoading('deny');
    setError(null);
    try {
      await getGateway().call('tx_queue.deny', {
        uuid: transaction.uuid,
        channel_id: channelId
      });
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to deny transaction');
    } finally {
      setIsLoading(null);
    }
  };

  if (!transaction) return null;

  return (
    <Modal isOpen={isOpen} onClose={() => {}} title="Confirm Transaction" size="md">
      <div className="space-y-4">
        <div className="flex items-start gap-3">
          <AlertTriangle className="w-6 h-6 text-amber-400 flex-shrink-0 mt-0.5" />
          <div>
            <h3 className="text-white font-medium">Broadcast Transaction?</h3>
            <p className="text-slate-400 text-sm mt-1">
              Partner mode requires your approval before broadcasting.
            </p>
          </div>
        </div>

        <div className="bg-slate-700/50 rounded-lg p-4 space-y-3">
          <div className="flex justify-between">
            <span className="text-slate-400">Network</span>
            <span className="text-white font-mono uppercase">{transaction.network}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-slate-400">To</span>
            <span className="text-white font-mono text-sm">
              {transaction.to.slice(0, 10)}...{transaction.to.slice(-8)}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-slate-400">Value</span>
            <span className="text-white font-medium">{transaction.value_formatted}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-slate-400">UUID</span>
            <span className="text-slate-300 font-mono text-xs">{transaction.uuid.slice(0, 12)}...</span>
          </div>
        </div>

        {error && (
          <div className="text-red-400 text-sm bg-red-900/20 p-2 rounded">{error}</div>
        )}

        <div className="flex gap-3 pt-2">
          <Button
            onClick={handleConfirm}
            disabled={isLoading !== null}
            className="flex-1 bg-green-600 hover:bg-green-700"
          >
            {isLoading === 'confirm' ? (
              <Loader2 className="w-4 h-4 animate-spin mr-2" />
            ) : (
              <Check className="w-4 h-4 mr-2" />
            )}
            Confirm
          </Button>
          <Button
            onClick={handleDeny}
            disabled={isLoading !== null}
            variant="secondary"
            className="flex-1 border border-red-600 text-red-400 hover:bg-red-900/20"
          >
            {isLoading === 'deny' ? (
              <Loader2 className="w-4 h-4 animate-spin mr-2" />
            ) : (
              <X className="w-4 h-4 mr-2" />
            )}
            Deny
          </Button>
        </div>
      </div>
    </Modal>
  );
}
