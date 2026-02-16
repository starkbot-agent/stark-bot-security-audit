import { useState, useCallback } from 'react';
import { ExternalLink, Check, X, Loader2, Copy } from 'lucide-react';
import clsx from 'clsx';
import type { TrackedTransaction } from '@/types';

interface TransactionTrackerProps {
  transactions: TrackedTransaction[];
  className?: string;
}

// Truncate tx hash for display
function truncateHash(hash: string): string {
  if (hash.length <= 16) return hash;
  return `${hash.slice(0, 10)}...${hash.slice(-6)}`;
}

// Single transaction card
function TransactionCard({ tx }: { tx: TrackedTransaction }) {
  const [copied, setCopied] = useState(false);

  const copyHash = useCallback(() => {
    navigator.clipboard.writeText(tx.tx_hash);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [tx.tx_hash]);

  const statusIcon = () => {
    switch (tx.status) {
      case 'pending':
        return <Loader2 className="w-4 h-4 animate-spin text-amber-400" />;
      case 'confirmed':
        return <Check className="w-4 h-4 text-green-400" />;
      case 'reverted':
        return <X className="w-4 h-4 text-red-400" />;
    }
  };

  const statusText = () => {
    switch (tx.status) {
      case 'pending':
        return 'Pending';
      case 'confirmed':
        return 'Confirmed';
      case 'reverted':
        return 'Reverted';
    }
  };

  const statusColor = () => {
    switch (tx.status) {
      case 'pending':
        return 'border-amber-500/50 bg-amber-500/10';
      case 'confirmed':
        return 'border-green-500/50 bg-green-500/10';
      case 'reverted':
        return 'border-red-500/50 bg-red-500/10';
    }
  };

  return (
    <div
      className={clsx(
        'flex items-center gap-3 px-4 py-3 rounded-lg border',
        statusColor()
      )}
    >
      {/* Status icon */}
      <div className="flex-shrink-0">{statusIcon()}</div>

      {/* Transaction info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-slate-200">
            Transaction
          </span>
          <span
            className={clsx(
              'text-xs px-2 py-0.5 rounded-full font-medium',
              tx.status === 'pending' && 'bg-amber-500/20 text-amber-400',
              tx.status === 'confirmed' && 'bg-green-500/20 text-green-400',
              tx.status === 'reverted' && 'bg-red-500/20 text-red-400'
            )}
          >
            {statusText()}
          </span>
        </div>
        <div className="flex items-center gap-2 mt-1">
          <code className="text-xs font-mono text-slate-400">
            {truncateHash(tx.tx_hash)}
          </code>
          <button
            onClick={copyHash}
            className="text-slate-500 hover:text-slate-300 transition-colors"
            title="Copy transaction hash"
          >
            {copied ? (
              <Check className="w-3 h-3 text-green-400" />
            ) : (
              <Copy className="w-3 h-3" />
            )}
          </button>
        </div>
      </div>

      {/* Network badge */}
      <div className="flex-shrink-0">
        <span className="text-xs px-2 py-1 rounded bg-slate-700 text-slate-300 font-medium">
          {tx.network === 'mainnet' ? 'ETH' : 'Base'}
        </span>
      </div>

      {/* Explorer link */}
      <a
        href={tx.explorer_url}
        target="_blank"
        rel="noopener noreferrer"
        className="flex-shrink-0 text-slate-400 hover:text-slate-200 transition-colors"
        title="View on explorer"
      >
        <ExternalLink className="w-4 h-4" />
      </a>
    </div>
  );
}

export default function TransactionTracker({
  transactions,
  className,
}: TransactionTrackerProps) {
  // Only show if there are transactions
  if (transactions.length === 0) {
    return null;
  }

  // Only show the most recent transaction
  const mostRecent = transactions[transactions.length - 1];

  return (
    <div className={clsx('space-y-2', className)}>
      <TransactionCard key={mostRecent.tx_hash} tx={mostRecent} />
    </div>
  );
}
