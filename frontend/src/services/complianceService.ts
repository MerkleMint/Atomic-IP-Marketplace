/**
 * RiskScoringService
 *
 * Transaction risk scoring and suspicious activity detection.
 * Implements machine learning-like heuristics for compliance scoring.
 *
 * Risk Factors:
 *  - Transaction amount and frequency
 *  - User behavior patterns (velocity checks)
 *  - Geographic indicators
 *  - Wallet age and reputation
 *  - Transaction pattern anomalies
 */

// ─── Types ─────────────────────────────────────────────────────────────────────

export type RiskFactor = 
  | 'high_velocity'
  | 'unusual_amount'
  | 'unusual_pattern'
  | 'new_wallet'
  | 'geographic_mismatch'
  | 'high_frequency'
  | 'sanctioned_wallet'
  | 'mixing_service';

export interface TransactionRiskAssessment {
  transactionId: string;
  userId: string;
  walletAddress: string;
  amount: number;
  timestamp: number;
  riskScore: number; // 0-100
  riskLevel: 'low' | 'medium' | 'high' | 'critical';
  riskFactors: RiskFactor[];
  requiresReview: boolean;
  flaggedAt?: number;
  reviewNotes?: string;
}

export interface UserTransactionHistory {
  userId: string;
  walletAddress: string;
  transactions: Array<{
    amount: number;
    timestamp: number;
    status: 'pending' | 'completed' | 'failed';
  }>;
  averageTransactionAmount: number;
  maxTransactionAmount: number;
  transactionFrequency24h: number; // number of transactions in last 24h
  totalVolume24h: number; // sum of transaction amounts in 24h
  walletAge: number; // ms since first transaction
  suspiciousActivityCount: number;
}

// ─── Service Implementation ─────────────────────────────────────────────────────

class RiskScoringServiceImpl {
  private transactionHistory: Map<string, UserTransactionHistory> = new Map();
  private flaggedTransactions: Map<string, TransactionRiskAssessment> = new Map();

  // Risk thresholds
  private readonly RISK_THRESHOLDS = {
    CRITICAL: 80,
    HIGH: 60,
    MEDIUM: 40,
    LOW: 0,
  };

  // Velocity limits per hour
  private readonly VELOCITY_LIMITS = {
    LEVEL_1: { maxTransactions: 5, maxVolume: 50000 },
    LEVEL_2: { maxTransactions: 20, maxVolume: 500000 },
    LEVEL_3: { maxTransactions: 100, maxVolume: 5000000 },
  };

  constructor() {
    this.loadTransactionHistory();
  }

