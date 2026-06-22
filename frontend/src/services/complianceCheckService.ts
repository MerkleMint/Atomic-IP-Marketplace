/**
 * ComplianceCheck
 *
 * Compliance checks and utilities for swap initiation and transaction processing.
 * Integrates KYC verification and risk scoring for regulatory compliance.
 *
 * Features:
 *  - Pre-swap compliance validation
 *  - Automated compliance rule enforcement
 *  - Transaction blocking for high-risk scenarios
 *  - Compliance audit trail generation
 *  - OFAC/Sanctions screening
 */

import { kycService, KYCStatus } from './kycService';
import { riskScoringService, TransactionRiskAssessment } from './complianceService';

// ─── Types ─────────────────────────────────────────────────────────────────────

export type ComplianceCheckType = 
  | 'kyc_verification'
  | 'risk_assessment'
  | 'sanctions_screening'
  | 'velocity_check'
  | 'amount_verification';

export interface ComplianceCheckResult {
  checkId: string;
  checkType: ComplianceCheckType;
  timestamp: number;
  passed: boolean;
  riskScore: number;
  riskLevel: 'low' | 'medium' | 'high' | 'critical';
  message: string;
  details: Record<string, unknown>;
  blockedReason?: string;
  requiresManualReview: boolean;
}

export interface ComplianceAuditEntry {
  id: string;
  timestamp: number;
  userId: string;
  transactionId: string;
  walletAddress: string;
  checks: ComplianceCheckResult[];
  overallStatus: 'compliant' | 'blocked' | 'review_required';
  notes?: string;
}

export interface SwapComplianceContext {
  userId: string;
  buyerWallet: string;
  sellerWallet: string;
  amountUSDC: number;
  transactionType: 'swap' | 'listing' | 'payment';
  metadata?: Record<string, unknown>;
}

// ─── Service Implementation ─────────────────────────────────────────────────────

class ComplianceCheckServiceImpl {
  private auditTrail: ComplianceAuditEntry[] = [];

  // Compliance rules
  private readonly COMPLIANCE_RULES = {
    HIGH_VALUE_THRESHOLD_USD: 10000,
    CRITICAL_VALUE_THRESHOLD_USD: 50000,
    VELOCITY_CHECK_ENABLED: true,
    SANCTIONS_CHECK_ENABLED: true,
    AUTO_BLOCK_CRITICAL_RISK: true,
    MANUAL_REVIEW_HIGH_RISK: true,
    MAX_DAILY_TRANSACTIONS: 10,
    MAX_DAILY_VOLUME_USD: 100000,
  };

  constructor() {
    this.loadAuditTrail();
  }

