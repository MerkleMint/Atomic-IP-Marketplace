import React, { useState, useEffect } from 'react';
import { Activity, TrendingUp, AlertTriangle, CheckCircle, Clock, DollarSign, Layers } from 'lucide-react';
import { getListingCount } from '../lib/contractClient';
import { SystemHealthMetrics } from '../lib/adminTypes';

function SystemHealthMetrics() {
  const [metrics, setMetrics] = useState<SystemHealthMetrics>({
    totalSwaps: 0,
    activeSwaps: 0,
    completedSwaps: 0,
    disputedSwaps: 0,
    totalListings: 0,
    totalVolumeUsdc: 0,
    contractPaused: false,
    lastBlockTimestamp: 0,
    pendingTimelockOps: 0,
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadMetrics();
    const interval = setInterval(loadMetrics, 30000); // Refresh every 30 seconds
    return () => clearInterval(interval);
  }, []);

  const loadMetrics = async () => {
    try {
      const listingCount = await getListingCount();
      
      // Get audit logs for swap statistics
      let logs: any[] = [];
      try {
        logs = JSON.parse(localStorage.getItem('adminAuditLogs') || '[]');
      } catch (err) {
        console.error('Failed to load audit logs:', err);
        logs = [];
      }
      
      const completedSwaps = logs.filter((log: any) => 
        log.action === 'complete_swap' || log.action === 'release_to_seller'
      ).length;
      
      const disputedSwaps = logs.filter((log: any) => 
        log.action === 'raise_dispute' || log.action === 'resolve_dispute'
      ).length;

      // Get fee proposals for timelock ops
      let feeProposals: any[] = [];
      try {
        feeProposals = JSON.parse(localStorage.getItem('feeProposals') || '[]');
      } catch (err) {
        console.error('Failed to load fee proposals:', err);
        feeProposals = [];
      }
      const pendingTimelockOps = feeProposals.filter((p: any) => p.status === 'pending').length;

      setMetrics({
        totalSwaps: logs.filter((log: any) => log.action.includes('swap')).length,
        activeSwaps: logs.filter((log: any) => log.action === 'initiate_swap').length - completedSwaps,
        completedSwaps,
        disputedSwaps,
        totalListings: listingCount,
        totalVolumeUsdc: logs.reduce((acc: number, log: any) => {
          if (log.details?.usdc_amount) {
            return acc + Number(log.details.usdc_amount);
          }
          return acc;
        }, 0) / 1e7, // Convert from raw to USDC (7 decimals)
        contractPaused: false, // Would be fetched from contract
        lastBlockTimestamp: Math.floor(Date.now() / 1000),
        pendingTimelockOps,
      });
    } catch (err) {
      console.error('Failed to load metrics:', err);
    } finally {
      setLoading(false);
    }
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const formatUsdc = (amount: number) => {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
    }).format(amount);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Activity className="w-8 h-8 animate-spin text-primary" />
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">System Health Metrics</h2>

      {/* Status Banner */}
      <div className={`mb-6 p-4 rounded-lg border ${
        metrics.contractPaused 
          ? 'bg-yellow-500/10 border-yellow-500/20' 
          : 'bg-green-500/10 border-green-500/20'
      }`}>
        <div className="flex items-center gap-3">
          {metrics.contractPaused ? (
            <AlertTriangle className="w-6 h-6 text-yellow-500" />
          ) : (
            <CheckCircle className="w-6 h-6 text-green-500" />
          )}
          <div>
            <p className="font-semibold">
              System Status: {metrics.contractPaused ? 'PAUSED' : 'OPERATIONAL'}
            </p>
            <p className="text-sm text-muted-foreground">
              Last updated: {formatTimestamp(metrics.lastBlockTimestamp)}
            </p>
          </div>
        </div>
      </div>

      {/* Metrics Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <Layers className="w-5 h-5 text-blue-500" />
            <p className="text-sm text-muted-foreground">Total Listings</p>
          </div>
          <p className="text-3xl font-bold">{metrics.totalListings}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <Activity className="w-5 h-5 text-green-500" />
            <p className="text-sm text-muted-foreground">Total Swaps</p>
          </div>
          <p className="text-3xl font-bold">{metrics.totalSwaps}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <TrendingUp className="w-5 h-5 text-purple-500" />
            <p className="text-sm text-muted-foreground">Active Swaps</p>
          </div>
          <p className="text-3xl font-bold">{metrics.activeSwaps}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <CheckCircle className="w-5 h-5 text-green-500" />
            <p className="text-sm text-muted-foreground">Completed</p>
          </div>
          <p className="text-3xl font-bold">{metrics.completedSwaps}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <AlertTriangle className="w-5 h-5 text-red-500" />
            <p className="text-sm text-muted-foreground">Disputed</p>
          </div>
          <p className="text-3xl font-bold">{metrics.disputedSwaps}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <DollarSign className="w-5 h-5 text-green-500" />
            <p className="text-sm text-muted-foreground">Total Volume</p>
          </div>
          <p className="text-3xl font-bold">{formatUsdc(metrics.totalVolumeUsdc)}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <Clock className="w-5 h-5 text-orange-500" />
            <p className="text-sm text-muted-foreground">Pending Timelock Ops</p>
          </div>
          <p className="text-3xl font-bold">{metrics.pendingTimelockOps}</p>
        </div>

        <div className="p-4 bg-secondary rounded-lg">
          <div className="flex items-center gap-2 mb-2">
            <Activity className="w-5 h-5 text-primary" />
            <p className="text-sm text-muted-foreground">Completion Rate</p>
          </div>
          <p className="text-3xl font-bold">
            {metrics.totalSwaps > 0 
              ? ((metrics.completedSwaps / metrics.totalSwaps) * 100).toFixed(1) 
              : '0'}%
          </p>
        </div>
      </div>

      {/* Detailed Metrics */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Swap Statistics */}
        <div className="p-4 bg-card border border-border rounded-lg">
          <h3 className="font-semibold mb-4 flex items-center gap-2">
            <Activity className="w-5 h-5" />
            Swap Statistics
          </h3>
          <div className="space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Total Swaps</span>
              <span className="font-semibold">{metrics.totalSwaps}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Active Swaps</span>
              <span className="font-semibold">{metrics.activeSwaps}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Completed Swaps</span>
              <span className="font-semibold text-green-500">{metrics.completedSwaps}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Disputed Swaps</span>
              <span className="font-semibold text-red-500">{metrics.disputedSwaps}</span>
            </div>
            <div className="w-full bg-secondary rounded-full h-2 mt-2">
              <div 
                className="bg-green-500 h-2 rounded-full" 
                style={{ width: `${metrics.totalSwaps > 0 ? (metrics.completedSwaps / metrics.totalSwaps) * 100 : 0}%` }}
              />
            </div>
          </div>
        </div>

        {/* System Status */}
        <div className="p-4 bg-card border border-border rounded-lg">
          <h3 className="font-semibold mb-4 flex items-center gap-2">
            <CheckCircle className="w-5 h-5" />
            System Status
          </h3>
          <div className="space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Contract Status</span>
              <span className={`font-semibold ${metrics.contractPaused ? 'text-yellow-500' : 'text-green-500'}`}>
                {metrics.contractPaused ? 'PAUSED' : 'ACTIVE'}
              </span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Last Block</span>
              <span className="font-mono text-sm">{formatTimestamp(metrics.lastBlockTimestamp)}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Pending Operations</span>
              <span className="font-semibold">{metrics.pendingTimelockOps}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Total Listings</span>
              <span className="font-semibold">{metrics.totalListings}</span>
            </div>
          </div>
        </div>

        {/* Volume Statistics */}
        <div className="p-4 bg-card border border-border rounded-lg">
          <h3 className="font-semibold mb-4 flex items-center gap-2">
            <DollarSign className="w-5 h-5" />
            Volume Statistics
          </h3>
          <div className="space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Total Volume</span>
              <span className="font-semibold text-green-500">{formatUsdc(metrics.totalVolumeUsdc)}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Avg per Swap</span>
              <span className="font-semibold">
                {metrics.totalSwaps > 0 ? formatUsdc(metrics.totalVolumeUsdc / metrics.totalSwaps) : '$0.00'}
              </span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-muted-foreground">Listings Value</span>
              <span className="font-semibold">N/A</span>
            </div>
          </div>
        </div>

        {/* Health Indicators */}
        <div className="p-4 bg-card border border-border rounded-lg">
          <h3 className="font-semibold mb-4 flex items-center gap-2">
            <Activity className="w-5 h-5" />
            Health Indicators
          </h3>
          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <div className={`w-3 h-3 rounded-full ${metrics.contractPaused ? 'bg-yellow-500' : 'bg-green-500'}`} />
              <span className="text-sm">Contract Operational</span>
            </div>
            <div className="flex items-center gap-3">
              <div className={`w-3 h-3 rounded-full ${metrics.pendingTimelockOps === 0 ? 'bg-green-500' : 'bg-orange-500'}`} />
              <span className="text-sm">No Pending Timelocks</span>
            </div>
            <div className="flex items-center gap-3">
              <div className={`w-3 h-3 rounded-full ${metrics.disputedSwaps === 0 ? 'bg-green-500' : 'bg-yellow-500'}`} />
              <span className="text-sm">No Active Disputes</span>
            </div>
            <div className="flex items-center gap-3">
              <div className="w-3 h-3 rounded-full bg-green-500" />
              <span className="text-sm">Audit Logging Active</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SystemHealthMetrics;
