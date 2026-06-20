import React, { useState, useEffect } from 'react';
import { Shield, Settings, Activity, FileText, Lock, Unlock, AlertTriangle } from 'lucide-react';
import { useWallet } from '../context/WalletContext';
import { AdminRole, Permission } from '../lib/adminTypes';
import PauseSwapControl from './PauseSwapControl';
import FeeConfiguration from './FeeConfiguration';
import AuditLogViewer from './AuditLogViewer';
import SystemHealthMetrics from './SystemHealthMetrics';

const ADMIN_ADDRESSES: string[] = (
  (import.meta.env.VITE_ADMIN_ADDRESSES as string) || ''
).split(',').map((addr: string) => addr.trim()).filter(Boolean);

const ROLE_PERMISSIONS: Record<AdminRole, Permission[]> = {
  super_admin: [
    'pause_contracts',
    'update_fees',
    'update_config',
    'resolve_disputes',
    'view_logs',
    'view_metrics',
    'manage_tokens',
    'transfer_admin',
  ],
  operator: [
    'pause_contracts',
    'resolve_disputes',
    'view_logs',
    'view_metrics',
  ],
  viewer: [
    'view_logs',
    'view_metrics',
  ],
};

function AdminPanel() {
  const { wallet } = useWallet();
  const [activeTab, setActiveTab] = useState<'overview' | 'pause' | 'fees' | 'logs' | 'health'>('overview');
  const [userRole, setUserRole] = useState<AdminRole | null>(null);
  const [isAuthorized, setIsAuthorized] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const checkAuthorization = async () => {
      if (!wallet?.address) {
        setIsAuthorized(false);
        setUserRole(null);
        setIsLoading(false);
        return;
      }

      // Check if user is an admin
      const isAdmin = ADMIN_ADDRESSES.includes(wallet.address);
      
      if (isAdmin) {
        setIsAuthorized(true);
        // For demo, assign super_admin role. In production, fetch from backend
        setUserRole('super_admin');
      } else {
        setIsAuthorized(false);
        setUserRole(null);
      }
      setIsLoading(false);
    };

    checkAuthorization();
  }, [wallet?.address]);

  const hasPermission = (permission: Permission): boolean => {
    if (!userRole) return false;
    return ROLE_PERMISSIONS[userRole as AdminRole].includes(permission);
  };

  if (isLoading) {
    return (
      <div className="min-h-screen bg-background text-foreground p-8 flex items-center justify-center">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary"></div>
      </div>
    );
  }

  if (!wallet?.address) {
    return (
      <div className="min-h-screen bg-background text-foreground p-8">
        <div className="max-w-7xl mx-auto">
          <div className="bg-card border border-border rounded-lg p-8 text-center">
            <Lock className="w-16 h-16 mx-auto mb-4 text-muted-foreground" />
            <h2 className="text-2xl font-bold mb-2">Admin Access Required</h2>
            <p className="text-muted-foreground">Please connect your wallet to access the admin panel.</p>
          </div>
        </div>
      </div>
    );
  }

  if (!isAuthorized) {
    return (
      <div className="min-h-screen bg-background text-foreground p-8">
        <div className="max-w-7xl mx-auto">
          <div className="bg-card border border-border rounded-lg p-8 text-center">
            <AlertTriangle className="w-16 h-16 mx-auto mb-4 text-yellow-500" />
            <h2 className="text-2xl font-bold mb-2">Access Denied</h2>
            <p className="text-muted-foreground">Your wallet address is not authorized for admin access.</p>
            <p className="text-sm text-muted-foreground mt-2">Connected: {wallet.address}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background text-foreground p-8">
      <div className="max-w-7xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <div className="flex items-center gap-3 mb-2">
            <Shield className="w-8 h-8 text-primary" />
            <h1 className="text-3xl font-bold">Admin Control Panel</h1>
          </div>
          <p className="text-muted-foreground">
            Role: <span className="font-semibold text-primary">{userRole?.toUpperCase()}</span> | 
            Address: <span className="font-mono text-sm">{wallet.address.slice(0, 8)}...{wallet.address.slice(-8)}</span>
          </p>
        </div>

        {/* Tabs */}
        <div className="flex gap-2 mb-6 border-b border-border pb-4">
          <button
            onClick={() => setActiveTab('overview')}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
              activeTab === 'overview' 
                ? 'bg-primary text-primary-foreground' 
                : 'bg-secondary text-secondary-foreground hover:bg-border'
            }`}
          >
            <Settings className="w-4 h-4" />
            Overview
          </button>
          
          {hasPermission('pause_contracts') && (
            <button
              onClick={() => setActiveTab('pause')}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
                activeTab === 'pause' 
                  ? 'bg-primary text-primary-foreground' 
                  : 'bg-secondary text-secondary-foreground hover:bg-border'
              }`}
            >
              {activeTab === 'pause' ? <Unlock className="w-4 h-4" /> : <Lock className="w-4 h-4" />}
              Pause/Resume
            </button>
          )}
          
          {hasPermission('update_fees') && (
            <button
              onClick={() => setActiveTab('fees')}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
                activeTab === 'fees' 
                  ? 'bg-primary text-primary-foreground' 
                  : 'bg-secondary text-secondary-foreground hover:bg-border'
              }`}
            >
              <Settings className="w-4 h-4" />
              Fee Config
            </button>
          )}
          
          {hasPermission('view_logs') && (
            <button
              onClick={() => setActiveTab('logs')}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
                activeTab === 'logs' 
                  ? 'bg-primary text-primary-foreground' 
                  : 'bg-secondary text-secondary-foreground hover:bg-border'
              }`}
            >
              <FileText className="w-4 h-4" />
              Audit Logs
            </button>
          )}
          
          {hasPermission('view_metrics') && (
            <button
              onClick={() => setActiveTab('health')}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
                activeTab === 'health' 
                  ? 'bg-primary text-primary-foreground' 
                  : 'bg-secondary text-secondary-foreground hover:bg-border'
              }`}
            >
              <Activity className="w-4 h-4" />
              System Health
            </button>
          )}
        </div>

        {/* Content */}
        <div className="bg-card border border-border rounded-lg p-6">
          {activeTab === 'overview' && (
            <div>
              <h2 className="text-2xl font-bold mb-4">Overview</h2>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <div className="bg-secondary rounded-lg p-4">
                  <p className="text-sm text-muted-foreground mb-1">Your Role</p>
                  <p className="text-2xl font-bold">{userRole?.toUpperCase()}</p>
                </div>
                <div className="bg-secondary rounded-lg p-4">
                  <p className="text-sm text-muted-foreground mb-1">Permissions</p>
                  <p className="text-2xl font-bold">{ROLE_PERMISSIONS[userRole || 'viewer'].length}</p>
                </div>
                <div className="bg-secondary rounded-lg p-4">
                  <p className="text-sm text-muted-foreground mb-1">Status</p>
                  <p className="text-2xl font-bold text-green-500">Active</p>
                </div>
                <div className="bg-secondary rounded-lg p-4">
                  <p className="text-sm text-muted-foreground mb-1">Network</p>
                  <p className="text-2xl font-bold">{import.meta.env.VITE_STELLAR_NETWORK === 'mainnet' ? 'Mainnet' : 'Testnet'}</p>
                </div>
              </div>
              
              <div className="mt-6">
                <h3 className="text-lg font-semibold mb-3">Your Permissions</h3>
                <div className="flex flex-wrap gap-2">
                  {ROLE_PERMISSIONS[userRole || 'viewer'].map((perm: Permission) => (
                    <span key={perm} className="px-3 py-1 bg-primary/10 text-primary rounded-full text-sm">
                      {perm.replace(/_/g, ' ')}
                    </span>
                  ))}
                </div>
              </div>
            </div>
          )}

          {activeTab === 'pause' && hasPermission('pause_contracts') && (
            <PauseSwapControl />
          )}

          {activeTab === 'fees' && hasPermission('update_fees') && (
            <FeeConfiguration />
          )}

          {activeTab === 'logs' && hasPermission('view_logs') && (
            <AuditLogViewer />
          )}

          {activeTab === 'health' && hasPermission('view_metrics') && (
            <SystemHealthMetrics />
          )}
        </div>
      </div>
    </div>
  );
}

export default AdminPanel;