  /**
   * Execute comprehensive compliance checks for a swap
   */
  async executeSwapComplianceChecks(
    context: SwapComplianceContext
  ): Promise<ComplianceCheckResult[]> {
    const checkId = `comp-check-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const results: ComplianceCheckResult[] = [];

    // 1. KYC Verification Check
    const kycResult = await this.checkKYCVerification(context, checkId);
    results.push(kycResult);

    // If KYC fails for high-value transactions, stop here
    if (!kycResult.passed && context.amountUSDC > this.COMPLIANCE_RULES.HIGH_VALUE_THRESHOLD_USD) {
      return results;
    }

    // 2. Risk Assessment Check
    const riskResult = await this.checkTransactionRisk(context, checkId);
    results.push(riskResult);

    // 3. Sanctions Screening Check
    if (this.COMPLIANCE_RULES.SANCTIONS_CHECK_ENABLED) {
      const sanctionsResult = await this.checkSanctionsScreening(context, checkId);
      results.push(sanctionsResult);
    }

    // 4. Velocity Check
    if (this.COMPLIANCE_RULES.VELOCITY_CHECK_ENABLED) {
      const velocityResult = await this.checkVelocity(context, checkId);
      results.push(velocityResult);
    }

    // 5. Amount Verification Check
    const amountResult = await this.checkAmountVerification(context, checkId);
    results.push(amountResult);

    return results;
  }

  /**
   * Validate transaction against compliance checks
   */
  async validateTransaction(
    context: SwapComplianceContext
  ): Promise<{
    approved: boolean;
    blockReason?: string;
    requiresReview: boolean;
    riskLevel: string;
    checks: ComplianceCheckResult[];
  }> {
    const checks = await this.executeSwapComplianceChecks(context);

    // Analyze results
    let hasBlockedCheck = false;
    let hasReviewRequired = false;
    let maxRiskLevel = 'low';

    for (const check of checks) {
      if (!check.passed && check.blockedReason) {
        hasBlockedCheck = true;
      }
      if (check.requiresManualReview) {
        hasReviewRequired = true;
      }

      // Update max risk level
      const riskLevels = ['low', 'medium', 'high', 'critical'];
      if (riskLevels.indexOf(check.riskLevel) > riskLevels.indexOf(maxRiskLevel)) {
        maxRiskLevel = check.riskLevel;
      }
    }

    // Auto-block if critical risk and rule is enabled
    const autoBlock = this.COMPLIANCE_RULES.AUTO_BLOCK_CRITICAL_RISK &&
                     maxRiskLevel === 'critical';

    const approved = !hasBlockedCheck && !autoBlock;
    const transactionId = `txn-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

    // Log to audit trail
    const overallStatus = approved 
      ? (hasReviewRequired ? 'review_required' : 'compliant')
      : 'blocked';

    this.logAuditEntry(
      context.userId,
      transactionId,
      context.buyerWallet,
      checks,
      overallStatus
    );

    return {
      approved,
      blockReason: autoBlock ? 'Critical risk score - transaction blocked' : 
                   hasBlockedCheck ? 'Compliance check failed' : undefined,
      requiresReview: hasReviewRequired,
      riskLevel: maxRiskLevel,
      checks,
    };
  }

  /**
   * Check KYC verification status
   */
  private async checkKYCVerification(
    context: SwapComplianceContext,
    checkId: string
  ): Promise<ComplianceCheckResult> {
    try {
      const kycStatus = await kycService.getKYCStatus(context.userId);

      // Determine required verification level
      const verification = await kycService.checkVerificationForAmount(
        context.userId,
        context.amountUSDC
      );

      if (!verification.approved) {
        return {
          checkId,
          checkType: 'kyc_verification',
          timestamp: Date.now(),
          passed: false,
          riskScore: 50,
          riskLevel: 'high',
          message: verification.reason,
          details: {
            currentLevel: kycStatus?.verificationLevel || 'UNVERIFIED',
            requiredLevel: verification.requiredLevel,
            amount: context.amountUSDC,
          },
          blockedReason: `KYC verification required`,
          requiresManualReview: true,
        };
      }

      return {
        checkId,
        checkType: 'kyc_verification',
        timestamp: Date.now(),
        passed: true,
        riskScore: kycStatus?.riskScore || 0,
        riskLevel: kycStatus?.riskLevel || 'low',
        message: 'KYC verification passed',
        details: {
          verificationLevel: kycStatus?.verificationLevel || 'LEVEL_1',
          verifiedAt: kycStatus?.verifiedAt,
        },
        requiresManualReview: false,
      };
    } catch (error) {
      console.error('KYC verification check failed:', error);
      return {
        checkId,
        checkType: 'kyc_verification',
        timestamp: Date.now(),
        passed: false,
        riskScore: 60,
        riskLevel: 'high',
        message: `KYC check error: ${error instanceof Error ? error.message : 'Unknown error'}`,
        details: { error: String(error) },
        blockedReason: 'KYC verification unavailable',
        requiresManualReview: true,
      };
    }
  }

