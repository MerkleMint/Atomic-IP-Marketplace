# KYC/AML Integration Example

This document provides practical examples of integrating the KYC/AML compliance framework into your application flows.

## Quick Start

### 1. Display KYC Status

Display the user's verification status in your UI:

```tsx
import { KYCStatusPanel } from './components/KYCStatusPanel';
import { useAuthContext } from './context/AuthContext';

export function MyListingsDashboard() {
  const { userId, walletAddress } = useAuthContext();

  return (
    <div className="dashboard">
      {/* Display KYC Status */}
      <KYCStatusPanel
        userId={userId}
        walletAddress={walletAddress}
        onVerificationNeeded={(level) => {
          console.log(`User needs to upgrade to ${level}`);
        }}
      />

      {/* Rest of component */}
    </div>
  );
}
```

### 2. Validate Transaction Before Swap

Check compliance before allowing a swap:

```tsx
import { complianceCheckService, SwapComplianceContext } from './services/complianceCheckService';
import { riskScoringService } from './services/complianceService';

async function initiateSwap(buyerUserId: string, swapDetails: SwapDetails) {
  try {
    // Prepare compliance context
    const complianceContext: SwapComplianceContext = {
      userId: buyerUserId,
      buyerWallet: swapDetails.buyerWallet,
      sellerWallet: swapDetails.sellerWallet,
      amountUSDC: swapDetails.amountUSDC,
      transactionType: 'swap',
      metadata: {
        listingId: swapDetails.listingId,
        ipAsset: swapDetails.ipAsset,
      },
    };

    // Run compliance validation
    const validation = await complianceCheckService.validateTransaction(complianceContext);

    if (!validation.approved) {
      // Transaction is blocked
      alert(`Transaction blocked: ${validation.blockReason}`);
      return;
    }

    if (validation.requiresReview) {
      // Transaction requires manual review
      alert(
        `Your transaction requires manual compliance review. ` +
        `Risk level: ${validation.riskLevel}`
      );
      // May want to notify user it could take longer to complete
    }

    // Log compliance checks for audit
    console.log('Compliance checks passed:', validation.checks);

    // Proceed with swap initiation
    await performSwapInitiation(swapDetails);
    alert('Swap initiated successfully!');
  } catch (error) {
    alert(`Failed to initiate swap: ${error}`);
  }
}
```

### 3. Monitor Transaction Risk in Real-Time

Display live transaction monitoring:

```tsx
import { TransactionMonitoringPanel } from './components/TransactionMonitoringPanel';
import { useAuthContext } from './context/AuthContext';

export function AdminDashboard() {
  const { userId } = useAuthContext();

  return (
    <div className="admin-dashboard">
      <h1>Compliance Dashboard</h1>

      {/* Show all transactions for admin */}
      <TransactionMonitoringPanel
        userId={undefined} // Show all users
        autoRefresh={true}
        refreshInterval={15000} // Refresh every 15 seconds
      />

      {/* Or show specific user's transactions */}
      <TransactionMonitoringPanel
        userId={userId}
        autoRefresh={true}
        refreshInterval={30000}
      />
    </div>
  );
}
```

### 4. Check KYC Status for Transaction Amount

Verify user can perform transaction at their verification level:

```tsx
import { kycService } from './services/kycService';

async function checkCanInitiateTransaction(
  userId: string,
  amountUSDC: number
): Promise<{ canProceed: boolean; reason: string }> {
  try {
    const verification = await kycService.checkVerificationForAmount(userId, amountUSDC);

    return {
      canProceed: verification.approved,
      reason: verification.reason,
    };
  } catch (error) {
    return {
      canProceed: false,
      reason: `Verification check failed: ${error}`,
    };
  }
}

// Usage
const result = await checkCanInitiateTransaction(userId, 50000);
if (!result.canProceed) {
  alert(`Cannot proceed with transaction: ${result.reason}`);
}
```

### 5. Assess Transaction Risk Score

Get detailed risk assessment:

```tsx
import { riskScoringService } from './services/complianceService';

async function assessTransactionRisk(
  userId: string,
  walletAddress: string,
  amount: number,
  verificationLevel: string
) {
  const assessment = await riskScoringService.assessTransactionRisk(
    userId,
    walletAddress,
    amount,
    verificationLevel
  );

  console.log('Risk Assessment Results:');
  console.log('- Risk Score:', assessment.riskScore);
  console.log('- Risk Level:', assessment.riskLevel);
  console.log('- Risk Factors:', assessment.riskFactors);
  console.log('- Requires Review:', assessment.requiresReview);

  // Take action based on risk level
  if (assessment.riskLevel === 'critical') {
    // Block transaction
    return { approved: false, message: 'Transaction blocked due to critical risk' };
  }

  if (assessment.requiresReview) {
    // Flag for manual review
    return { approved: true, flaggedForReview: true };
  }

  // Approve transaction
  return { approved: true };
}
```

