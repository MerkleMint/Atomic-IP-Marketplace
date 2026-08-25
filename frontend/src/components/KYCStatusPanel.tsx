import React, { useEffect, useState } from 'react';
import './KYCStatusPanel.css';
import { kycService, KYCStatus } from '../services/kycService';
import { riskScoringService } from '../services/complianceService';

interface KYCStatusPanelProps {
  userId: string;
  walletAddress: string;
  onVerificationNeeded?: (level: string) => void;
}

interface VerificationStats {
  level: string;
  status: string;
  riskScore: number;
  riskLevel: string;
  daysUntilExpiry: number | null;
  transactionLimit: number;
}

export function KYCStatusPanel({
  userId,
  walletAddress,
  onVerificationNeeded,
}: KYCStatusPanelProps) {
  const [kycStatus, setKycStatus] = useState<KYCStatus | null>(null);
  const [stats, setStats] = useState<VerificationStats | null>(null);
  const [isInitiatingVerification, setIsInitiatingVerification] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadKYCStatus();
  }, [userId]);

  const loadKYCStatus = async () => {
    try {
      setLoading(true);
      const status = await kycService.getKYCStatus(userId);
      setKycStatus(status);

      if (status) {
        const daysRemaining = status.expiresAt
          ? Math.ceil((status.expiresAt - Date.now()) / (24 * 60 * 60 * 1000))
          : null;

        const transactionLimit = getTransactionLimit(status.verificationLevel);

        setStats({
          level: status.verificationLevel,
          status: status.status,
          riskScore: status.riskScore,
          riskLevel: status.riskLevel,
          daysUntilExpiry: daysRemaining,
          transactionLimit,
        });
      }
      setError(null);
    } catch (err) {
      console.error('Failed to load KYC status:', err);
      setError('Failed to load verification status');
    } finally {
      setLoading(false);
    }
  };

  const getTransactionLimit = (level: string): number => {
    switch (level) {
      case 'LEVEL_1':
        return 10000;
      case 'LEVEL_2':
        return 100000;
      case 'LEVEL_3':
        return 1000000;
      default:
        return 1000;
    }
  };

  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'approved':
        return 'status-approved';
      case 'rejected':
        return 'status-rejected';
      case 'expired':
        return 'status-expired';
      default:
        return 'status-pending';
    }
  };

  const getRiskColor = (riskLevel: string): string => {
    switch (riskLevel) {
      case 'low':
        return 'risk-low';
      case 'medium':
        return 'risk-medium';
      case 'high':
        return 'risk-high';
      case 'critical':
        return 'risk-critical';
      default:
        return 'risk-unknown';
    }
  };

  const handleInitiateVerification = async () => {
    setIsInitiatingVerification(true);
    setError(null);

    try {
      const response = await kycService.initiateVerification({
        userId,
        walletAddress,
        level: 'LEVEL_1',
        userEmail: '', // Would be provided by user
      });

      if (response.status === 'approved') {
        await loadKYCStatus();
        alert('Verification approved!');
      } else if (response.verificationUrl) {
        // Redirect to external provider
        window.open(response.verificationUrl, '_blank');
      } else {
        setError(response.message);
      }
    } catch (err) {
      setError(`Verification failed: ${err instanceof Error ? err.message : 'Unknown error'}`);
    } finally {
      setIsInitiatingVerification(false);
    }
  };

  const handleUpgradeVerification = async () => {
    onVerificationNeeded?.(kycStatus?.verificationLevel || 'LEVEL_1');
  };

  if (loading) {
    return (
      <div className="kyc-status-panel loading">
        <div className="spinner"></div>
        <p>Loading verification status...</p>
      </div>
    );
  }

  return (
    <div className="kyc-status-panel">
      <div className="kyc-header">
        <h3>KYC Verification Status</h3>
        <button
          className="refresh-btn"
          onClick={loadKYCStatus}
          title="Refresh verification status"
        >
          ↻
        </button>
      </div>

      {error && (
        <div className="kyc-error">
          <span className="error-icon">⚠</span>
          <p>{error}</p>
        </div>
      )}

      {!kycStatus ? (
        <div className="kyc-not-verified">
          <div className="verification-icon">🔒</div>
          <h4>No Verification</h4>
          <p>You haven't completed any KYC verification yet.</p>
          <p className="transaction-limit">
            Current limit: $1,000 per transaction
          </p>
          <button
            className="btn-primary"
            onClick={handleInitiateVerification}
            disabled={isInitiatingVerification}
          >
            {isInitiatingVerification ? 'Initiating...' : 'Start Verification'}
          </button>
        </div>
      ) : (
        <div className={`kyc-verified ${getStatusColor(kycStatus.status)}`}>
          <div className="verification-level">
            <span className="level-badge">{kycStatus.verificationLevel}</span>
            <span className={`status-badge ${getStatusColor(kycStatus.status)}`}>
              {kycStatus.status.charAt(0).toUpperCase() + kycStatus.status.slice(1)}
            </span>
          </div>

          <div className="kyc-stats">
            <div className="stat-item">
              <label>Risk Score</label>
              <div className={`risk-score ${getRiskColor(kycStatus.riskLevel)}`}>
                <span className="score">{kycStatus.riskScore}</span>
                <span className="level">{kycStatus.riskLevel.toUpperCase()}</span>
              </div>
            </div>

            <div className="stat-item">
              <label>Transaction Limit</label>
              <p className="limit">
                ${stats?.transactionLimit.toLocaleString() || '—'}
              </p>
            </div>

            {kycStatus.verifiedAt && (
              <div className="stat-item">
                <label>Verified Date</label>
                <p>
                  {new Date(kycStatus.verifiedAt).toLocaleDateString()}
                </p>
              </div>
            )}

            {kycStatus.expiresAt && (
              <div className="stat-item">
                <label>Expires In</label>
                <p className={stats?.daysUntilExpiry ?? 0 < 7 ? 'warning' : ''}>
                  {stats?.daysUntilExpiry ?? 0} days
                </p>
              </div>
            )}
          </div>

          {kycStatus.status === 'approved' && (
            <div className="kyc-actions">
              {kycStatus.verificationLevel !== 'LEVEL_3' && (
                <button
                  className="btn-secondary"
                  onClick={handleUpgradeVerification}
                >
                  Upgrade Verification
                </button>
              )}
              <button
                className="btn-secondary"
                onClick={loadKYCStatus}
              >
                Refresh Status
              </button>
            </div>
          )}

          {kycStatus.status === 'expired' && (
            <div className="kyc-actions">
              <button
                className="btn-primary"
                onClick={handleInitiateVerification}
                disabled={isInitiatingVerification}
              >
                Renew Verification
              </button>
            </div>
          )}
        </div>
      )}

      <div className="kyc-info">
        <details>
          <summary>ℹ️ Why KYC Verification?</summary>
          <div className="info-content">
            <p>
              KYC (Know Your Customer) verification helps us comply with financial
              regulations and prevent fraud. Higher verification levels unlock higher
              transaction limits.
            </p>
            <h4>Verification Levels:</h4>
            <ul>
              <li><strong>Level 1:</strong> Email verification - $10,000 limit</li>
              <li><strong>Level 2:</strong> Enhanced verification - $100,000 limit</li>
              <li><strong>Level 3:</strong> Full verification - $1M+ limit</li>
            </ul>
            <p>
              Your data is encrypted and handled in accordance with GDPR regulations.
            </p>
          </div>
        </details>
      </div>
    </div>
  );
}
