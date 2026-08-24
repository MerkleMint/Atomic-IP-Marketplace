import { useState, useEffect } from 'react';
import { Settings, Users, CheckCircle, AlertTriangle } from 'lucide-react';
import { useWallet } from '../context/WalletContext';
import {
  getAtomicSwapConfig,
  getGovernanceConfig,
  getFeeProposal,
  proposeFeeUpdate,
  approveFeeUpdate,
  executeFeeUpdate,
} from '../lib/contractClient';
import { FEE_GOVERNANCE_TIMELOCK_SECS } from '../lib/feeGovernanceTypes';
import type { GovernanceConfig, FeeProposal } from '../lib/feeGovernanceTypes';

function shortAddr(addr: string): string {
  if (!addr || addr.length < 16) return addr;
  return `${addr.slice(0, 8)}...${addr.slice(-8)}`;
}

function FeeConfiguration() {
  const { wallet } = useWallet();
  const [currentConfig, setCurrentConfig] = useState<any>(null);
  const [governanceConfig, setGovernanceConfig] = useState<GovernanceConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Propose form state
  const [feeBps, setFeeBps] = useState(250);
  const [feeRecipient, setFeeRecipient] = useState('');
  const [cancelDelaySecs, setCancelDelaySecs] = useState(3600);

  // Proposals known to this session. There is no on-chain "list all
  // proposals" call, so proposals are added here either by creating one or
  // by looking one up by id below — every entry always reflects a fresh
  // on-chain read, never client-side approval state.
  const [proposals, setProposals] = useState<FeeProposal[]>([]);
  const [lookupId, setLookupId] = useState('');

  useEffect(() => {
    loadCurrentConfig();
    loadGovernanceConfig();
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

  const loadGovernanceConfig = async () => {
    try {
      const cfg = await getGovernanceConfig();
      setGovernanceConfig(cfg);
    } catch (err) {
      console.error('Failed to load governance config:', err);
    }
  };

  const upsertProposal = (proposal: FeeProposal) => {
    setProposals((prev) => {
      const idx = prev.findIndex((p) => p.proposal_id === proposal.proposal_id);
      if (idx === -1) return [proposal, ...prev];
      const next = [...prev];
      next[idx] = proposal;
      return next;
    });
  };

  const refreshProposal = async (proposalId: number) => {
    const proposal = await getFeeProposal(proposalId);
    if (proposal) upsertProposal(proposal);
    return proposal;
  };

  const createProposal = async () => {
    if (!wallet) return;

    setLoading(true);
    setError(null);
    setSuccess(null);

    try {
      const proposalId = await proposeFeeUpdate(feeBps, feeRecipient, cancelDelaySecs, wallet);
      await refreshProposal(proposalId);
      setSuccess(`Fee update proposal #${proposalId} created on-chain. Awaiting governance approval.`);
    } catch (err: any) {
      setError(err.message || 'Failed to create proposal');
    } finally {
      setLoading(false);
    }
  };

  const approveProposal = async (proposalId: number) => {
    if (!wallet) return;

    setLoading(true);
    setError(null);

    try {
      await approveFeeUpdate(proposalId, wallet);
      const proposal = await refreshProposal(proposalId);
      if (proposal?.quorum_reached_at) {
        setSuccess(`Proposal #${proposalId} reached quorum. It can be executed after the timelock elapses.`);
      } else {
        setSuccess(`Proposal #${proposalId} approved. Awaiting more approvals.`);
      }
    } catch (err: any) {
      setError(err.message || 'Failed to approve proposal');
    } finally {
      setLoading(false);
    }
  };

  const executeProposal = async (proposalId: number) => {
    if (!wallet) return;

    setLoading(true);
    setError(null);

    try {
      await executeFeeUpdate(proposalId, wallet);
      await refreshProposal(proposalId);
      setSuccess(`Fee configuration updated on-chain from proposal #${proposalId}.`);
      await loadCurrentConfig();
    } catch (err: any) {
      setError(err.message || 'Failed to execute proposal');
    } finally {
      setLoading(false);
    }
  };

  const lookupProposal = async () => {
    const id = Number(lookupId);
    if (!Number.isFinite(id) || id < 0) {
      setError('Enter a valid proposal ID');
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const proposal = await refreshProposal(id);
      if (!proposal) {
        setError(`No proposal found with ID ${id}`);
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load proposal');
    } finally {
      setLoading(false);
    }
  };

  const isGovernanceMember = Boolean(
    wallet?.address && governanceConfig?.signers.includes(wallet.address)
  );

  const executableAt = (proposal: FeeProposal): number | null =>
    proposal.quorum_reached_at !== null
      ? proposal.quorum_reached_at + FEE_GOVERNANCE_TIMELOCK_SECS
      : null;

  const isExecutable = (proposal: FeeProposal): boolean => {
    const at = executableAt(proposal);
    return at !== null && Math.floor(Date.now() / 1000) >= at;
  };

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

      {/* Propose Form */}
      <div className="mb-6 p-4 bg-card border border-border rounded-lg">
        <h3 className="font-semibold mb-4">Propose Fee Update</h3>

        {!isGovernanceMember && (
          <p className="text-sm text-muted-foreground mb-4">
            Only configured governance signers can propose or approve fee updates.
            {wallet ? '' : ' Connect a governance wallet to continue.'}
          </p>
        )}

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
        </div>

        <button
          onClick={createProposal}
          disabled={loading || !isGovernanceMember || !feeRecipient || feeBps < 0 || feeBps > 10000}
          className="px-6 py-2 bg-primary text-primary-foreground rounded-lg hover:opacity-90 disabled:opacity-50 font-semibold"
        >
          {loading ? 'Submitting...' : 'Create Proposal On-Chain'}
        </button>
      </div>

      {/* Look up a proposal by ID — the way a signer on a separate device
          finds a proposal created elsewhere, since state lives on-chain
          rather than in any shared client store. */}
      <div className="mb-6 p-4 bg-card border border-border rounded-lg">
        <h3 className="font-semibold mb-3">Look Up a Proposal</h3>
        <div className="flex gap-2">
          <input
            type="number"
            min="0"
            value={lookupId}
            onChange={(e) => setLookupId(e.target.value)}
            className="flex-1 px-3 py-2 bg-secondary border border-border rounded-lg focus:ring-2 focus:ring-primary/50"
            placeholder="Proposal ID"
          />
          <button
            onClick={lookupProposal}
            disabled={loading || !lookupId}
            className="px-4 py-2 bg-secondary border border-border rounded-lg hover:opacity-90 disabled:opacity-50 font-semibold"
          >
            Load
          </button>
        </div>
      </div>

      {/* Known Proposals */}
      {proposals.length > 0 && (
        <div className="mb-6">
          <h3 className="font-semibold mb-4 flex items-center gap-2">
            <Users className="w-4 h-4" />
            Governance Proposals
          </h3>

          <div className="space-y-3">
            {proposals.map((proposal) => {
              const requiredApprovals = governanceConfig?.required_approvals ?? proposal.approved_by.length;
              const hasApproved = wallet?.address ? proposal.approved_by.includes(wallet.address) : false;
              const execAt = executableAt(proposal);
              return (
                <div key={proposal.proposal_id} className="p-4 bg-card border border-border rounded-lg">
                  <div className="flex items-start justify-between mb-3">
                    <div>
                      <p className="font-semibold">
                        Proposal #{proposal.proposal_id} — Fee: {proposal.fee_bps} bps ({(proposal.fee_bps / 100).toFixed(2)}%)
                      </p>
                      <p className="text-sm text-muted-foreground">
                        Proposed by {shortAddr(proposal.proposer)}
                      </p>
                      <p className="text-sm text-muted-foreground">
                        Recipient: {shortAddr(proposal.fee_recipient)} · Cancel delay: {proposal.cancel_delay_secs}s
                      </p>
                    </div>
                    <span className={`px-2 py-1 rounded text-xs font-semibold ${
                      proposal.executed ? 'bg-green-500/10 text-green-500' :
                      proposal.quorum_reached_at ? 'bg-blue-500/10 text-blue-500' :
                      'bg-yellow-500/10 text-yellow-500'
                    }`}>
                      {proposal.executed ? 'EXECUTED' : proposal.quorum_reached_at ? 'QUORUM REACHED' : 'PENDING'}
                    </span>
                  </div>

                  <div className="flex items-center gap-2 mb-3">
                    <Users className="w-4 h-4 text-muted-foreground" />
                    <span className="text-sm">
                      On-chain approvals: {proposal.approved_by.length} / {requiredApprovals}
                    </span>
                  </div>

                  {proposal.quorum_reached_at && !proposal.executed && (
                    <p className="text-xs text-muted-foreground mb-3">
                      {isExecutable(proposal)
                        ? 'Timelock elapsed — ready to execute.'
                        : `Executable at ${execAt ? new Date(execAt * 1000).toLocaleString() : '—'} (on-chain timelock).`}
                    </p>
                  )}

                  {!proposal.executed && (
                    <div className="flex gap-2">
                      {isGovernanceMember && !hasApproved && (
                        <button
                          onClick={() => approveProposal(proposal.proposal_id)}
                          disabled={loading}
                          className="px-3 py-1 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50 text-sm"
                        >
                          Approve
                        </button>
                      )}
                      {proposal.quorum_reached_at && (
                        <button
                          onClick={() => executeProposal(proposal.proposal_id)}
                          disabled={loading || !isExecutable(proposal)}
                          className="px-3 py-1 bg-primary text-primary-foreground rounded hover:opacity-90 disabled:opacity-50 text-sm"
                        >
                          Execute
                        </button>
                      )}
                      <button
                        onClick={() => refreshProposal(proposal.proposal_id)}
                        disabled={loading}
                        className="px-3 py-1 bg-secondary border border-border rounded hover:opacity-90 disabled:opacity-50 text-sm"
                      >
                        Refresh
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
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
              Fee changes are proposed and approved on-chain via the atomic_swap contract's
              fee-governance functions. Each approval is authorised by that signer's own wallet —
              no shared or local proposal state is used, so signers on different
              devices independently reach the same on-chain quorum. Once quorum is reached, a
              minimum on-chain timelock must elapse before the change can be executed.
            </p>
            <p className="text-xs text-muted-foreground mt-2">
              {governanceConfig
                ? `Governance scheme: ${governanceConfig.required_approvals}-of-${governanceConfig.signers.length} signers`
                : 'Fee governance has not been configured on-chain yet. Contact the contract admin.'}
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
