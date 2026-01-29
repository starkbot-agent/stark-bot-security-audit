import { useState, useEffect, useCallback } from 'react';
import { BrowserProvider, Contract, formatUnits } from 'ethers';

// Base network configuration
const BASE_CHAIN_ID_HEX = '0x2105';

// USDC on Base
const USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
const USDC_DECIMALS = 6;

// Minimal ERC20 ABI for balance check
const ERC20_ABI = [
  'function balanceOf(address owner) view returns (uint256)',
];

interface WalletState {
  address: string | null;
  usdcBalance: string | null;
  isConnected: boolean;
  isLoading: boolean;
  error: string | null;
  isCorrectNetwork: boolean;
}

export function useWallet() {
  const [state, setState] = useState<WalletState>({
    address: null,
    usdcBalance: null,
    isConnected: false,
    isLoading: true,
    error: null,
    isCorrectNetwork: false,
  });

  const checkNetwork = useCallback(async (): Promise<boolean> => {
    if (!window.ethereum) return false;
    try {
      const chainId = await window.ethereum.request({ method: 'eth_chainId' });
      return chainId === BASE_CHAIN_ID_HEX;
    } catch {
      return false;
    }
  }, []);

  const switchToBase = useCallback(async () => {
    if (!window.ethereum) return false;
    try {
      await window.ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: BASE_CHAIN_ID_HEX }],
      });
      return true;
    } catch (switchError: unknown) {
      // Chain not added, try to add it
      if ((switchError as { code?: number })?.code === 4902) {
        try {
          await window.ethereum.request({
            method: 'wallet_addEthereumChain',
            params: [{
              chainId: BASE_CHAIN_ID_HEX,
              chainName: 'Base',
              nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
              rpcUrls: ['https://mainnet.base.org'],
              blockExplorerUrls: ['https://basescan.org'],
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

  const fetchUsdcBalance = useCallback(async (address: string): Promise<string | null> => {
    if (!window.ethereum) return null;
    try {
      const provider = new BrowserProvider(window.ethereum);
      const usdcContract = new Contract(USDC_ADDRESS, ERC20_ABI, provider);
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
      const isCorrectNetwork = await checkNetwork();

      // Switch to Base if not on it
      if (!isCorrectNetwork) {
        await switchToBase();
      }

      const finalNetworkCheck = await checkNetwork();
      let usdcBalance: string | null = null;

      if (finalNetworkCheck) {
        usdcBalance = await fetchUsdcBalance(address);
      }

      setState({
        address,
        usdcBalance,
        isConnected: true,
        isLoading: false,
        error: null,
        isCorrectNetwork: finalNetworkCheck,
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
    if (!state.address || !state.isCorrectNetwork) return;

    const usdcBalance = await fetchUsdcBalance(state.address);
    setState(prev => ({ ...prev, usdcBalance }));
  }, [state.address, state.isCorrectNetwork, fetchUsdcBalance]);

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
          const isCorrectNetwork = await checkNetwork();
          let usdcBalance: string | null = null;

          if (isCorrectNetwork) {
            usdcBalance = await fetchUsdcBalance(address);
          }

          setState({
            address,
            usdcBalance,
            isConnected: true,
            isLoading: false,
            error: null,
            isCorrectNetwork,
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
        });
      } else {
        const address = accounts[0].toLowerCase();
        const isCorrectNetwork = await checkNetwork();
        let usdcBalance: string | null = null;

        if (isCorrectNetwork) {
          usdcBalance = await fetchUsdcBalance(address);
        }

        setState(prev => ({
          ...prev,
          address,
          usdcBalance,
          isConnected: true,
          isCorrectNetwork,
        }));
      }
    };

    const handleChainChanged = async () => {
      const isCorrectNetwork = await checkNetwork();

      if (state.address && isCorrectNetwork) {
        const usdcBalance = await fetchUsdcBalance(state.address);
        setState(prev => ({ ...prev, isCorrectNetwork, usdcBalance }));
      } else {
        setState(prev => ({ ...prev, isCorrectNetwork, usdcBalance: null }));
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
  };
}