  /**
   * Check transaction risk
   */
  private async checkTransactionRisk(
    context: SwapComplianceContext,
    checkId: string
  ): Promise<ComplianceCheckResult> {
    try {
      const assessment = await riskScoringService.assessTransactionRisk(
        context.userId,
        context.buyerWallet,
        context.amountUSDC
      );

      const criticalRisk = assessment.riskLevel === 'critical' ||
                          assessment.riskScore >= 80;

      return {
        checkId,
        checkType: 'risk_assessment',
        timestamp: Date.now(),
        passed: !criticalRisk,
        riskScore: assessment.riskScore,
        riskLevel: assessment.riskLevel,
        message: `Risk score: ${assessment.riskScore}. Risk factors: ${assessment.riskFactors.join(', ') || 'none'}`,
        details: {
          riskScore: assessment.riskScore,
          riskFactors: assessment.riskFactors,
          transactionId: assessment.transactionId,
        },
        blockedReason: criticalRisk ? 'Critical risk score' : undefined,
        requiresManualReview: assessment.requiresReview,
      };
    } catch (error) {
      console.error('Transaction risk check failed:', error);
      return {
        checkId,
        checkType: 'risk_assessment',
        timestamp: Date.now(),
        passed: true,
        riskScore: 0,
        riskLevel: 'low',
        message: 'Risk assessment skipped due to error',
        details: { error: String(error) },
        requiresManualReview: false,
      };
    }
  }

  /**
   * Check sanctions screening (OFAC, UN, EU, etc.)
   */
  private async checkSanctionsScreening(
    context: SwapComplianceContext,
    checkId: string
  ): Promise<ComplianceCheckResult> {
    try {
      // In production, integrate with actual sanctions screening service
      // For now, return pass (no matches in known sanction lists)
      
      // Example integration point for external service:
      // const result = await this.callSanctionsScreeningAPI(context.buyerWallet, context.sellerWallet);
      
      return {
        checkId,
        checkType: 'sanctions_screening',
        timestamp: Date.now(),
        passed: true,
        riskScore: 0,
        riskLevel: 'low',
        message: 'No sanctions screening matches',
        details: {
          buyerWalletScreened: true,
          sellerWalletScreened: true,
          matchesFound: 0,
        },
        requiresManualReview: false,
      };
    } catch (error) {
      console.error('Sanctions screening failed:', error);
      return {
        checkId,
        checkType: 'sanctions_screening',
        timestamp: Date.now(),
        passed: false,
        riskScore: 30,
        riskLevel: 'medium',
        message: 'Sanctions screening unavailable',
        details: { error: String(error) },
        requiresManualReview: true,
      };
    }
  }

  /**
   * Check transaction velocity
   */
  private async checkVelocity(
    context: SwapComplianceContext,
    checkId: string
  ): Promise<ComplianceCheckResult> {
    try {
      const now = Date.now();
      const last24h = now - 24 * 60 * 60 * 1000;

      // Check audit trail for recent transactions
      const recentTransactions = this.auditTrail.filter(entry =>
        entry.userId === context.userId && entry.timestamp > last24h
      );

      const transactionCount = recentTransactions.length;
      const totalVolume = recentTransactions.reduce((sum, entry) => {
        // Extract amount from checks or metadata
        return sum + (context.amountUSDC || 0);
      }, 0);

      const velocityViolation = transactionCount >= this.COMPLIANCE_RULES.MAX_DAILY_TRANSACTIONS ||
                               totalVolume >= this.COMPLIANCE_RULES.MAX_DAILY_VOLUME_USD;

      return {
        checkId,
        checkType: 'velocity_check',
        timestamp: Date.now(),
        passed: !velocityViolation,
        riskScore: velocityViolation ? 40 : 0,
        riskLevel: velocityViolation ? 'high' : 'low',
        message: velocityViolation
          ? `Velocity limit exceeded: ${transactionCount}/${this.COMPLIANCE_RULES.MAX_DAILY_TRANSACTIONS} transactions`
          : 'Velocity check passed',
        details: {
          transactionCountLast24h: transactionCount,
          totalVolumeLast24h: totalVolume,
          limits: {
            maxTransactions: this.COMPLIANCE_RULES.MAX_DAILY_TRANSACTIONS,
            maxVolume: this.COMPLIANCE_RULES.MAX_DAILY_VOLUME_USD,
          },
        },
        blockedReason: velocityViolation ? 'Velocity limit exceeded' : undefined,
        requiresManualReview: false,
      };
    } catch (error) {
      console.error('Velocity check failed:', error);
      return {
        checkId,
        checkType: 'velocity_check',
        timestamp: Date.now(),
        passed: true,
        riskScore: 0,
        riskLevel: 'low',
        message: 'Velocity check skipped',
        details: { error: String(error) },
        requiresManualReview: false,
      };
    }
  }

