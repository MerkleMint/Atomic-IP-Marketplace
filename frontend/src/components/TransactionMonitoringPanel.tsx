import React, { useEffect, useState } from 'react';
import './TransactionMonitoringPanel.css';
import { complianceCheckService, ComplianceAuditEntry } from '../services/complianceCheckService';

interface TransactionMonitoringPanelProps {
  userId?: string;
  autoRefresh?: boolean;
  refreshInterval?: number;
}

interface ComplianceStats {
  totalChecks: number;
  compliantTransactions: number;
  blockedTransactions: number;
  reviewRequired: number;
  blockRate: number;
}

export function TransactionMonitoringPanel({
  userId,
  autoRefresh = true,
  refreshInterval = 30000,
}: TransactionMonitoringPanelProps) {
  const [auditEntries, setAuditEntries] = useState<ComplianceAuditEntry[]>([]);
  const [stats, setStats] = useState<ComplianceStats | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<ComplianceAuditEntry | null>(null);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<'all' | 'compliant' | 'blocked' | 'review'>('all');

  useEffect(() => {
    loadMonitoringData();

    if (autoRefresh) {
      const interval = setInterval(loadMonitoringData, refreshInterval);
      return () => clearInterval(interval);
    }
  }, [userId, autoRefresh, refreshInterval]);

  const loadMonitoringData = async () => {
    try {
      setLoading(true);
      const entries = complianceCheckService.getAuditEntries(userId);
      setAuditEntries(entries);

      const complianceStats = complianceCheckService.getComplianceStats();
      setStats(complianceStats);
    } catch (error) {
      console.error('Failed to load monitoring data:', error);
    } finally {
      setLoading(false);
    }
  };

  const getFilteredEntries = (): ComplianceAuditEntry[] => {
    return auditEntries.filter(entry => {
      if (filter === 'all') return true;
      if (filter === 'compliant') return entry.overallStatus === 'compliant';
      if (filter === 'blocked') return entry.overallStatus === 'blocked';
      if (filter === 'review') return entry.overallStatus === 'review_required';
      return true;
    });
  };

  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'compliant':
        return 'status-compliant';
      case 'blocked':
        return 'status-blocked';
      case 'review_required':
        return 'status-review';
      default:
        return '';
    }
  };

  const getStatusIcon = (status: string): string => {
    switch (status) {
      case 'compliant':
        return '✓';
      case 'blocked':
        return '✕';
      case 'review_required':
        return '⚠';
      default:
        return '−';
    }
  };

  const formatDate = (timestamp: number): string => {
    const date = new Date(timestamp);
    return date.toLocaleString();
  };

  const formatCurrency = (amount: number): string => {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
    }).format(amount);
  };

  const filteredEntries = getFilteredEntries();

  if (loading) {
    return (
      <div className="transaction-monitoring-panel loading">
        <div className="spinner"></div>
        <p>Loading transaction monitoring...</p>
      </div>
    );
  }

  return (
    <div className="transaction-monitoring-panel">
      <div className="monitoring-header">
        <h3>Transaction Monitoring</h3>
        <button
          className="refresh-btn"
          onClick={loadMonitoringData}
          title="Refresh monitoring data"
        >
          ↻
        </button>
      </div>

      {/* Statistics Section */}
      {stats && (
        <div className="compliance-stats">
          <div className="stat-card compliant">
            <div className="stat-number">{stats.compliantTransactions}</div>
            <div className="stat-label">Compliant</div>
          </div>

          <div className="stat-card review">
            <div className="stat-number">{stats.reviewRequired}</div>
            <div className="stat-label">Review Required</div>
          </div>

          <div className="stat-card blocked">
            <div className="stat-number">{stats.blockedTransactions}</div>
            <div className="stat-label">Blocked</div>
          </div>

          <div className="stat-card rate">
            <div className="stat-number">{stats.blockRate.toFixed(1)}%</div>
            <div className="stat-label">Block Rate</div>
          </div>
        </div>
      )}

      {/* Filter Section */}
      <div className="filter-section">
        <label>Filter by Status:</label>
        <div className="filter-buttons">
          {(['all', 'compliant', 'blocked', 'review'] as const).map(status => (
            <button
              key={status}
              className={`filter-btn ${filter === status ? 'active' : ''}`}
              onClick={() => setFilter(status)}
            >
              {status.charAt(0).toUpperCase() + status.slice(1).replace('_', ' ')}
            </button>
          ))}
        </div>
      </div>

      {/* Transactions List */}
      <div className="transactions-list">
        {filteredEntries.length === 0 ? (
          <div className="empty-state">
            <p>No transactions {filter !== 'all' ? `with status "${filter}"` : ''}</p>
          </div>
        ) : (
          filteredEntries.map(entry => (
            <div
              key={entry.id}
              className={`transaction-item ${getStatusColor(entry.overallStatus)}`}
              onClick={() => setSelectedEntry(selectedEntry?.id === entry.id ? null : entry)}
            >
              <div className="transaction-main">
                <div className="status-icon">
                  {getStatusIcon(entry.overallStatus)}
                </div>
                <div className="transaction-info">
                  <div className="transaction-id">
                    Txn: {entry.transactionId.substring(0, 12)}...
                  </div>
                  <div className="transaction-time">
                    {formatDate(entry.timestamp)}
                  </div>
                  <div className="wallet-address">
                    {entry.walletAddress.substring(0, 10)}...
                    {entry.walletAddress.substring(entry.walletAddress.length - 8)}
                  </div>
                </div>
              </div>

              <div className="transaction-status">
                <span className={`status-badge ${getStatusColor(entry.overallStatus)}`}>
                  {entry.overallStatus.replace(/_/g, ' ').toUpperCase()}
                </span>
              </div>

              {selectedEntry?.id === entry.id && (
                <div className="transaction-details">
                  <h4>Compliance Checks</h4>
                  <div className="checks-list">
                    {entry.checks.map((check, idx) => (
                      <div key={idx} className="check-item">
                        <div className="check-header">
                          <span className="check-type">
                            {check.checkType.replace(/_/g, ' ').toUpperCase()}
                          </span>
                          <span
                            className={`check-status ${check.passed ? 'passed' : 'failed'}`}
                          >
                            {check.passed ? '✓ Passed' : '✕ Failed'}
                          </span>
                        </div>
                        <p className="check-message">{check.message}</p>

                        <div className="risk-info">
                          <span className="risk-score">
                            Risk Score: {check.riskScore}
                          </span>
                          <span className={`risk-level ${check.riskLevel}`}>
                            {check.riskLevel.toUpperCase()}
                          </span>
                        </div>

                        {check.blockedReason && (
                          <div className="blocked-reason">
                            <strong>Block Reason:</strong> {check.blockedReason}
                          </div>
                        )}

                        {check.requiresManualReview && (
                          <div className="review-notice">
                            ⚠ This check requires manual review
                          </div>
                        )}
                      </div>
                    ))}
                  </div>

                  {entry.notes && (
                    <div className="audit-notes">
                      <strong>Notes:</strong>
                      <p>{entry.notes}</p>
                    </div>
                  )}
                </div>
              )}
            </div>
          ))
        )}
      </div>

      {/* Info Section */}
      <div className="monitoring-info">
        <details>
          <summary>ℹ️ How Transaction Monitoring Works</summary>
          <div className="info-content">
            <p>
              Transactions are monitored for compliance with KYC/AML regulations through
              automated compliance checks.
            </p>
            <h4>Compliance Checks Performed:</h4>
            <ul>
              <li><strong>KYC Verification:</strong> Validates user verification status</li>
              <li><strong>Risk Assessment:</strong> Evaluates transaction risk factors</li>
              <li><strong>Sanctions Screening:</strong> Checks against OFAC lists</li>
              <li><strong>Velocity Check:</strong> Detects unusual transaction patterns</li>
              <li><strong>Amount Verification:</strong> Validates transaction amounts</li>
            </ul>
            <h4>Status Indicators:</h4>
            <ul>
              <li><strong>✓ Compliant:</strong> Transaction passed all checks</li>
              <li>
                <strong>⚠ Review Required:</strong> Transaction needs manual review
              </li>
              <li><strong>✕ Blocked:</strong> Transaction failed compliance checks</li>
            </ul>
          </div>
        </details>
      </div>
    </div>
  );
}
