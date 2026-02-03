import { useState, useEffect, useCallback } from 'react';
import { BrowserProvider, Contract, formatUnits } from 'ethers';

// Supported networks configuration
type SupportedNetwork = 'mainnet' | 'base' | 'polygon';

interface NetworkConfig {
  chainIdHex: string;
  chainIdDecimal: number;
  name: string;
  displayName: string;
  usdcAddress: string;
  nativeCurrency: { name: string; symbol: string; decimals: number };
  rpcUrls: string[];
  blockExplorerUrls: string[];
}

const SUPPORTED_NETWORKS: Record<SupportedNetwork, NetworkConfig> = {
  mainnet: {
    chainIdHex: '0x1',
    chainIdDecimal: 1,
    name: 'mainnet',
    displayName: 'Mainnet',
    usdcAddress: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
    nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
    rpcUrls: ['https://eth.llamarpc.com'],
    blockExplorerUrls: ['https://etherscan.io'],
  },
  base: {
    chainIdHex: '0x2105',
    chainIdDecimal: 8453,
    name: 'base',
    displayName: 'Base',
    usdcAddress: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
    nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
    rpcUrls: ['https://mainnet.base.org'],
    blockExplorerUrls: ['https://basescan.org'],
  },
  polygon: {
    chainIdHex: '0x89',
    chainIdDecimal: 137,
    name: 'polygon',
    displayName: 'Polygon',
    usdcAddress: '0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359',
    nativeCurrency: { name: 'MATIC', symbol: 'MATIC', decimals: 18 },
    rpcUrls: ['https://polygon-rpc.com'],
    blockExplorerUrls: ['https://polygonscan.com'],
  },
};

const USDC_DECIMALS = 6;

// Minimal ERC20 ABI for balance check
const ERC20_ABI = [
  'function balanceOf(address owner) view returns (uint256)',
];

// Helper to get network config from chain ID
function getNetworkFromChainId(chainIdHex: string): NetworkConfig | null {
  for (const network of Object.values(SUPPORTED_NETWORKS)) {
    if (network.chainIdHex.toLowerCase() === chainIdHex.toLowerCase()) {
      return network;
    }
  }
  return null;
}

interface WalletState {
  address: string | null;
  usdcBalance: string | null;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  isCorrectNetwork: boolean;
  currentNetwork: NetworkConfig | null;
}