  /**
   * Assess transaction risk
   */
  async assessTransactionRisk(
    userId: string,
    walletAddress: string,
    amount: number,
    verificationLevel: string = 'LEVEL_1'
  ): Promise<TransactionRiskAssessment> {
    const transactionId = `txn-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const timestamp = Date.now();

    // Get or create transaction history
    const history = await this.getOrCreateHistory(userId, walletAddress);

    // Calculate risk score components
    let riskScore = 0;
    const riskFactors: RiskFactor[] = [];

    // 1. High velocity check (30 points max)
    const velocityRisk = this.checkVelocity(history, verificationLevel, amount);
    riskScore += velocityRisk.score;
    if (velocityRisk.flagged) riskFactors.push('high_velocity');

    // 2. Unusual amount check (25 points max)
    const amountRisk = this.checkUnusualAmount(history, amount);
    riskScore += amountRisk.score;
    if (amountRisk.flagged) riskFactors.push('unusual_amount');

    // 3. Pattern anomaly check (20 points max)
    const patternRisk = this.checkPatternAnomaly(history, amount);
    riskScore += patternRisk.score;
    if (patternRisk.flagged) riskFactors.push('unusual_pattern');

    // 4. Wallet age check (15 points max)
    const ageRisk = this.checkWalletAge(history);
    riskScore += ageRisk.score;
    if (ageRisk.flagged) riskFactors.push('new_wallet');

    // 5. Transaction frequency check (10 points max)
    const frequencyRisk = this.checkTransactionFrequency(history);
    riskScore += frequencyRisk.score;
    if (frequencyRisk.flagged) riskFactors.push('high_frequency');

    // Clamp score between 0-100
    riskScore = Math.min(100, Math.max(0, riskScore));

    // Determine risk level
    const riskLevel = this.getRiskLevel(riskScore);
    const requiresReview = riskScore >= this.RISK_THRESHOLDS.HIGH || 
                          riskFactors.length >= 3;

    const assessment: TransactionRiskAssessment = {
      transactionId,
      userId,
      walletAddress,
      amount,
      timestamp,
      riskScore,
      riskLevel,
      riskFactors,
      requiresReview,
      flaggedAt: requiresReview ? timestamp : undefined,
    };

    // Store assessment
    this.flaggedTransactions.set(transactionId, assessment);
    this.persistFlaggedTransactions();

    // Update history
    history.transactions.push({ amount, timestamp, status: 'pending' });
    this.transactionHistory.set(`${userId}-${walletAddress}`, history);
    this.persistTransactionHistory();

    return assessment;
  }

  /**
   * Check velocity against user's verification level
   */
  private checkVelocity(
    history: UserTransactionHistory,
    verificationLevel: string,
    newAmount: number
  ): { score: number; flagged: boolean } {
    const now = Date.now();
    const oneHourAgo = now - 60 * 60 * 1000;

    // Get transactions in last hour
    const recentTransactions = history.transactions.filter(
      t => t.timestamp > oneHourAgo && t.status === 'completed'
    );

    const recentVolume = recentTransactions.reduce((sum, t) => sum + t.amount, 0) + newAmount;
    const limits = this.VELOCITY_LIMITS[verificationLevel as keyof typeof this.VELOCITY_LIMITS] || 
                   this.VELOCITY_LIMITS.LEVEL_1;

    // Check against limits
    const volumeExceeded = recentVolume > limits.maxVolume;
    const transactionCountExceeded = recentTransactions.length >= limits.maxTransactions;

    if (volumeExceeded || transactionCountExceeded) {
      return {
        score: 30,
        flagged: true,
      };
    }

    // Calculate proportional score if close to limit
    const volumePercentage = (recentVolume / limits.maxVolume) * 100;
    const transactionPercentage = (recentTransactions.length / limits.maxTransactions) * 100;
    const utilizationPercentage = Math.max(volumePercentage, transactionPercentage);

    return {
      score: utilizationPercentage > 80 ? 15 : utilizationPercentage > 60 ? 8 : 0,
      flagged: utilizationPercentage > 80,
    };
  }

  /**
   * Check for unusual transaction amounts
   */
  private checkUnusualAmount(
    history: UserTransactionHistory,
    newAmount: number
  ): { score: number; flagged: boolean } {
    if (history.transactions.length < 3) {
      // Not enough history
      return { score: 0, flagged: false };
    }

    const avgAmount = history.averageTransactionAmount;
    const maxAmount = history.maxTransactionAmount;

    // Flag if significantly larger than historical average
    if (newAmount > avgAmount * 5) {
      return { score: 25, flagged: true };
    }

    if (newAmount > avgAmount * 3) {
      return { score: 15, flagged: false };
    }

    if (newAmount > avgAmount * 2) {
      return { score: 8, flagged: false };
    }

    return { score: 0, flagged: false };
  }

  /**
   * Check for pattern anomalies
   */
  private checkPatternAnomaly(
    history: UserTransactionHistory,
    newAmount: number
  ): { score: number; flagged: boolean } {
    if (history.transactions.length < 5) {
      return { score: 0, flagged: false };
    }

    // Check for structured deposits (amounts designed to avoid thresholds)
    const roundedAmounts = history.transactions.map(t => t.amount)
      .filter(a => a % 1000 === 0 || a % 5000 === 0 || a % 10000 === 0).length;

    const structuringRatio = roundedAmounts / history.transactions.length;

    if (structuringRatio > 0.7) {
      return { score: 20, flagged: true };
    }

    if (structuringRatio > 0.5) {
      return { score: 10, flagged: false };
    }

    return { score: 0, flagged: false };
  }

  /**
   * Check wallet age
   */
  private checkWalletAge(history: UserTransactionHistory): { score: number; flagged: boolean } {
    const walletAgeHours = history.walletAge / (60 * 60 * 1000);

    // Very new wallet (less than 1 hour)
    if (walletAgeHours < 1) {
      return { score: 15, flagged: true };
    }

    // New wallet (less than 24 hours)
    if (walletAgeHours < 24) {
      return { score: 10, flagged: false };
    }

    // New wallet (less than 7 days)
    if (walletAgeHours < 168) {
      return { score: 5, flagged: false };
    }

    return { score: 0, flagged: false };
  }

  /**
   * Check transaction frequency
   */
  private checkTransactionFrequency(
    history: UserTransactionHistory
  ): { score: number; flagged: boolean } {
    const now = Date.now();
    const last24h = now - 24 * 60 * 60 * 1000;

    const transactionsLast24h = history.transactions.filter(t => t.timestamp > last24h).length;

    // More than 50 transactions in 24h
    if (transactionsLast24h > 50) {
      return { score: 10, flagged: true };
    }

    // More than 20 transactions in 24h
    if (transactionsLast24h > 20) {
      return { score: 5, flagged: false };
    }

    return { score: 0, flagged: false };
  }

  /**
   * Get or create transaction history for user
   */
  private async getOrCreateHistory(
    userId: string,
    walletAddress: string
  ): Promise<UserTransactionHistory> {
    const key = `${userId}-${walletAddress}`;

    if (this.transactionHistory.has(key)) {
      return this.transactionHistory.get(key)!;
    }

    const now = Date.now();
    const history: UserTransactionHistory = {
      userId,
      walletAddress,
      transactions: [],
      averageTransactionAmount: 0,
      maxTransactionAmount: 0,
      transactionFrequency24h: 0,
      totalVolume24h: 0,
      walletAge: now,
      suspiciousActivityCount: 0,
    };

    this.transactionHistory.set(key, history);
    return history;
  }

  /**
   * Get risk level from score
   */
  private getRiskLevel(score: number): 'low' | 'medium' | 'high' | 'critical' {
    if (score >= this.RISK_THRESHOLDS.CRITICAL) return 'critical';
    if (score >= this.RISK_THRESHOLDS.HIGH) return 'high';
    if (score >= this.RISK_THRESHOLDS.MEDIUM) return 'medium';
    return 'low';
  }

  /**
   * Update transaction status
   */
  updateTransactionStatus(
    transactionId: string,
    status: 'completed' | 'failed'
  ): void {
    const assessment = this.flaggedTransactions.get(transactionId);
    if (assessment) {
      // Update in history
      const key = `${assessment.userId}-${assessment.walletAddress}`;
      const history = this.transactionHistory.get(key);
      if (history) {
        const transaction = history.transactions.find(
          t => t.timestamp === assessment.timestamp
        );
        if (transaction) {
          transaction.status = status;
        }
        // Recalculate statistics
        this.updateHistoryStats(history);
        this.transactionHistory.set(key, history);
        this.persistTransactionHistory();
      }
    }
  }

  /**
   * Update history statistics
   */
  private updateHistoryStats(history: UserTransactionHistory): void {
    const completedTransactions = history.transactions.filter(t => t.status === 'completed');

    if (completedTransactions.length === 0) {
      history.averageTransactionAmount = 0;
      history.maxTransactionAmount = 0;
      history.totalVolume24h = 0;
      history.transactionFrequency24h = 0;
      return;
    }

    const amounts = completedTransactions.map(t => t.amount);
    history.averageTransactionAmount = amounts.reduce((a, b) => a + b, 0) / amounts.length;
    history.maxTransactionAmount = Math.max(...amounts);

    // Last 24h stats
    const last24h = Date.now() - 24 * 60 * 60 * 1000;
    const last24hTransactions = completedTransactions.filter(t => t.timestamp > last24h);
    history.transactionFrequency24h = last24hTransactions.length;
    history.totalVolume24h = last24hTransactions.reduce((sum, t) => sum + t.amount, 0);
  }

  /**
   * Get transaction assessment
   */
  getTransactionAssessment(transactionId: string): TransactionRiskAssessment | undefined {
    return this.flaggedTransactions.get(transactionId);
  }

  /**
   * Get flagged transactions for review
   */
  getFlaggedTransactions(userId?: string): TransactionRiskAssessment[] {
    const assessments = Array.from(this.flaggedTransactions.values());
    if (!userId) {
      return assessments.filter(a => a.requiresReview);
    }
    return assessments.filter(a => a.userId === userId && a.requiresReview);
  }

  /**
   * Persist transaction history to localStorage
   */
  private persistTransactionHistory(): void {
    const data = Array.from(this.transactionHistory.entries()).map(([key, value]) => ({
      key,
      value,
    }));
    localStorage.setItem('risk_transaction_history', JSON.stringify(data));
  }

  /**
   * Load transaction history from localStorage
   */
  private loadTransactionHistory(): void {
    try {
      const stored = localStorage.getItem('risk_transaction_history');
      if (stored) {
        const data = JSON.parse(stored);
        this.transactionHistory = new Map(data.map((item: any) => [item.key, item.value]));
      }
    } catch (error) {
      console.error('Failed to load transaction history:', error);
    }
  }

  /**
   * Persist flagged transactions to localStorage
   */
  private persistFlaggedTransactions(): void {
    const data = Array.from(this.flaggedTransactions.entries());
    localStorage.setItem('risk_flagged_transactions', JSON.stringify(data));
  }
}

export const riskScoringService = new RiskScoringServiceImpl();
