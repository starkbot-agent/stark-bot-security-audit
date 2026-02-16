import { useState, useEffect, useCallback } from 'react';
import { getGateway } from '@/lib/gateway-client';

/**
 * Hook that listens for heartbeat events and returns a pulsing state.
 * Use this to trigger animations when heartbeats occur.
 */
export function useHeartbeatPulse() {
  const [isPulsing, setIsPulsing] = useState(false);

  const triggerPulse = useCallback(() => {
    setIsPulsing(true);
    // Reset after animation completes (matches the heartbeat animation duration)
    setTimeout(() => setIsPulsing(false), 1200);
  }, []);

  useEffect(() => {
    const gateway = getGateway();

    const handleHeartbeatStarted = () => {
      triggerPulse();
    };

    const handlePulseStarted = () => {
      triggerPulse();
    };

    gateway.on('heartbeat_started', handleHeartbeatStarted);
    gateway.on('heartbeat_pulse_started', handlePulseStarted);

    // Ensure gateway is connected
    gateway.connect().catch(e => {
      console.error('[useHeartbeatPulse] Failed to connect to gateway:', e);
    });

    return () => {
      gateway.off('heartbeat_started', handleHeartbeatStarted);
      gateway.off('heartbeat_pulse_started', handlePulseStarted);
    };
  }, [triggerPulse]);

  return { isPulsing, triggerPulse };
}
