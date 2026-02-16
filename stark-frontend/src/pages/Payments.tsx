import { useState, useEffect } from 'react';
import { DollarSign, Receipt, TrendingUp, ExternalLink, Clock } from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import Button from '@/components/ui/Button';
import { useApi } from '@/hooks/useApi';

interface PaymentInfo {
  id: number;
  channel_id: number | null;
  tool_name: string | null;
  resource: string | null;
  amount: string;
  amount_formatted: string;
  asset: string;
  pay_to: string;
  tx_hash: string | null;
  status: 'pending' | 'confirmed' | 'failed';
  feedback_submitted: boolean;
  created_at: string;
}

interface PaymentSummary {
  total_payments: number;
  total_usdc_spent: string;
  payments_with_feedback: number;
  payments_without_feedback: number;
}

interface PaymentsResponse {
  success: boolean;
  payments?: PaymentInfo[];
  total?: number;
  error?: string;
}

interface SummaryResponse {
  success: boolean;
  summary?: PaymentSummary;
  error?: string;
}

export default function Payments() {
  const [filter, setFilter] = useState<'all' | 'with_feedback' | 'without_feedback'>('all');
  const { data: paymentsData, isLoading: paymentsLoading, refetch: refetchPayments } = useApi<PaymentsResponse>('/payments');
  const { data: summaryData, isLoading: summaryLoading, refetch: refetchSummary } = useApi<SummaryResponse>('/payments/summary');

  // Poll every 5 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      refetchPayments();
      refetchSummary();
    }, 5000);
    return () => clearInterval(interval);
  }, [refetchPayments, refetchSummary]);

  const payments = paymentsData?.payments ?? [];
  const summary = summaryData?.summary;

  const filteredPayments = payments.filter(p => {
    if (filter === 'with_feedback') return p.feedback_submitted;
    if (filter === 'without_feedback') return !p.feedback_submitted;
    return true;
  });

  // Calculate sum of filtered payments
  const filteredTotal = filteredPayments.reduce((sum, p) => sum + parseFloat(p.amount), 0).toFixed(6);

  // Extract tool name from resource URL when tool_name is null
  const getToolName = (payment: PaymentInfo): string => {
    if (payment.tool_name) return payment.tool_name;
    if (payment.resource) {
      try {
        const url = new URL(payment.resource);
        return url.hostname;
      } catch {
        return payment.resource.split('/')[0] || 'Service';
      }
    }
    return 'Service';
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  const shortenAddress = (addr: string) => {
    if (addr.length <= 12) return addr;
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
  };

  const shortenTxHash = (hash: string) => {
    if (hash.length <= 16) return hash;
    return `${hash.slice(0, 10)}...${hash.slice(-6)}`;
  };

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white mb-2">x402 Payments</h1>
        <p className="text-slate-400">Track micropayments made through the x402 protocol</p>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
        <Card>
          <CardContent>
            <div className="flex items-center gap-4">
              <div className="p-3 rounded-lg bg-green-500/20">
                <DollarSign className="w-6 h-6 text-green-400" />
              </div>
              <div>
                <p className="text-2xl font-bold text-white">
                  {summaryLoading ? '...' : `$${filter === 'all' ? (summary?.total_usdc_spent ?? '0.00') : filteredTotal}`}
                </p>
                <p className="text-sm text-slate-400">
                  {filter === 'all' ? 'Total Spent (USDC)' : 'Filtered Total (USDC)'}
                </p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent>
            <div className="flex items-center gap-4">
              <div className="p-3 rounded-lg bg-blue-500/20">
                <Receipt className="w-6 h-6 text-blue-400" />
              </div>
              <div>
                <p className="text-2xl font-bold text-white">
                  {summaryLoading ? '...' : summary?.total_payments ?? 0}
                </p>
                <p className="text-sm text-slate-400">Total Transactions</p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent>
            <div className="flex items-center gap-4">
              <div className="p-3 rounded-lg bg-purple-500/20">
                <TrendingUp className="w-6 h-6 text-purple-400" />
              </div>
              <div>
                <p className="text-2xl font-bold text-white">
                  {summaryLoading ? '...' : summary?.payments_with_feedback ?? 0}
                </p>
                <p className="text-sm text-slate-400">With Feedback</p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent>
            <div className="flex items-center gap-4">
              <div className="p-3 rounded-lg bg-amber-500/20">
                <Clock className="w-6 h-6 text-amber-400" />
              </div>
              <div>
                <p className="text-2xl font-bold text-white">
                  {summaryLoading ? '...' : summary?.payments_without_feedback ?? 0}
                </p>
                <p className="text-sm text-slate-400">Pending Feedback</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Filter Tabs */}
      <div className="flex gap-2 mb-6">
        <Button
          variant={filter === 'all' ? 'primary' : 'secondary'}
          size="sm"
          onClick={() => setFilter('all')}
        >
          All
        </Button>
        <Button
          variant={filter === 'with_feedback' ? 'primary' : 'secondary'}
          size="sm"
          onClick={() => setFilter('with_feedback')}
        >
          With Feedback
        </Button>
        <Button
          variant={filter === 'without_feedback' ? 'primary' : 'secondary'}
          size="sm"
          onClick={() => setFilter('without_feedback')}
        >
          Pending Feedback
        </Button>
      </div>

      {/* Payments Table */}
      <Card>
        <CardContent>
          {paymentsLoading ? (
            <div className="text-center py-8 text-slate-400">Loading payments...</div>
          ) : filteredPayments.length === 0 ? (
            <div className="text-center py-8 text-slate-400">
              No payments found. Payments will appear here when you use x402-enabled services.
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-slate-700">
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">Date</th>
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">Tool</th>
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">Amount</th>
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">Recipient</th>
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">Status</th>
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">TX Hash</th>
                    <th className="text-left py-3 px-4 text-slate-400 font-medium">Feedback</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredPayments.map((payment) => (
                    <tr key={payment.id} className="border-b border-slate-700/50 hover:bg-slate-700/30">
                      <td className="py-3 px-4 text-slate-300 text-sm">
                        {formatDate(payment.created_at)}
                      </td>
                      <td className="py-3 px-4">
                        <span className="text-white font-medium">{getToolName(payment)}</span>
                        {payment.resource && (
                          <span className="block text-xs text-slate-500 truncate max-w-[200px]">
                            {payment.resource}
                          </span>
                        )}
                      </td>
                      <td className="py-3 px-4">
                        <span className="text-green-400 font-mono">
                          {payment.amount_formatted} {payment.asset}
                        </span>
                      </td>
                      <td className="py-3 px-4">
                        <span className="text-slate-300 font-mono text-sm">
                          {shortenAddress(payment.pay_to)}
                        </span>
                      </td>
                      <td className="py-3 px-4">
                        {payment.status === 'confirmed' ? (
                          <span className="px-2 py-1 bg-green-500/20 text-green-400 rounded text-xs">
                            Confirmed
                          </span>
                        ) : payment.status === 'failed' ? (
                          <span className="px-2 py-1 bg-red-500/20 text-red-400 rounded text-xs">
                            Failed
                          </span>
                        ) : (
                          <span className="px-2 py-1 bg-amber-500/20 text-amber-400 rounded text-xs">
                            Pending
                          </span>
                        )}
                      </td>
                      <td className="py-3 px-4">
                        {payment.tx_hash ? (
                          <a
                            href={`https://basescan.org/tx/${payment.tx_hash}`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="flex items-center gap-1 text-stark-400 hover:text-stark-300 font-mono text-sm"
                          >
                            {shortenTxHash(payment.tx_hash)}
                            <ExternalLink className="w-3 h-3" />
                          </a>
                        ) : (
                          <span className="text-slate-500">-</span>
                        )}
                      </td>
                      <td className="py-3 px-4">
                        {payment.feedback_submitted ? (
                          <span className="px-2 py-1 bg-green-500/20 text-green-400 rounded text-xs">
                            Submitted
                          </span>
                        ) : (
                          <span className="px-2 py-1 bg-amber-500/20 text-amber-400 rounded text-xs">
                            Pending
                          </span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
