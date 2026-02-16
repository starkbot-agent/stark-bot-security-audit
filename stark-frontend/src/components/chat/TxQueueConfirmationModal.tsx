import { useState, useEffect } from 'react';
import Modal from '../ui/Modal';
import Button from '../ui/Button';
import { AlertTriangle, Check, X, Loader2, ExternalLink, Code, FlaskConical, FileCode2, ChevronDown, ChevronRight } from 'lucide-react';
import { getGateway } from '@/lib/gateway-client';
import { decodeCalldata, DecodedFunction } from '@/lib/abi-decoder';

// Get explorer URL for address based on network
function getAddressExplorerUrl(network: string, address: string): string {
  if (network === 'mainnet') {
    return `https://etherscan.io/address/${address}`;
  }
  return `https://basescan.org/address/${address}`;
}

// Extract function selector (first 4 bytes / 8 hex chars) from calldata
function extractFunctionSelector(data: string): string | null {
  const cleaned = data.startsWith('0x') ? data.slice(2) : data;
  if (cleaned.length < 8) return null;
  return '0x' + cleaned.slice(0, 8).toLowerCase();
}

// Lookup function signature using OpenChain API
async function lookupFunctionSignature(selector: string): Promise<string | null> {
  try {
    const response = await fetch(
      `https://api.openchain.xyz/signature-database/v1/lookup?function=${selector}&filter=true`
    );
    if (!response.ok) return null;

    const data = await response.json();
    // API returns { result: { function: { "0x...": [{ name: "transfer(address,uint256)", ... }] } } }
    const functions = data?.result?.function?.[selector];
    if (functions && functions.length > 0) {
      // Return the first (most likely) match
      return functions[0].name;
    }
    return null;
  } catch {
    return null;
  }
}

// Parse function name from signature (e.g. "transfer(address,uint256)" -> "transfer")
function parseFunctionName(signature: string): string {
  const match = signature.match(/^([a-zA-Z_][a-zA-Z0-9_]*)/);
  return match ? match[1] : signature;
}

export interface TxQueueTransaction {
  uuid: string;
  network: string;
  from?: string;
  to: string;
  value: string;
  value_formatted: string;
  /** Hex-encoded calldata for function selector lookup */
  data?: string;
}

// Get Tenderly simulation URL
// Format based on: https://dashboard.tenderly.co/.../simulator/new?...
function getTenderlySimulationUrl(
  network: string,
  from: string,
  to: string,
  value: string,
  data: string
): string {
  // Chain IDs: mainnet = 1, base = 8453
  const chainId = network === 'mainnet' ? 1 : 8453;
  const hasData = data && data !== '0x' && data.length >= 10;

  // Extract function selector (first 4 bytes / 10 chars including 0x)
  const contractFunction = hasData ? data.slice(0, 10) : '';

  const params = new URLSearchParams();
  params.set('network', String(chainId));
  params.set('contractAddress', to);
  params.set('from', from);
  params.set('value', value || '0');
  params.set('gas', '8000000');
  params.set('gasPrice', '0');
  params.set('block', '');
  params.set('blockIndex', '0');

  if (hasData) {
    params.set('contractFunction', contractFunction);
    params.set('rawFunctionInput', data);
  }

  return `https://dashboard.tenderly.co/simulator/new?${params.toString()}`;
}

interface TxQueueConfirmationModalProps {
  isOpen: boolean;
  onClose: () => void;
  channelId: number;
  transaction: TxQueueTransaction | null;
}

interface FunctionInfo {
  selector: string;
  signature: string | null;
  name: string;
  loading: boolean;
}

