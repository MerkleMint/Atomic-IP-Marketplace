import React, { useState, useEffect } from 'react';
import { Lock, Unlock, Clock, AlertTriangle, CheckCircle } from 'lucide-react';
import { useWallet } from '../context/WalletContext';
import { pauseAtomicSwap, unpauseAtomicSwap, isAtomicSwapPaused } from '../lib/contractClient';
import { TimelockOperation } from '../lib/adminTypes';

const TIMELOCK_DELAY_SECONDS = Number(import.meta.env.VITE_TIMELOCK_DELAY_SECONDS) || 3600; // 1 hour timelock default

function PauseSwapControl() {
  const { wallet } = useWallet();
  const [isPaused, setIsPaused] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [pendingTimelock, setPendingTimelock] = useState<TimelockOperation | null>(null);
  const [timelockRemaining, setTimelockRemaining] = useState(0);

  useEffect(() => {
    loadPauseStatus();
    loadPersistedTimelock();
  }, []);

  useEffect(() => {
    let interval: NodeJS.Timeout;
    if (pendingTimelock && pendingTimelock.status === 'pending') {
      interval = setInterval(() => {
        const now = Math.floor(Date.now() / 1000);
        const remaining = Math.max(0, pendingTimelock.executeAt - now);
        setTimelockRemaining(remaining);
        
        if (remaining === 0) {
          clearInterval(interval);
        }
      }, 1000);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [pendingTimelock]);

  const loadPauseStatus = async () => {
    try {
      const paused = await isAtomicSwapPaused();
      setIsPaused(paused);
    } catch (err) {
      console.error('Failed to load pause status:', err);
    }
  };

  const loadPersistedTimelock = () => {
    try {
      const stored = localStorage.getItem('pendingTimelock');
      if (stored) {
        const timelock = JSON.parse(stored) as TimelockOperation;
        const now = Math.floor(Date.now() / 1000);
        
        // Check if timelock has expired
        if (timelock.executeAt > now && timelock.status === 'pending') {
          setPendingTimelock(timelock);
          setTimelockRemaining(timelock.executeAt - now);
        } else {
          // Clear expired timelock
          localStorage.removeItem('pendingTimelock');
        }
      }
    } catch (err) {
      console.error('Failed to load persisted timelock:', err);
      localStorage.removeItem('pendingTimelock');
    }
  };

  const initiatePause = async () => {
    if (!wallet) return;
    
    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      // Create timelock operation
      const now = Math.floor(Date.now() / 1000);
      const timelockOp: TimelockOperation = {
        id: `pause-${now}`,
        type: 'pause',
        targetContract: 'atomic_swap',
        proposedBy: wallet.address,
        proposedAt: now,
        executeAt: now + TIMELOCK_DELAY_SECONDS,
        status: 'pending',
        approvals: [wallet.address],
        requiredApprovals: 1,
      };

      setPendingTimelock(timelockOp);
      localStorage.setItem('pendingTimelock', JSON.stringify(timelockOp));
      setSuccess('Pause operation initiated. Timelock active.');
    } catch (err: any) {
      setError(err.message || 'Failed to initiate pause');
    } finally {
      setLoading(false);
    }
  };

  const executePause = async () => {
    if (!wallet || !pendingTimelock) return;

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      await pauseAtomicSwap(wallet);
      setIsPaused(true);
      setPendingTimelock(null);
      localStorage.removeItem('pendingTimelock');
      setSuccess('Contract paused successfully');
      
      // Refresh contract state
      await loadPauseStatus();
      
      // Log to audit
      await logAuditAction('pause_contract', 'atomic_swap', { timelockId: pendingTimelock.id });
    } catch (err: any) {
      setError(err.message || 'Failed to pause contract');
    } finally {
      setLoading(false);
    }
  };

  const initiateUnpause = async () => {
    if (!wallet) return;
    
    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      // Create timelock operation
      const now = Math.floor(Date.now() / 1000);
      const timelockOp: TimelockOperation = {
        id: `unpause-${now}`,
        type: 'unpause',
        targetContract: 'atomic_swap',
        proposedBy: wallet.address,
        proposedAt: now,
        executeAt: now + TIMELOCK_DELAY_SECONDS,
        status: 'pending',
        approvals: [wallet.address],
        requiredApprovals: 1,
      };

      setPendingTimelock(timelockOp);
      localStorage.setItem('pendingTimelock', JSON.stringify(timelockOp));
      setSuccess('Unpause operation initiated. Timelock active.');
    } catch (err: any) {
      setError(err.message || 'Failed to initiate unpause');
    } finally {
      setLoading(false);
    }
  };

  const executeUnpause = async () => {
    if (!wallet || !pendingTimelock) return;

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      await unpauseAtomicSwap(wallet);
      setIsPaused(false);
      setPendingTimelock(null);
      localStorage.removeItem('pendingTimelock');
      setSuccess('Contract unpaused successfully');
      
      // Refresh contract state
      await loadPauseStatus();
      
      // Log to audit
      await logAuditAction('unpause_contract', 'atomic_swap', { timelockId: pendingTimelock.id });
    } catch (err: any) {
      setError(err.message || 'Failed to unpause contract');
    } finally {
      setLoading(false);
    }
  };

  const cancelTimelock = () => {
    setPendingTimelock(null);
    setTimelockRemaining(0);
    localStorage.removeItem('pendingTimelock');
    setSuccess('Timelock operation cancelled');
  };

  const logAuditAction = async (action: string, targetContract: string, details: any) => {
    // In production, this would send to a backend or on-chain log
    const auditLog = {
      id: `${Date.now()}-${Math.random()}`,
      timestamp: Math.floor(Date.now() / 1000),
      admin: wallet?.address,
      action,
      targetContract: targetContract as 'atomic_swap' | 'ip_registry',
      details,
    };
    
    // Store in localStorage for demo
    try {
      const logs = JSON.parse(localStorage.getItem('adminAuditLogs') || '[]');
      logs.unshift(auditLog);
      localStorage.setItem('adminAuditLogs', JSON.stringify(logs.slice(0, 100)));
    } catch (err) {
      console.error('Failed to save audit log:', err);
    }
  };

  const formatTime = (seconds: number) => {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hrs}h ${mins}m ${secs}s`;
  };

  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Pause/Resume Control</h2>
      
      {/* Current Status */}
      <div className={`mb-6 p-4 rounded-lg border ${
        isPaused 
          ? 'bg-yellow-500/10 border-yellow-500/20' 
          : 'bg-green-500/10 border-green-500/20'
      }`}>
        <div className="flex items-center gap-3">
          {isPaused ? (
            <Lock className="w-6 h-6 text-yellow-500" />
          ) : (
            <Unlock className="w-6 h-6 text-green-500" />
          )}
          <div>
            <p className="font-semibold">
              Contract Status: {isPaused ? 'PAUSED' : 'ACTIVE'}
            </p>
            <p className="text-sm text-muted-foreground">
              {isPaused 
                ? 'No new swaps can be initiated. Existing swaps can still be completed.' 
                : 'All contract functions are operational.'}
            </p>
          </div>
        </div>
      </div>

      {/* Timelock Warning */}
      {pendingTimelock && (
        <div className="mb-6 p-4 bg-orange-500/10 border border-orange-500/20 rounded-lg">
          <div className="flex items-center gap-3 mb-3">
            <Clock className="w-5 h-5 text-orange-500" />
            <p className="font-semibold text-orange-500">Timelock Active</p>
          </div>
          <p className="text-sm text-muted-foreground mb-3">
            Operation will be executable in: <span className="font-mono font-bold">{formatTime(timelockRemaining)}</span>
          </p>
          <div className="flex gap-2">
            {timelockRemaining === 0 ? (
              <button
                onClick={pendingTimelock.type === 'pause' ? executePause : executeUnpause}
                disabled={loading}
                className="px-4 py-2 bg-primary text-primary-foreground rounded-lg hover:opacity-90 disabled:opacity-50"
              >
                {loading ? 'Executing...' : 'Execute Now'}
              </button>
            ) : (
              <button
                onClick={cancelTimelock}
                className="px-4 py-2 bg-secondary text-secondary-foreground rounded-lg hover:bg-border"
              >
                Cancel Operation
              </button>
            )}
          </div>
        </div>
      )}

      {/* Action Buttons */}
      {!pendingTimelock && (
        <div className="flex gap-4">
          {!isPaused ? (
            <button
              onClick={initiatePause}
              disabled={loading}
              className="flex items-center gap-2 px-6 py-3 bg-yellow-500 text-white rounded-lg hover:bg-yellow-600 disabled:opacity-50 font-semibold"
            >
              <Lock className="w-4 h-4" />
              {loading ? 'Initiating...' : 'Pause Contract'}
            </button>
          ) : (
            <button
              onClick={initiateUnpause}
              disabled={loading}
              className="flex items-center gap-2 px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 font-semibold"
            >
              <Unlock className="w-4 h-4" />
              {loading ? 'Initiating...' : 'Resume Contract'}
            </button>
          )}
        </div>
      )}

      {/* Info Box */}
      <div className="mt-6 p-4 bg-secondary rounded-lg">
        <div className="flex items-start gap-3">
          <AlertTriangle className="w-5 h-5 text-yellow-500 mt-0.5" />
          <div>
            <p className="font-semibold mb-1">Emergency Pause</p>
            <p className="text-sm text-muted-foreground">
              Pause functionality is protected by a {TIMELOCK_DELAY_SECONDS / 3600}-hour timelock for security.
              This prevents accidental or malicious pause operations. Once initiated, the timelock must
              expire before the operation can be executed.
            </p>
          </div>
        </div>
      </div>

      {/* Success/Error Messages */}
      {success && (
        <div className="mt-4 p-4 bg-green-500/10 border border-green-500/20 rounded-lg flex items-center gap-3">
          <CheckCircle className="w-5 h-5 text-green-500" />
          <p className="text-green-500">{success}</p>
        </div>
      )}

      {error && (
        <div className="mt-4 p-4 bg-red-500/10 border border-red-500/20 rounded-lg flex items-center gap-3">
          <AlertTriangle className="w-5 h-5 text-red-500" />
          <p className="text-red-500">{error}</p>
        </div>
      )}
    </div>
  );
}

export default PauseSwapControl;
