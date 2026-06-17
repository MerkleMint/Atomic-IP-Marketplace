# Security Policy

## Responsible Disclosure

We take security seriously and appreciate your efforts to protect our users and the integrity of Atomic IP Marketplace contracts.

If you discover a vulnerability, we would like to know about it as soon as possible. Please report it privately to the project maintainers via email at `farouq@atomic-ip-marketplace.com`.
If the upstream repository has GitHub Security Advisories enabled, you may also use that private reporting channel instead of email.

### Disclosure Process
1. **Report the issue** privately with a detailed description, including reproduction steps, impact, and any proof-of-concept.
2. We will acknowledge receipt within **48 hours**.
3. We will assess and work on a fix with **7 days** target resolution for critical issues.
4. Do not publicly disclose the vulnerability without our explicit permission.
5. We may credit responsible reporters in release notes.

Preferred reporting format:
- Use [GPG encryption](https://emailselfdefense.fsf.org/) if possible.
- Include contract affected (e.g., atomic_swap), network (testnet/mainnet), and version.

## Scope
This policy applies to:
- Soroban contracts: `atomic_swap`, `ip_registry`, `zk_verifier`.
- Deployment scripts (`deploy_testnet.sh`).
- Related infrastructure under our control.

## Known Limitations
- **Smart Contract Risks**: While Soroban provides protections (e.g., no reentrancy), risks like economic exploits, oracle manipulation (if used), or pause mechanism abuse possible. No formal security audit conducted yet.
- **Testnet Focus**: Current deploys are testnet-only; mainnet untested.
- **Data TTL**: IP listings and ZK proofs expire (e.g., via TTL); permanent storage not guaranteed.
- **USDC Handling**: Atomic swaps handle real USDC; users bear custody risks.
- **Dependencies**: Relies on Soroban SDK v22.0.0; upstream vulns possible.

## Validation Checks (atomic_swap)

The following checks are enforced in the `atomic_swap` contract:

| Check | Error Code | Where |
|---|---|---|
| Token allowance must be ≥ `usdc_amount` before transfer | `InsufficientAllowance` (#24) | `initiate_swap` |
| Only tokens added via `add_allowed_token` are accepted | `InvalidToken` (#20) | `initiate_swap` |
| Buyer's payment must be ≥ `listing.price_usdc` | `UnderpaymentNotAllowed` (#14) | `initiate_swap` |
| Fee basis points must not exceed 10 000 (100%) | `FeeBpsTooHigh` (#21) | `initialize`, `update_config` |
| ZK Merkle proof must pass verifier | `InvalidProof` (#16) | `confirm_swap` |
| Cancel only after `cancel_delay_secs` elapses | `CancelTooEarly` (#18) | `cancel_swap` |
| Release only after dispute window expires | `DisputeWindowActive` (#19) | `release_to_seller` |
| Seller address must match listing owner | `SellerMismatch` (#9) | `initiate_swap` |
| Arithmetic overflow is detected on fee calculation | `Overflow` (#23) | `initiate_swap` |
| Fee truncation to zero is rejected | `FeeWouldTruncate` (#15) | `initiate_swap` |

### Error Recovery

Errors are classified into recovery categories via `SwapRecoveryKind`:
- **Validation** — pre-transfer; no funds moved; safe to retry after fixing inputs.
- **ProofVerification** — ZK proof rejected; swap stays Pending; seller must resubmit.
- **StateRollback** — post-transfer inconsistency; rollback attempted via `attempt_rollback_swap`.
- **FundsLocked** — funds in escrow; dispute window active; retry after window expires.
- **CancelDelay** — cancel attempted too early; retry after `created_at + cancel_delay_secs`.
- **TokenTransfer** — transfer step failed; contact admin if funds appear stuck.
- **Unauthorized** — wrong signer; ensure correct account signs the transaction.
- **Expired** — dispute window expired; raise_dispute must be called within the window.

Failed operations emit diagnostic events (`SwapInitFailed`, `SwapConfirmFailed`, `SwapCancelFailed`, `SwapReleaseFailed`) visible in the Soroban diagnostic event log even when a transaction rolls back. Off-chain indexers should monitor these events for alerting and recovery workflows.

## Out-of-Scope Items
- Attacks requiring control of user wallets or private keys.
- Theoretical attacks without practical impact.
- Previously known public issues.
- Third-party services (Stellar network, Soroban SDK, wallets).
- Denial-of-service from network congestion.
- Social engineering or phishing.

## Rewards
No formal bug bounty program yet. Responsible disclosures may receive recognition and swag/merch.

---

*Last updated: March 2026*
*Project: Atomic IP Marketplace (Soroban contracts)*