// Component for rendering decoded parameters
function DecodedParams({ decoded, network }: { decoded: DecodedFunction; network: string }) {
  const [expanded, setExpanded] = useState(true);

  return (
    <div className="mt-2 space-y-2">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 text-slate-400 text-xs hover:text-slate-300 transition-colors"
      >
        {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
        <FileCode2 className="w-3 h-3" />
        <span>Decoded from {decoded.abiName} ABI</span>
      </button>

      {expanded && (
        <div className="bg-slate-800/70 rounded-md p-3 space-y-2">
          {decoded.params.length === 0 ? (
            <span className="text-slate-500 text-xs italic">No parameters</span>
          ) : (
            decoded.params.map((param, i) => (
              <div key={i} className="flex flex-col gap-0.5">
                <div className="flex items-center gap-2">
                  <span className="text-purple-400 font-mono text-xs">{param.name}</span>
                  <span className="text-slate-600 text-xs">({param.type})</span>
                </div>
                <div className="text-slate-200 font-mono text-xs break-all pl-2 border-l-2 border-slate-600">
                  {param.type === 'address' ? (
                    <a
                      href={getAddressExplorerUrl(network, param.value)}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-cyan-400 hover:text-cyan-300 flex items-center gap-1"
                    >
                      {param.value}
                      <ExternalLink className="w-3 h-3 flex-shrink-0" />
                    </a>
                  ) : (
                    param.value
                  )}
                </div>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

export default function TxQueueConfirmationModal({
  isOpen,
  onClose,
  channelId,
  transaction
}: TxQueueConfirmationModalProps) {
  const [isLoading, setIsLoading] = useState<'confirm' | 'deny' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [functionInfo, setFunctionInfo] = useState<FunctionInfo | null>(null);
  const [decodedFunction, setDecodedFunction] = useState<DecodedFunction | null>(null);
  const [showRawCalldata, setShowRawCalldata] = useState(false);

  // Auto-clear error after 3 seconds
  useEffect(() => {
    if (error) {
      const timer = setTimeout(() => setError(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [error]);

  // Lookup function signature and try to decode when transaction data changes
  useEffect(() => {
    if (!transaction?.data || transaction.data === '0x' || transaction.data === '') {
      setFunctionInfo(null);
      setDecodedFunction(null);
      return;
    }

    const selector = extractFunctionSelector(transaction.data);
    if (!selector) {
      setFunctionInfo(null);
      setDecodedFunction(null);
      return;
    }

    // Try to decode using our local ABIs first (synchronous)
    const decoded = decodeCalldata(transaction.data);
    setDecodedFunction(decoded);

    // If we decoded successfully, use that info
    if (decoded) {
      setFunctionInfo({
        selector,
        signature: decoded.signature,
        name: decoded.functionName,
        loading: false
      });
      return;
    }

    // Otherwise, fall back to OpenChain lookup
    setFunctionInfo({
      selector,
      signature: null,
      name: 'Looking up...',
      loading: true
    });

    lookupFunctionSignature(selector).then(signature => {
      if (signature) {
        setFunctionInfo({
          selector,
          signature,
          name: parseFunctionName(signature),
          loading: false
        });
      } else {
        setFunctionInfo({
          selector,
          signature: null,
          name: 'Unknown Function',
          loading: false
        });
      }
    });
  }, [transaction?.data]);

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

  // Check if this is a simple ETH transfer (no data)
  const isSimpleTransfer = !transaction.data || transaction.data === '0x' || transaction.data === '';

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
          {/* Function Call Info - shown prominently if there's calldata */}
          {!isSimpleTransfer && (
            <div className="flex flex-col gap-1 pb-3 border-b border-slate-600">
              <div className="flex items-center gap-2">
                <Code className="w-4 h-4 text-purple-400" />
                <span className="text-slate-400 text-sm">Function Call</span>
                {decodedFunction && (
                  <span className="bg-green-600/20 text-green-400 px-1.5 py-0.5 rounded text-xs">
                    Decoded
                  </span>
                )}
              </div>
              {functionInfo?.loading ? (
                <div className="flex items-center gap-2">
                  <Loader2 className="w-4 h-4 animate-spin text-slate-400" />
                  <span className="text-slate-300 text-sm">Looking up function...</span>
                </div>
              ) : functionInfo?.signature ? (
                <div className="space-y-1">
                  <span className="text-purple-300 font-semibold text-lg">
                    {functionInfo.name}()
                  </span>
                  <div className="text-slate-400 font-mono text-xs break-all">
                    {functionInfo.signature}
                  </div>
                  {/* Show decoded parameters if available */}
                  {decodedFunction && <DecodedParams decoded={decodedFunction} network={transaction.network} />}
                </div>
              ) : functionInfo ? (
                <div className="space-y-1">
                  <span className="text-amber-300 font-medium">
                    Unknown Function
                  </span>
                  <div className="text-slate-500 font-mono text-xs">
                    Selector: {functionInfo.selector}
                  </div>
                </div>
              ) : null}
            </div>
          )}

          {/* Simple Transfer Badge */}
          {isSimpleTransfer && (
            <div className="flex items-center gap-2 pb-3 border-b border-slate-600">
              <span className="bg-green-600/20 text-green-400 px-2 py-1 rounded text-sm font-medium">
                ETH Transfer
              </span>
            </div>
          )}

          <div className="flex justify-between">
            <span className="text-slate-400">Network</span>
            <span className="text-white font-mono uppercase">{transaction.network}</span>
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-slate-400">{decodedFunction ? 'Contract' : 'To'}</span>
            <a
              href={getAddressExplorerUrl(transaction.network, transaction.to)}
              target="_blank"
              rel="noopener noreferrer"
              className="text-cyan-400 hover:text-cyan-300 font-mono text-xs break-all flex items-center gap-1"
            >
              {transaction.to}
              <ExternalLink className="w-3 h-3 flex-shrink-0" />
            </a>
          </div>
          <div className="flex justify-between">
            <span className="text-slate-400">Value</span>
            <span className="text-white font-medium">{transaction.value_formatted}</span>
          </div>

          {/* Show calldata - collapsed by default if decoded, expanded if not */}
          {!isSimpleTransfer && transaction.data && (
            <div className="flex flex-col gap-1">
              <button
                onClick={() => setShowRawCalldata(!showRawCalldata)}
                className="flex items-center gap-1 text-slate-400 text-sm hover:text-slate-300 transition-colors"
              >
                {showRawCalldata ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                <span>Raw Calldata</span>
                <span className="text-slate-500 text-xs">({transaction.data.length} chars)</span>
              </button>
              {showRawCalldata && (
                <div className="text-slate-500 font-mono text-xs break-all max-h-24 overflow-y-auto bg-slate-800/50 p-2 rounded">
                  {transaction.data}
                </div>
              )}
            </div>
          )}

          {/* Tenderly Simulation Link */}
          {transaction.from && (
            <div className="pt-2 border-t border-slate-600">
              <a
                href={getTenderlySimulationUrl(
                  transaction.network,
                  transaction.from,
                  transaction.to,
                  transaction.value,
                  transaction.data || '0x'
                )}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 text-sm text-orange-400 hover:text-orange-300 transition-colors"
              >
                <FlaskConical className="w-4 h-4" />
                <span>Simulate on Tenderly</span>
                <ExternalLink className="w-3 h-3" />
              </a>
            </div>
          )}
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