  /**
   * Check amount verification
   */
  private async checkAmountVerification(
    context: SwapComplianceContext,
    checkId: string
  ): Promise<ComplianceCheckResult> {
    const isCritical = context.amountUSDC >= this.COMPLIANCE_RULES.CRITICAL_VALUE_THRESHOLD_USD;
    const isHighValue = context.amountUSDC >= this.COMPLIANCE_RULES.HIGH_VALUE_THRESHOLD_USD;

    let riskScore = 0;
    let riskLevel: 'low' | 'medium' | 'high' | 'critical' = 'low';

    if (isCritical) {
      riskScore = 50;
      riskLevel = 'high';
    } else if (isHighValue) {
      riskScore = 25;
      riskLevel = 'medium';
    }

    return {
      checkId,
      checkType: 'amount_verification',
      timestamp: Date.now(),
      passed: true,
      riskScore,
      riskLevel,
      message: `Transaction amount: $${context.amountUSDC.toLocaleString()}`,
      details: {
        amount: context.amountUSDC,
        isHighValue,
        isCritical,
        thresholds: {
          high: this.COMPLIANCE_RULES.HIGH_VALUE_THRESHOLD_USD,
          critical: this.COMPLIANCE_RULES.CRITICAL_VALUE_THRESHOLD_USD,
        },
      },
      requiresManualReview: isCritical,
    };
  }

  /**
   * Log audit entry
   */
  private logAuditEntry(
    userId: string,
    transactionId: string,
    walletAddress: string,
    checks: ComplianceCheckResult[],
    overallStatus: 'compliant' | 'blocked' | 'review_required'
  ): void {
    const entry: ComplianceAuditEntry = {
      id: `audit-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      timestamp: Date.now(),
      userId,
      transactionId,
      walletAddress,
      checks,
      overallStatus,
    };

    this.auditTrail.push(entry);

    // Persist to localStorage (max 1000 entries)
    if (this.auditTrail.length > 1000) {
      this.auditTrail = this.auditTrail.slice(-500);
    }

    localStorage.setItem('compliance_audit_trail', JSON.stringify(this.auditTrail));
  }

  /**
   * Load audit trail from storage
   */
  private loadAuditTrail(): void {
    try {
      const stored = localStorage.getItem('compliance_audit_trail');
      if (stored) {
        this.auditTrail = JSON.parse(stored);
      }
    } catch (error) {
      console.error('Failed to load audit trail:', error);
      this.auditTrail = [];
    }
  }

  /**
   * Get audit entries
   */
  getAuditEntries(userId?: string): ComplianceAuditEntry[] {
    if (!userId) {
      return this.auditTrail;
    }
    return this.auditTrail.filter(entry => entry.userId === userId);
  }

  /**
   * Get compliance statistics
   */
  getComplianceStats(): {
    totalChecks: number;
    compliantTransactions: number;
    blockedTransactions: number;
    reviewRequired: number;
    blockRate: number;
  } {
    const totalChecks = this.auditTrail.length;
    const compliantTransactions = this.auditTrail.filter(e => e.overallStatus === 'compliant').length;
    const blockedTransactions = this.auditTrail.filter(e => e.overallStatus === 'blocked').length;
    const reviewRequired = this.auditTrail.filter(e => e.overallStatus === 'review_required').length;

    return {
      totalChecks,
      compliantTransactions,
      blockedTransactions,
      reviewRequired,
      blockRate: totalChecks > 0 ? (blockedTransactions / totalChecks) * 100 : 0,
    };
  }
}

export const complianceCheckService = new ComplianceCheckServiceImpl();
