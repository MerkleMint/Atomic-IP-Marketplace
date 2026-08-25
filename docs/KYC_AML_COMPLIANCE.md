# KYC/AML Compliance Framework Integration

This document describes the KYC (Know Your Customer) and AML (Anti-Money Laundering) compliance framework integrated into the Atomic IP Marketplace.

## Overview

The KYC/AML framework implements regulatory compliance controls for high-value transactions and suspicious activity detection. The system includes:

- **KYC Verification**: Multi-level user verification with integration points for third-party providers
- **Risk Scoring**: Machine learning-like transaction risk assessment
- **Transaction Monitoring**: Automated compliance checks and alerts
- **Compliance Audit Trail**: Complete audit logging for regulatory reviews
- **GDPR Compliance**: Secure credential handling and user data protection

## Architecture

### Components

#### 1. KYCService (`frontend/src/services/kycService.ts`)

Manages user verification status and KYC provider integration.

**Key Features:**
- Multi-level verification (LEVEL_1, LEVEL_2, LEVEL_3)
- Provider integration (Chainalysis, Elliptic, internal)
- Encrypted credential storage
- Verification status tracking with expiration
- GDPR-compliant data handling

**Verification Levels:**
- **Level 1**: Basic verification (email) - $1,000 to $10,000 transaction limit
- **Level 2**: Enhanced verification (identity document) - $10,000 to $100,000 limit
- **Level 3**: Full verification (enhanced due diligence) - $100,000+ limit

#### 2. RiskScoringService (`frontend/src/services/complianceService.ts`)

Implements transaction risk scoring with machine learning-like heuristics.

**Risk Factors (0-100 score):**
- **High Velocity** (30 points): Rapid succession of transactions
- **Unusual Amount** (25 points): Significant deviation from user's historical average
- **Unusual Pattern** (20 points): Structured deposits or suspicious patterns
- **New Wallet** (15 points): Wallet age less than 24 hours
- **High Frequency** (10 points): Unusually high transaction frequency

**Risk Levels:**
- Low: 0-39
- Medium: 40-59
- High: 60-79
- Critical: 80-100

#### 3. ComplianceCheckService (`frontend/src/services/complianceCheckService.ts`)

Orchestrates comprehensive compliance checks for transactions.

**Check Types:**
1. **KYC Verification**: Validates user's verification status against transaction amount
2. **Risk Assessment**: Evaluates transaction risk score
3. **Sanctions Screening**: Checks against OFAC and other sanction lists
4. **Velocity Check**: Detects unusual transaction patterns (daily limits)
5. **Amount Verification**: Validates high-value and critical transactions

**Compliance Rules:**
```javascript
{
  HIGH_VALUE_THRESHOLD_USD: 10000,
  CRITICAL_VALUE_THRESHOLD_USD: 50000,
  VELOCITY_CHECK_ENABLED: true,
  SANCTIONS_CHECK_ENABLED: true,
  AUTO_BLOCK_CRITICAL_RISK: true,
  MANUAL_REVIEW_HIGH_RISK: true,
  MAX_DAILY_TRANSACTIONS: 10,
  MAX_DAILY_VOLUME_USD: 100000,
}
```

### UI Components

#### 1. KYCStatusPanel

Displays user's verification status with upgrade options.

```tsx
<KYCStatusPanel 
  userId={userId}
  walletAddress={walletAddress}
  onVerificationNeeded={(level) => console.log(level)}
/>
```

Features:
- Current verification level and status
- Risk score visualization
- Transaction limit display
- Days until expiration
- Verification upgrade options
- GDPR information notice

#### 2. TransactionMonitoringPanel

Real-time transaction monitoring and compliance audit trail.

```tsx
<TransactionMonitoringPanel
  userId={userId}
  autoRefresh={true}
  refreshInterval={30000}
/>
```

Features:
- Compliance statistics dashboard
- Transaction filtering (compliant/blocked/review)
- Individual compliance check details
- Audit trail display
- Risk factor breakdown

## Integration Points

### Swap Initiation Flow

Before initiating a swap, compliance checks must be executed:

```typescript
import { complianceCheckService } from './services/complianceCheckService';

async function initiateSwap(context: SwapComplianceContext) {
  // Run compliance checks
  const validation = await complianceCheckService.validateTransaction(context);

  if (!validation.approved) {
    // Handle blocked transaction
    showError(validation.blockReason);
    return;
  }

  if (validation.requiresReview) {
    // Notify user of manual review requirement
    showWarning('Your transaction requires manual review');
  }

  // Proceed with swap
  performSwap();
}
```

### KYC Provider Integration

To integrate with Chainalysis or similar providers:

1. Set environment variables:
```bash
REACT_APP_CHAINALYSIS_API_KEY=your_api_key
REACT_APP_CHAINALYSIS_ENDPOINT=https://api.chainalysis.com/v1
REACT_APP_KYC_ENCRYPTION_KEY=your_encryption_key
```

2. Implement provider-specific API calls in `kycService.ts`
3. Handle verification responses and update user status

## Compliance Requirements

### Regulatory Standards

The framework implements controls for:
- **FinCEN** (US Financial Crimes Enforcement Network)
- **FATF** (Financial Action Task Force) Recommendations
- **GDPR** (EU General Data Protection Regulation)
- **SOX** (Sarbanes-Oxley Act) - Audit logging
- **PCI DSS** - Secure credential storage

### Risk Scoring Methodology

Risk scores are calculated using weighted factors:

```
Risk Score = (V × 0.30) + (A × 0.25) + (P × 0.20) + (W × 0.15) + (F × 0.10)

Where:
- V = Velocity risk (0-30)
- A = Amount risk (0-25)
- P = Pattern risk (0-20)
- W = Wallet age risk (0-15)
- F = Frequency risk (0-10)
```

### Transaction Monitoring

Daily volume limits per verification level:
- **Level 1**: $50,000 max daily volume, 5 transactions max
- **Level 2**: $500,000 max daily volume, 20 transactions max
- **Level 3**: $5,000,000 max daily volume, 100 transactions max

### Audit Logging

All KYC-related operations are logged:
- Verification initiated/completed
- Status updates
- Data access (for GDPR tracking)
- Risk assessments
- Compliance checks
- Manual reviews

Logs are stored encrypted in localStorage and can be exported for compliance reviews.

## Security Considerations

### Credential Handling

1. **Encryption**: API keys and sensitive credentials are encrypted using AES encryption
2. **Environment Variables**: Credentials are stored only in environment variables, never in code
3. **Token Management**: Implement secure token rotation and expiration
4. **Access Control**: Restrict credential access to authorized services only

### Data Protection

1. **GDPR Compliance**: 
   - User consent required before data collection
   - Data minimization principle applied
   - Right to deletion (clearUserData method)
   - Data processing agreements with providers

2. **Encryption in Transit**: All API communications use HTTPS
3. **Encryption at Rest**: Sensitive data encrypted in localStorage
4. **Data Retention**: Implement automatic purge of old audit logs

## Testing

### Unit Tests

Test coverage should include:

```typescript
// KYC Service Tests
- testInitiateVerification()
- testGetKYCStatus()
- testCheckVerificationForAmount()
- testEncryption()
- testDataClear()

// Risk Scoring Tests
- testAssessTransactionRisk()
- testCheckVelocity()
- testCheckUnusualAmount()
- testCheckWalletAge()

// Compliance Check Tests
- testExecuteSwapComplianceChecks()
- testValidateTransaction()
- testSanctionsScreening()
- testAuditLogging()
```

### Compliance Rule Edge Cases

Test scenarios:
1. New wallet with high-value transaction
2. Multiple rapid transactions (velocity test)
3. Structured deposits to avoid thresholds
4. Expired verification
5. Level upgrade requirements
6. Critical risk blocking
7. Manual review flags
8. Sanctions match scenarios

## Configuration

### Environment Variables

```bash
# KYC Encryption
REACT_APP_KYC_ENCRYPTION_KEY=your-secure-encryption-key

# Chainalysis Integration
REACT_APP_CHAINALYSIS_API_KEY=your-api-key
REACT_APP_CHAINALYSIS_ENDPOINT=https://api.chainalysis.com/v1

# Elliptic Integration (optional)
REACT_APP_ELLIPTIC_API_KEY=your-api-key
REACT_APP_ELLIPTIC_ENDPOINT=https://api.elliptic.com

# Compliance Rules
REACT_APP_HIGH_VALUE_THRESHOLD=10000
REACT_APP_CRITICAL_VALUE_THRESHOLD=50000
REACT_APP_MAX_DAILY_VOLUME=100000
REACT_APP_MAX_DAILY_TRANSACTIONS=10
```

### Provider Configuration

Each provider configuration includes:
- API endpoint
- API key (encrypted)
- Timeout settings
- Enable/disable flag
- Rate limiting

## Audit Trail Example

```json
{
  "id": "audit-1687432800000-abc123",
  "timestamp": 1687432800000,
  "userId": "user-123",
  "transactionId": "txn-1687432800000-def456",
  "walletAddress": "0x742d35Cc6634C0532925a3b844Bc9e7595f42471",
  "checks": [
    {
      "checkType": "kyc_verification",
      "passed": true,
      "riskScore": 15,
      "riskLevel": "low",
      "message": "KYC verification passed"
    },
    {
      "checkType": "risk_assessment",
      "passed": true,
      "riskScore": 25,
      "riskLevel": "low"
    }
  ],
  "overallStatus": "compliant"
}
```

## Maintenance and Updates

### Regular Tasks

1. **Daily**: Monitor high-risk flagged transactions
2. **Weekly**: Review manual review queue
3. **Monthly**: Analyze compliance statistics and trends
4. **Quarterly**: Update risk scoring parameters based on patterns
5. **Annually**: Audit and update compliance documentation

### Provider Updates

- Monitor Chainalysis/Elliptic API updates
- Implement API version upgrades
- Update sanctions list feeds
- Review and update risk thresholds

## Support and Escalation

### Risk Score Ranges

- **Low Risk (0-39)**: Automatic approval
- **Medium Risk (40-59)**: Automatic approval with monitoring
- **High Risk (60-79)**: Manual review required
- **Critical Risk (80-100)**: Automatic block, escalate to compliance team

### Manual Review Process

1. Flag transaction for review
2. Notify compliance team
3. Collect additional documentation if needed
4. Approval/rejection decision
5. Audit log update

## References

- [FinCEN Guidelines](https://www.fincen.gov/)
- [FATF Recommendations](https://www.fatf-gafi.org/)
- [GDPR Compliance](https://gdpr-info.eu/)
- [Chainalysis API Docs](https://docs.chainalysis.com/)
- [OFAC Sanctions List](https://home.treasury.gov/policy-issues/office-of-foreign-assets-control-sanctions-programs-and-information)

## Version History

- **v1.0.0** (2026-06-22): Initial KYC/AML compliance framework integration
  - KYC verification with multi-level support
  - Transaction risk scoring
  - Compliance checks
  - UI components for status and monitoring
  - Full audit trail logging