## Integration with Swap Flow

### Complete Swap Initiation with Compliance

```tsx
import { InitiateSwapModal } from './components/InitiateSwapModal';
import { complianceCheckService } from './services/complianceCheckService';

export function SwapPage() {
  const [showComplianceWarning, setShowComplianceWarning] = useState(false);
  const [complianceDetails, setComplianceDetails] = useState(null);

  async function handleSwapSubmit(formData: SwapFormData) {
    try {
      // Step 1: Validate compliance
      const complianceContext: SwapComplianceContext = {
        userId: formData.buyerId,
        buyerWallet: formData.buyerWallet,
        sellerWallet: formData.sellerWallet,
        amountUSDC: formData.amountUSDC,
        transactionType: 'swap',
      };

      const validation = await complianceCheckService.validateTransaction(complianceContext);

      // Step 2: Handle compliance results
      if (!validation.approved) {
        // Show error dialog
        showError(`Swap blocked: ${validation.blockReason}`);
        return;
      }

      if (validation.requiresReview) {
        // Show warning
        setComplianceDetails({
          riskLevel: validation.riskLevel,
          checks: validation.checks,
          warning: 'This transaction requires manual compliance review and may take longer to process.',
        });
        setShowComplianceWarning(true);
        return;
      }

      // Step 3: Proceed with swap
      await submitSwap(formData);
      showSuccess('Swap initiated successfully!');
    } catch (error) {
      showError(`Failed to process swap: ${error}`);
    }
  }

  return (
    <>
      <InitiateSwapModal onSubmit={handleSwapSubmit} />

      {showComplianceWarning && (
        <ComplianceWarningDialog
          details={complianceDetails}
          onConfirm={() => {
            setShowComplianceWarning(false);
            // Proceed with swap
          }}
          onCancel={() => setShowComplianceWarning(false)}
        />
      )}
    </>
  );
}
```

## Audit Trail Access

### Retrieve and Review Audit Logs

```tsx
import { complianceCheckService } from './services/complianceCheckService';
import { kycService } from './services/kycService';

// Get compliance audit trail
const auditEntries = complianceCheckService.getAuditEntries(userId);

// Get KYC audit trail
const kycAuditLogs = kycService.getAuditLogs(userId);

// Export audit trail for compliance review
function exportAuditTrail(userId: string) {
  const complianceAudit = complianceCheckService.getAuditEntries(userId);
  const kycAudit = kycService.getAuditLogs(userId);

  const report = {
    exportDate: new Date().toISOString(),
    userId,
    complianceAudit,
    kycAudit,
    summary: {
      totalComplianceChecks: complianceAudit.length,
      totalKYCEvents: kycAudit.length,
    },
  };

  // Convert to JSON and download
  const dataStr = JSON.stringify(report, null, 2);
  const dataBlob = new Blob([dataStr], { type: 'application/json' });
  const url = URL.createObjectURL(dataBlob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `audit-trail-${userId}-${Date.now()}.json`;
  link.click();
}
```

## Compliance Rules Enforcement

### Daily Transaction Limits

```tsx
// Check if user can perform transaction based on daily limits
async function checkDailyLimits(userId: string, amountUSDC: number) {
  const auditEntries = complianceCheckService.getAuditEntries(userId);

  // Filter transactions from last 24 hours
  const last24h = Date.now() - 24 * 60 * 60 * 1000;
  const todaysTransactions = auditEntries.filter(
    entry => entry.timestamp > last24h && entry.overallStatus === 'compliant'
  );

  // Calculate today's volume
  const todaysVolume = todaysTransactions.reduce((sum, entry) => {
    // Extract amount from compliance checks
    return sum + 0; // Would extract from entry.checks
  }, 0);

  // Get user's verification level
  const kycStatus = await kycService.getKYCStatus(userId);
  const dailyLimit = getDailyLimit(kycStatus?.verificationLevel || 'UNVERIFIED');

  if (todaysVolume + amountUSDC > dailyLimit) {
    return {
      canProceed: false,
      message: `Daily limit of $${dailyLimit} exceeded. Used: $${todaysVolume}`,
    };
  }

  return { canProceed: true };
}

function getDailyLimit(verificationLevel: string): number {
  const limits: Record<string, number> = {
    UNVERIFIED: 1000,
    LEVEL_1: 50000,
    LEVEL_2: 500000,
    LEVEL_3: 5000000,
  };
  return limits[verificationLevel] || 0;
}
```

