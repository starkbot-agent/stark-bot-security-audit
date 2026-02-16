import { useState, useEffect, useRef, useCallback } from 'react';
import { GatewayClient, getGateway } from '@/lib/gateway-client';

interface UseGatewayReturn {
  connected: boolean;
  gateway: GatewayClient;
  connect: () => Promise<void>;
  disconnect: () => void;
  call: <T = unknown>(method: string, params?: Record<string, unknown>) => Promise<T>;
  on: (event: string, callback: (data: unknown) => void) => void;
  off: (event: string, callback: (data: unknown) => void) => void;
}

export function useGateway(): UseGatewayReturn {
  const [connected, setConnected] = useState(false);
  const gatewayRef = useRef<GatewayClient>(getGateway());
  const listenersRef = useRef<Map<string, Set<(data: unknown) => void>>>(new Map());

  useEffect(() => {
    const gateway = gatewayRef.current;

    const handleConnected = () => setConnected(true);
    const handleDisconnected = () => setConnected(false);

    gateway.on('connected', handleConnected);
    gateway.on('disconnected', handleDisconnected);

    // Auto-connect
    gateway.connect().catch(console.error);

    // Check initial state
    setConnected(gateway.isConnected());

    return () => {
      gateway.off('connected', handleConnected);
      gateway.off('disconnected', handleDisconnected);

      // Clean up all registered listeners
      listenersRef.current.forEach((callbacks, event) => {
        callbacks.forEach((callback) => {
          gateway.off(event, callback);
        });
      });
      listenersRef.current.clear();
    };
  }, []);

  const connect = useCallback(async () => {
    await gatewayRef.current.connect();
  }, []);

  const disconnect = useCallback(() => {
    gatewayRef.current.disconnect();
  }, []);

  const call = useCallback(<T = unknown>(method: string, params?: Record<string, unknown>) => {
    return gatewayRef.current.call<T>(method, params);
  }, []);

  const on = useCallback((event: string, callback: (data: unknown) => void) => {
    gatewayRef.current.on(event, callback);

    // Track listener for cleanup
    if (!listenersRef.current.has(event)) {
      listenersRef.current.set(event, new Set());
    }
    listenersRef.current.get(event)!.add(callback);
  }, []);

  const off = useCallback((event: string, callback: (data: unknown) => void) => {
    gatewayRef.current.off(event, callback);
    listenersRef.current.get(event)?.delete(callback);
  }, []);

  return {
    connected,
    gateway: gatewayRef.current,
    connect,
    disconnect,
    call,
    on,
    off,
  };
}