export function useWallet() {
  const [state, setState] = useState<WalletState>({
    address: null,
    usdcBalance: null,
    isConnected: false,
    isLoading: true,
    error: null,
    isCorrectNetwork: false,
    currentNetwork: null,
  });

  const checkNetwork = useCallback(async (): Promise<{ isSupported: boolean; network: NetworkConfig | null }> => {
    if (!window.ethereum) return { isSupported: false, network: null };
    try {
      const chainId = await window.ethereum.request({ method: 'eth_chainId' }) as string;
      const network = getNetworkFromChainId(chainId);
      return { isSupported: network !== null, network };
    } catch {
      return { isSupported: false, network: null };
    }
  }, []);

  const switchNetwork = useCallback(async (networkName: SupportedNetwork) => {
    if (!window.ethereum) return false;
    const networkConfig = SUPPORTED_NETWORKS[networkName];
    try {
      await window.ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: networkConfig.chainIdHex }],
      });
      return true;
    } catch (switchError: unknown) {
      // Chain not added, try to add it (not needed for mainnet, but for Base/Polygon)
      if ((switchError as { code?: number })?.code === 4902) {
        try {
          await window.ethereum.request({
            method: 'wallet_addEthereumChain',
            params: [{
              chainId: networkConfig.chainIdHex,
              chainName: networkConfig.displayName,
              nativeCurrency: networkConfig.nativeCurrency,
              rpcUrls: networkConfig.rpcUrls,
              blockExplorerUrls: networkConfig.blockExplorerUrls,
            }],
          });
          return true;
        } catch {
          return false;
        }
      }
      return false;
    }
  }, []);

  const switchToBase = useCallback(async () => {
    return switchNetwork('base');
  }, [switchNetwork]);

  const fetchUsdcBalance = useCallback(async (address: string, network: NetworkConfig): Promise<string | null> => {
    if (!window.ethereum) return null;
    try {
      const provider = new BrowserProvider(window.ethereum);
      const usdcContract = new Contract(network.usdcAddress, ERC20_ABI, provider);
      const balance = await usdcContract.balanceOf(address);
      return formatUnits(balance, USDC_DECIMALS);
    } catch (err) {
      console.error('Failed to fetch USDC balance:', err);
      return null;
    }
  }, []);

  const connect = useCallback(async () => {
    if (!window.ethereum) {
      setState(prev => ({ ...prev, error: 'No wallet found', isLoading: false }));
      return;
    }

    setState(prev => ({ ...prev, isLoading: true, error: null }));

    try {
      const provider = new BrowserProvider(window.ethereum);
      const accounts = await provider.send('eth_requestAccounts', []);

      if (!accounts || accounts.length === 0) {
        throw new Error('No accounts found');
      }

      const address = accounts[0].toLowerCase();
      let { isSupported, network } = await checkNetwork();

      // Switch to Base if not on a supported network
      if (!isSupported) {
        await switchToBase();
        const result = await checkNetwork();
        isSupported = result.isSupported;
        network = result.network;
      }

      let usdcBalance: string | null = null;
      if (isSupported && network) {
        usdcBalance = await fetchUsdcBalance(address, network);
      }

      setState({
        address,
        usdcBalance,
        isConnected: true,
        isLoading: false,
        error: null,
        isCorrectNetwork: isSupported,
        currentNetwork: network,
      });
    } catch (err) {
      console.error('Wallet connection error:', err);
      setState(prev => ({
        ...prev,
        isLoading: false,
        error: err instanceof Error ? err.message : 'Connection failed',
      }));
    }
  }, [checkNetwork, switchToBase, fetchUsdcBalance]);

  const refreshBalance = useCallback(async () => {
    if (!state.address || !state.isCorrectNetwork || !state.currentNetwork) return;

    const usdcBalance = await fetchUsdcBalance(state.address, state.currentNetwork);
    setState(prev => ({ ...prev, usdcBalance }));
  }, [state.address, state.isCorrectNetwork, state.currentNetwork, fetchUsdcBalance]);

  // Auto-connect on mount if wallet was previously connected
  useEffect(() => {
    const autoConnect = async () => {
      if (!window.ethereum) {
        setState(prev => ({ ...prev, isLoading: false }));
        return;
      }

      try {
        const accounts = await window.ethereum.request({ method: 'eth_accounts' }) as string[];

        if (accounts && accounts.length > 0) {
          const address = accounts[0].toLowerCase();
          const { isSupported, network } = await checkNetwork();
          let usdcBalance: string | null = null;

          if (isSupported && network) {
            usdcBalance = await fetchUsdcBalance(address, network);
          }

          setState({
            address,
            usdcBalance,
            isConnected: true,
            isLoading: false,
            error: null,
            isCorrectNetwork: isSupported,
            currentNetwork: network,
          });
        } else {
          setState(prev => ({ ...prev, isLoading: false }));
        }
      } catch {
        setState(prev => ({ ...prev, isLoading: false }));
      }
    };

    autoConnect();
  }, [checkNetwork, fetchUsdcBalance]);

  // Auto-refresh USDC balance every 10 seconds
  useEffect(() => {
    if (!state.address || !state.isCorrectNetwork || !state.isConnected || !state.currentNetwork) return;

    const network = state.currentNetwork;
    const intervalId = setInterval(() => {
      fetchUsdcBalance(state.address!, network).then(usdcBalance => {
        if (usdcBalance !== null) {
          setState(prev => ({ ...prev, usdcBalance }));
        }
      });
    }, 10000); // 10 seconds

    return () => clearInterval(intervalId);
  }, [state.address, state.isCorrectNetwork, state.isConnected, state.currentNetwork, fetchUsdcBalance]);

  // Listen for account and network changes
  useEffect(() => {
    if (!window.ethereum) return;

    const handleAccountsChanged = async (...args: unknown[]) => {
      const accounts = args[0] as string[];
      if (!accounts || accounts.length === 0) {
        setState({
          address: null,
          usdcBalance: null,
          isConnected: false,
          isLoading: false,
          error: null,
          isCorrectNetwork: false,
          currentNetwork: null,
        });
      } else {
        const address = accounts[0].toLowerCase();
        const { isSupported, network } = await checkNetwork();
        let usdcBalance: string | null = null;

        if (isSupported && network) {
          usdcBalance = await fetchUsdcBalance(address, network);
        }

        setState(prev => ({
          ...prev,
          address,
          usdcBalance,
          isConnected: true,
          isCorrectNetwork: isSupported,
          currentNetwork: network,
        }));
      }
    };

    const handleChainChanged = async () => {
      const { isSupported, network } = await checkNetwork();

      if (state.address && isSupported && network) {
        const usdcBalance = await fetchUsdcBalance(state.address, network);
        setState(prev => ({ ...prev, isCorrectNetwork: isSupported, currentNetwork: network, usdcBalance }));
      } else {
        setState(prev => ({ ...prev, isCorrectNetwork: isSupported, currentNetwork: network, usdcBalance: null }));
      }
    };

    window.ethereum.on?.('accountsChanged', handleAccountsChanged);
    window.ethereum.on?.('chainChanged', handleChainChanged);

    return () => {
      window.ethereum?.removeListener?.('accountsChanged', handleAccountsChanged);
      window.ethereum?.removeListener?.('chainChanged', handleChainChanged);
    };
  }, [state.address, checkNetwork, fetchUsdcBalance]);

  return {
    ...state,
    connect,
    refreshBalance,
    switchToBase,
    switchNetwork,
  };
}

// Export for use in UI components
export { SUPPORTED_NETWORKS };
export type { SupportedNetwork, NetworkConfig };

