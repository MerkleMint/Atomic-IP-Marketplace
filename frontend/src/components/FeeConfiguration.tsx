import React, { useState, useEffect } from 'react';
import { Settings, Clock, Users, CheckCircle, AlertTriangle } from 'lucide-react';
import { useWallet } from '../context/WalletContext';
import { updateAtomicSwapConfig, getAtomicSwapConfig } from '../lib/contractClient';
import { FeeUpdateProposal } from '../lib/adminTypes';

const GOVERNANCE_ADDRESSES: string[] = [
  // Add governance addresses here
  // 'GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX'
];

const REQUIRED_APPROVALS = 2;

function FeeConfiguration() {
  const { wallet } = useWallet();
  const [currentConfig, setCurrentConfig] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  
  // Form state
  const [feeBps, setFeeBps] = useState(250);
  const [feeRecipient, setFeeRecipient] = useState('');
  const [cancelDelaySecs, setCancelDelaySecs] = useState(3600);
  const [reason, setReason] = useState('');
  
  // Governance state
  const [pendingProposal, setPendingProposal] = useState<FeeUpdateProposal | null>(null);
  const [proposals, setProposals] = useState<FeeUpdateProposal[]>([]);

  useEffect(() => {
    loadCurrentConfig();
    loadProposals();
  }, []);

  const loadCurrentConfig = async () => {
    try {
      const config = await getAtomicSwapConfig();
      setCurrentConfig(config);
      if (config) {
        setFeeBps(Number(config.fee_bps) || 250);
        setFeeRecipient(config.fee_recipient || '');
        setCancelDelaySecs(Number(config.cancel_delay_secs) || 3600);
      }
    } catch (err) {
      console.error('Failed to load config:', err);
    }
  };

  const loadProposals = () => {
    const stored = localStorage.getItem('feeProposals');
    if (stored) {
      setProposals(JSON.parse(stored));
    }
  };

  const saveProposals = (newProposals: FeeUpdateProposal[]) => {
    localStorage.setItem('feeProposals', JSON.stringify(newProposals));
    setProposals(newProposals);
  };

  const createProposal = async () => {
    if (!wallet) return;
    
    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      const proposal: FeeUpdateProposal = {
        id: `fee-${Date.now()}`,
        proposedBy: wallet.address,
        proposedAt: Math.floor(Date.now() / 1000),
        newFeeBps: feeBps,
        newFeeRecipient: feeRecipient,
        reason,
        status: 'pending',
        approvals: [wallet.address],
        requiredApprovals: REQUIRED_APPROVALS,
      };

      const newProposals = [proposal, ...proposals];
      saveProposals(newProposals);
      setPendingProposal(proposal);
      setSuccess('Fee update proposal created. Awaiting governance approval.');
      
      // Log to audit
      await logAuditAction('propose_fee_update', 'atomic_swap', { proposalId: proposal.id });
    } catch (err: any) {
      setError(err.message || 'Failed to create proposal');
    } finally {
      setLoading(false);
    }
  };

  const approveProposal = async (proposalId: string) => {
    if (!wallet) return;
    
    setLoading(true);
    setError(null);

    try {
      const proposalIndex = proposals.findIndex(p => p.id === proposalId);
      if (proposalIndex === -1) return;

      const proposal = { ...proposals[proposalIndex] };
      
      if (proposal.approvals.includes(wallet.address)) {
        setError('You have already approved this proposal');
        setLoading(false);
        return;
      }

      proposal.approvals = [...proposal.approvals, wallet.address];
      
      const newProposals = [...proposals];
      newProposals[proposalIndex] = proposal;
      saveProposals(newProposals);

      if (proposal.approvals.length >= proposal.requiredApprovals) {
        // Execute the proposal
        await executeProposal(proposal);
      } else {
        setSuccess('Proposal approved. Awaiting more approvals.');
      }
      
      // Log to audit
      await logAuditAction('approve_fee_update', 'atomic_swap', { proposalId });
    } catch (err: any) {
      setError(err.message || 'Failed to approve proposal');
    } finally {
      setLoading(false);
    }
  };

  const executeProposal = async (proposal: FeeUpdateProposal) => {
    if (!wallet) return;

    try {
      await updateAtomicSwapConfig(
        proposal.newFeeBps,
        proposal.newFeeRecipient,
        cancelDelaySecs,
        wallet
      );

      // Update proposal status
      const proposalIndex = proposals.findIndex(p => p.id === proposal.id);
      if (proposalIndex !== -1) {
        const newProposals = [...proposals];
        newProposals[proposalIndex] = { ...proposal, status: 'executed' };
        saveProposals(newProposals);
      }

      setSuccess('Fee configuration updated successfully');
      await loadCurrentConfig();
      setPendingProposal(null);
      
      // Log to audit
      await logAuditAction('execute_fee_update', 'atomic_swap', { proposalId: proposal.id });
    } catch (err: any) {
      setError(err.message || 'Failed to execute proposal');
    }
  };

  const rejectProposal = async (proposalId: string) => {
    const proposalIndex = proposals.findIndex(p => p.id === proposalId);
    if (proposalIndex === -1) return;

    const newProposals = [...proposals];
    newProposals[proposalIndex] = { ...newProposals[proposalIndex], status: 'rejected' };
    saveProposals(newProposals);
    
    setSuccess('Proposal rejected');
    
    // Log to audit
    await logAuditAction('reject_fee_update', 'atomic_swap', { proposalId });
  };

  const logAuditAction = async (action: string, targetContract: string, details: any) => {
    const auditLog = {
      id: `${Date.now()}-${Math.random()}`,
      timestamp: Math.floor(Date.now() / 1000),
      admin: wallet?.address,
      action,
      targetContract: targetContract as 'atomic_swap' | 'ip_registry',
      details,
    };
    
    const logs = JSON.parse(localStorage.getItem('adminAuditLogs') || '[]');
    logs.unshift(auditLog);
    localStorage.setItem('adminAuditLogs', JSON.stringify(logs.slice(0, 100)));
  };

  const isGovernanceMember = wallet?.address && GOVERNANCE_ADDRESSES.includes(wallet.address);

  return (
    <div>
      <h2 className="text-2xl font-bold mb-4">Fee Configuration</h2>

      {/* Current Configuration */}
      {currentConfig && (
        <div className="mb-6 p-4 bg-secondary rounded-lg">
          <h3 className="font-semibold mb-3">Current Configuration</h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <p className="text-sm text-muted-foreground">Fee Basis Points</p>
              <p className="text-xl font-bold">{currentConfig.fee_bps} bps ({(currentConfig.fee_bps / 100).toFixed(2)}%)</p>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Fee Recipient</p>
              <p className="text-sm font-mono">{currentConfig.fee_recipient?.slice(0, 12)}...</p>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Cancel Delay</p>
              <p className="text-xl font-bold">{currentConfig.cancel_delay_secs}s</p>
            </div>
          </div>
        </div>
      )}

      {/* Update Form */}
      <div className="mb-6 p-4 bg-card border border-border rounded-lg">
        <h3 className="font-semibold mb-4">Propose Fee Update</h3>
        
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
          <div>
            <label className="block text-sm font-medium mb-2">Fee Basis Points (0-10000)</label>
            <input
              type="number"
              min="0"
              max="10000"
              value={feeBps}
              onChange={(e) => setFeeBps(Number(e.target.value))}
              className="w-full px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
              placeholder="250"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {(feeBps / 100).toFixed(2)}% fee on all swaps
            </p>
          </div>
          
          <div>
            <label className="block text-sm font-medium mb-2">Fee Recipient Address</label>
            <input
              type="text"
              value={feeRecipient}
              onChange={(e) => setFeeRecipient(e.target.value)}
              className="w-full px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50 font-mono"
              placeholder="G..."
            />
          </div>
          
          <div>
            <label className="block text-sm font-medium mb-2">Cancel Delay (seconds)</label>
            <input
              type="number"
              min="0"
              value={cancelDelaySecs}
              onChange={(e) => setCancelDelaySecs(Number(e.target.value))}
              className="w-full px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
              placeholder="3600"
            />
            <p className="text-xs text-muted-foreground mt-1">
              Time before buyer can cancel swap
            </p>
          </div>
          
          <div>
            <label className="block text-sm font-medium mb-2">Reason for Change</label>
            <input
              type="text"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              className="w-full px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
              placeholder="Explain the reason for this update"
            />
          </div>
        </div>

        <button
          onClick={createProposal}
          disabled={loading || !feeRecipient || feeBps < 0 || feeBps > 10000}
          className="px-6 py-2 bg-primary text-primary-foreground rounded-lg hover:opacity-90 disabled:opacity-50 font-semibold"
        >
          {loading ? 'Creating...' : 'Create Proposal'}
        </button>
      </div>

      {/* Pending Proposals */}
      {proposals.length > 0 && (
        <div className="mb-6">
          <h3 className="font-semibold mb-4 flex items-center gap-2">
            <Users className="w-4 h-4" />
            Governance Proposals
          </h3>
          
          <div className="space-y-3">
            {proposals.map((proposal) => (
              <div key={proposal.id} className="p-4 bg-card border border-border rounded-lg">
                <div className="flex items-start justify-between mb-3">
                  <div>
                    <p className="font-semibold">
                      Fee: {proposal.newFeeBps} bps ({(proposal.newFeeBps / 100).toFixed(2)}%)
                    </p>
                    <p className="text-sm text-muted-foreground">
                      Proposed by {proposal.proposedBy.slice(0, 8)}...{proposal.proposedBy.slice(-8)}
                    </p>
                  </div>
                  <span className={`px-2 py-1 rounded text-xs font-semibold ${
                    proposal.status === 'executed' ? 'bg-green-500/10 text-green-500' :
                    proposal.status === 'rejected' ? 'bg-red-500/10 text-red-500' :
                    'bg-yellow-500/10 text-yellow-500'
                  }`}>
                    {proposal.status.toUpperCase()}
                  </span>
                </div>
                
                {proposal.reason && (
                  <p className="text-sm text-muted-foreground mb-3">Reason: {proposal.reason}</p>
                )}
                
                <div className="flex items-center gap-2 mb-3">
                  <Users className="w-4 h-4 text-muted-foreground" />
                  <span className="text-sm">
                    Approvals: {proposal.approvals.length} / {proposal.requiredApprovals}
                  </span>
                </div>
                
                {proposal.status === 'pending' && isGovernanceMember && (
                  <div className="flex gap-2">
                    {!proposal.approvals.includes(wallet?.address || '') && (
                      <button
                        onClick={() => approveProposal(proposal.id)}
                        disabled={loading}
                        className="px-3 py-1 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50 text-sm"
                      >
                        Approve
                      </button>
                    )}
                    <button
                      onClick={() => rejectProposal(proposal.id)}
                      className="px-3 py-1 bg-red-500 text-white rounded hover:bg-red-600 text-sm"
                    >
                      Reject
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Governance Info */}
      <div className="p-4 bg-secondary rounded-lg">
        <div className="flex items-start gap-3">
          <Settings className="w-5 h-5 text-primary mt-0.5" />
          <div>
            <p className="font-semibold mb-1">Governance Process</p>
            <p className="text-sm text-muted-foreground">
              Fee changes require {REQUIRED_APPROVALS} approvals from governance members.
              Proposals are stored and can be approved by authorized addresses. Once the required
              number of approvals is reached, the change is executed automatically.
            </p>
            <p className="text-xs text-muted-foreground mt-2">
              Current governance members: {GOVERNANCE_ADDRESSES.length}
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

export default FeeConfiguration;