## Error Handling

### Comprehensive Error Management

```tsx
async function performSwapWithComplianceHandling(swapDetails) {
  try {
    // Validate compliance
    const validation = await complianceCheckService.validateTransaction({
      userId: swapDetails.userId,
      buyerWallet: swapDetails.buyerWallet,
      sellerWallet: swapDetails.sellerWallet,
      amountUSDC: swapDetails.amount,
      transactionType: 'swap',
    });

    if (!validation.approved) {
      // Handle block reasons
      switch (validation.blockReason) {
        case 'Critical risk score - transaction blocked':
          showError('Your transaction has been flagged as high-risk and cannot proceed.');
          break;
        case 'KYC verification required':
          showError('Please complete KYC verification to proceed with this transaction.');
          break;
        case 'Velocity limit exceeded':
          showError('You have exceeded your daily transaction limits. Please try again tomorrow.');
          break;
        default:
          showError(`Transaction blocked: ${validation.blockReason}`);
      }
      return;
    }

    // Handle manual review requirement
    if (validation.requiresReview) {
      const userConfirmed = await askUserConfirmation(
        `This transaction (${validation.riskLevel} risk) requires manual review. Continue?`
      );
      if (!userConfirmed) return;
    }

    // Proceed with swap
    const swapId = await submitSwap(swapDetails);
    showSuccess(`Swap ${swapId} initiated successfully!`);

    // Update transaction status after completion
    riskScoringService.updateTransactionStatus(swapId, 'completed');
  } catch (error) {
    handleComplianceError(error);
  }
}

function handleComplianceError(error: Error) {
  if (error.message.includes('KYC')) {
    showError('KYC verification error. Please try again.');
  } else if (error.message.includes('Risk')) {
    showError('Risk assessment failed. Please try again.');
  } else if (error.message.includes('Network')) {
    showError('Network error during compliance check. Please try again.');
  } else {
    showError(`Compliance check failed: ${error.message}`);
  }
}
```

## Testing Compliance

### Test Scenarios

```typescript
// Test Case 1: New wallet with high-value transaction
await testComplianceCheck({
  userId: 'test-user-1',
  walletAddress: 'new-wallet-address',
  amount: 50000,
  expectedRiskLevel: 'high', // High due to wallet age
  expectedAction: 'manual_review',
});

// Test Case 2: Structured deposits
await testComplianceCheck({
  userId: 'test-user-2',
  walletAddress: 'structured-wallet',
  transactionPattern: [10000, 10000, 10000],
  expectedRiskFactor: 'unusual_pattern',
});

// Test Case 3: Velocity violation
await testComplianceCheck({
  userId: 'test-user-3',
  transactions: Array(15).fill(10000),
  expectedRiskLevel: 'high',
  expectedAction: 'block',
});

// Test Case 4: Critical risk blocking
await testComplianceCheck({
  userId: 'test-user-4',
  amount: 1000000,
  walletAge: 0, // Brand new
  expectedRiskLevel: 'critical',
  expectedAction: 'auto_block',
});
```

## GDPR Compliance

### User Data Management

```tsx
import { kycService } from './services/kycService';

// Right to Access - User can download their data
async function downloadUserData(userId: string) {
  const kycStatus = await kycService.getKYCStatus(userId);
  const auditLogs = kycService.getAuditLogs(userId);

  return {
    kycStatus,
    auditLogs,
    exportDate: new Date().toISOString(),
  };
}

// Right to Deletion (Right to be Forgotten)
async function deleteUserData(userId: string) {
  await kycService.clearUserData(userId);
  // Also clear from compliance service
  // complianceCheckService.clearUserData(userId);
}

// Data Processing Agreement
const dpaNotice = `
  Your personal data will be processed for compliance purposes.
  Data is encrypted and shared only with authorized compliance providers.
  You have the right to access, correct, or delete your data.
  For inquiries, contact: compliance@marketplace.com
`;
```

This integration guide should help you implement KYC/AML compliance throughout your application!
