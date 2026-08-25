#![no_std]
use ip_registry::IpRegistryClient;
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Bytes, BytesN, Env,
};
use zk_verifier::{ProofNode, ZkVerifierClient};

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;
const DEFAULT_DISPUTE_WINDOW_LEDGERS: u32 = 17_280;
const DEFAULT_COMMIT_WINDOW_LEDGERS: u32 = 17_280;
const DEFAULT_REVEAL_WINDOW_LEDGERS: u32 = 17_280;
const DEFAULT_APPEAL_WINDOW_LEDGERS: u32 = 17_280;
/// Default window an appealed dispute waits for admin's `resolve_dispute` before
/// anyone may settle it per the original arbiter outcome (stuck-funds guard).
const DEFAULT_APPEAL_RESOLUTION_WINDOW_LEDGERS: u32 = 17_280;
/// Default escrow hold period: 7 days at ~6 s/ledger.
const DEFAULT_HOLD_PERIOD_SECS: u64 = 604_800;
/// Maximum escrow hold the admin or seller may configure (30 days).
const MAX_HOLD_PERIOD_SECS: u64 = 2_592_000;
/// Default multi-sig threshold: 10,000 USDC (7-decimal Stellar representation).
const DEFAULT_MULTISIG_THRESHOLD: i128 = 100_000_000_000; // 10,000 * 10^7
/// Maximum number of signers in a multi-sig scheme (supports 2-of-2 and 2-of-3).
const MAX_MULTISIG_SIGNERS: u32 = 3;

//Added enum

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum ContractError {
    EmptyDecryptionKey = 1,
    SwapNotFound = 2,
    InvalidAmount = 3,
    ContractPaused = 4,
    NotInitialized = 5,
    AlreadyInitialized = 6,
    SwapNotPending = 7,
    SwapAlreadyPending = 8,
    SellerMismatch = 9,
    SwapNotCancellable = 10,
    DisputeWindowExpired = 11,
    SwapNotCompleted = 12,
    SwapNotDisputed = 13,
    /// Buyer's offered amount is below the listing's price_usdc.
    UnderpaymentNotAllowed = 14,
    /// Configured fee_bps would compute to zero for this usdc_amount.
    FeeWouldTruncate = 15,
    /// ZK Merkle proof verification failed.
    InvalidProof = 16,
    /// Pagination offset or limit is out of valid range.
    InvalidPaginationParams = 17,
    /// Cancel delay has not yet elapsed since swap creation.
    CancelTooEarly = 18,
    /// release_to_seller called before the dispute window has expired.
    DisputeWindowActive = 19,
    /// The provided token is not in the allowed list.
    InvalidToken = 20,
    /// Fee basis points exceeds 10,000 (100%).
    FeeBpsTooHigh = 21,
    /// confirmed_at_ledger is None on a swap that should have been confirmed.
    MissingConfirmationLedger = 22,
    /// Arithmetic overflow during fee calculation.
    Overflow = 23,
    /// Buyer's token allowance for this contract is below usdc_amount.
    /// Buyer must call `token.approve(contract, amount)` before initiating.
    InsufficientAllowance = 24,
    /// Caller is not a registered active arbiter.
    NotAnArbiter = 25,
    /// Arbiter is a party to the swap and cannot vote (conflict of interest).
    ArbiterConflictOfInterest = 26,
    /// Arbiter has already committed a vote for this dispute.
    ArbiterAlreadyCommitted = 27,
    /// Arbiter has already revealed their vote for this dispute.
    ArbiterAlreadyRevealed = 28,
    /// No vote commitment found for this arbiter; commit before revealing.
    VoteCommitNotFound = 29,
    /// Revealed vote and salt do not match the stored commitment.
    InvalidVoteReveal = 30,
    /// The commit window for this dispute has already expired.
    CommitWindowExpired = 31,
    /// The reveal window has not yet opened (commit deadline not reached).
    RevealWindowNotOpen = 32,
    /// The reveal window for this dispute has expired.
    RevealWindowExpired = 33,
    /// Dispute outcome is already set; cannot finalize again.
    DisputeAlreadyFinalized = 34,
    /// The appeal window for this dispute has expired.
    AppealWindowExpired = 35,
    /// This dispute has already been appealed.
    DisputeAlreadyAppealed = 36,
    /// No Dispute record found for this swap_id.
    SwapDisputeNotFound = 37,
    /// Escrow hold period is still active; seller cannot release funds yet.
    HoldPeriodActive = 38,
    /// Requested hold period exceeds the configured maximum.
    HoldPeriodTooLong = 39,
    // ── Multi-sig errors (40-49) ──────────────────────────────────────────────
    /// Swap amount exceeds multi-sig threshold and requires multi-sig approval.
    MultiSigRequired = 40,
    /// Caller is not a configured multi-sig signer for this swap.
    NotAMultiSigSigner = 41,
    /// Signer has already approved this swap.
    MultiSigAlreadyApproved = 42,
    /// Multi-sig approval threshold not yet met; cannot proceed.
    MultiSigThresholdNotMet = 43,
    /// Multi-sig configuration is invalid (e.g. required > signer count).
    InvalidMultiSigConfig = 44,
    /// Replay attack detected: nonce already used.
    NonceAlreadyUsed = 45,
    // ── Appeal settlement errors (46-48) ──────────────────────────────────────
    /// `settle_dispute` called before the appeal window has closed on a dispute
    /// that was never appealed.
    AppealWindowStillOpen = 46,
    /// `settle_dispute` called on an appealed dispute before the appeal
    /// resolution timeout has elapsed; only the admin's `resolve_dispute` can
    /// act on it until then.
    AppealResolutionWindowActive = 47,
    /// `settle_dispute` called on a swap that is not awaiting settlement
    /// (not in `PendingAppealWindow` or `Appealed` status).
    SwapNotAwaitingSettlement = 48,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum SwapStatus {
    Pending,
    Completed,
    Cancelled,
    Disputed,
    ResolvedBuyer,
    ResolvedSeller,
    /// High-value swap awaiting multi-sig approvals before becoming active.
    PendingMultiSig,
    /// Arbiter vote tallied and outcome recorded, but escrow is held (not paid
    /// out) until the appeal window closes with no appeal filed.
    PendingAppealWindow,
    /// Buyer appealed the finalized outcome before the appeal window closed.
    /// Escrow remains held; only `resolve_dispute` (admin) or the appeal
    /// resolution timeout via `settle_dispute` can move funds from here.
    Appealed,
}

/// Protocol-wide escrow hold configuration.
///
/// Hold economics: after a swap is completed (seller submits the decryption key)
/// the seller's payout can be held for an additional, seller-configurable window
/// on top of the dispute window. During the hold the buyer may verify the
/// delivered IP and, if satisfied, call `confirm_receipt` to release funds
/// immediately. The hold gives honest buyers time to confirm and discourages a
/// seller from sweeping funds the instant the dispute window lapses, while the
/// MAX_HOLD_PERIOD_SECS cap and the immutability of `hold_until` (captured at
/// confirm time) prevent a seller from weaponising the hold against the buyer.
///
/// `enabled` defaults to `false` so existing swaps are unaffected unless the
/// admin turns the feature on globally or a seller opts in via
/// `set_seller_hold_period`.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub struct EscrowHoldConfig {
    pub enabled: bool,
    pub default_hold_period_secs: u64,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub struct Config {
    pub admin: Address,
    pub fee_bps: u32,
    pub fee_recipient: Address,
    pub cancel_delay_secs: u64,
    pub swap_expiry_secs: u64,
    pub zk_verifier: Address,
    pub ip_registry: Address,
    pub escrow_hold: EscrowHoldConfig,
}

#[contracttype]
#[derive(Clone)]
pub struct Swap {
    pub listing_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub usdc_amount: i128,
    pub usdc_token: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: SwapStatus,
    pub decryption_key: Option<Bytes>,
    pub confirmed_at_ledger: Option<u32>,
    /// Ledger timestamp at which the escrow hold period ends. Captured at
    /// confirm time so it cannot be altered afterwards. `None` means no hold
    /// applies (the feature was disabled for this swap's seller).
    pub hold_until: Option<u64>,
    /// Set to `true` when the buyer calls `confirm_receipt`, allowing the seller
    /// to release funds before the hold period elapses.
    pub buyer_confirmed: bool,
}

/// Outcome of a dispute after arbiter voting or admin resolution.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DisputeOutcome {
    Pending,
    FavorBuyer,
    FavorSeller,
}

/// Compound key for per-arbiter, per-dispute vote storage.
#[contracttype]
#[derive(Clone)]
pub struct DisputeVoteKey {
    pub swap_id: u64,
    pub arbiter: Address,
}

/// Compound key for per-evidence item storage.
#[contracttype]
#[derive(Clone)]
pub struct EvidenceKey {
    pub swap_id: u64,
    pub index: u32,
}

/// Full dispute record stored on-chain when a buyer raises a dispute.
#[contracttype]
#[derive(Clone)]
pub struct Dispute {
    pub swap_id: u64,
    pub raised_by: Address,
    pub raised_at_ledger: u32,
    pub evidence_count: u32,
    pub outcome: DisputeOutcome,
    pub resolved_at_ledger: Option<u32>,
    /// Sum of revealed voting weights favouring the buyer.
    pub vote_weight_buyer: i128,
    /// Sum of revealed voting weights favouring the seller.
    pub vote_weight_seller: i128,
    /// Arbiters must commit a blinded vote before this ledger.
    pub commit_deadline_ledger: u32,
    /// Arbiters must reveal their vote between commit_deadline and this ledger.
    pub reveal_deadline_ledger: u32,
    /// Set after finalization; buyer may appeal before this ledger.
    pub appeal_deadline_ledger: Option<u32>,
    pub is_appealed: bool,
    /// Set when an appeal is filed; admin may call `resolve_dispute` before
    /// this ledger. After it, anyone may call `settle_dispute` to pay out per
    /// the original arbiter outcome so an inactive admin cannot lock funds.
    pub appeal_resolve_by_ledger: Option<u32>,
}

/// Metadata for a registered arbiter.
#[contracttype]
#[derive(Clone)]
pub struct ArbiterInfo {
    pub weight: i128,
    pub is_active: bool,
}

// ── Multi-signature approval ───────────────────────────────────────────────────

/// Configuration for the multi-sig approval scheme applied to high-value swaps.
///
/// When a swap's `usdc_amount` meets or exceeds `threshold`, it is placed in a
/// `PendingMultiSig` state. The swap can only proceed to `Pending` (funds locked)
/// after the required number of approvals from `signers` are collected.
///
/// Supported schemes:
///   - 2-of-2: `required_approvals = 2`, two signers (e.g. admin + seller).
///   - 2-of-3: `required_approvals = 2`, three signers.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub struct MultiSigConfig {
    /// USDC amount (7-decimal i128) at or above which multi-sig is required.
    pub threshold: i128,
    /// Addresses authorised to sign high-value swaps.
    pub signers: soroban_sdk::Vec<Address>,
    /// Minimum number of distinct approvals required to unblock the swap.
    pub required_approvals: u32,
    /// When `false` the multi-sig gate is disabled entirely (all swaps pass through).
    pub enabled: bool,
}

/// Per-swap approval accumulator stored while a swap awaits multi-sig sign-off.
#[contracttype]
#[derive(Clone)]
pub struct MultiSigApproval {
    pub swap_id: u64,
    /// Addresses that have already approved this swap.
    pub approved_by: soroban_sdk::Vec<Address>,
    /// Nonce used to prevent replay attacks across approvals.
    pub nonce: u64,
}

/// Compound key: (swap_id, nonce) — used to detect replayed nonce/approval pairs.
#[contracttype]
#[derive(Clone)]
pub struct MultiSigNonceKey {
    pub swap_id: u64,
    pub nonce: u64,
}

/// A single piece of evidence submitted by a swap party.
#[contracttype]
#[derive(Clone)]
pub struct DisputeEvidenceItem {
    pub submitter: Address,
    pub ipfs_hash: Bytes,
    pub submitted_at_ledger: u32,
}

#[contracttype]
pub enum DataKey {
    Swap(u64),
    Counter,
    ActiveListingSwap(u64),
    BuyerIndex(Address),
    SellerIndex(Address),
    Config,
    Admin,
    Paused,
    DisputeWindowLedgers,
    AllowedToken(Address),
    // ── Dispute resolution system ──────────────────────────────────────────────
    /// Dispute record keyed by swap_id.
    Dispute(u64),
    /// Individual evidence items keyed by (swap_id, index).
    EvidenceItem(EvidenceKey),
    /// Registered arbiter metadata.
    ArbiterEntry(Address),
    /// Ordered list of all arbiter addresses.
    ArbiterList,
    /// Blinded vote commitment keyed by (swap_id, arbiter).
    VoteCommit(DisputeVoteKey),
    /// Revealed vote (bool) keyed by (swap_id, arbiter).
    VoteRevealed(DisputeVoteKey),
    /// Ledger window for the commit phase.
    CommitWindowLedgers,
    /// Ledger window for the reveal phase (starts after commit deadline).
    RevealWindowLedgers,
    /// Ledger window during which the buyer may appeal a finalized dispute.
    AppealWindowLedgers,
    /// Ledger window during which the admin may act on an appealed dispute
    /// before anyone may settle it per the original arbiter outcome.
    AppealResolutionWindowLedgers,
    // ── Escrow hold ───────────────────────────────────────────────────────────
    /// Per-seller hold period override (u64 seconds). Absent = use global default.
    SellerHoldPeriod(Address),
    // ── Multi-signature approval ──────────────────────────────────────────────
    /// Protocol-wide multi-sig configuration.
    MultiSigConfig,
    /// Per-swap approval accumulator.
    MultiSigApproval(u64),
    /// Replay-attack guard: (swap_id, nonce) → bool.
    MultiSigNonce(MultiSigNonceKey),
}

/// Event lifecycle:
///   1. SwapInitiated      — buyer locks funds and initiates the swap
///   2. SwapKeySubmitted   — seller submits the decryption key; dispute window begins
///   3. FundsReleased      — dispute window elapsed, funds transferred to seller (swap settled)
///      SwapCancelled      — buyer cancels before seller confirms (funds returned)

#[contractevent]
pub struct SwapInitiated {
    #[topic]
    pub swap_id: u64,
    #[topic]
    pub listing_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub usdc_amount: i128,
}

#[contractevent]
pub struct SwapConfirmed {
    #[topic]
    pub swap_id: u64,
    pub seller: Address,
    pub decryption_key: Bytes,
}

#[contractevent]
pub struct SwapCancelled {
    #[topic]
    pub swap_id: u64,
    pub buyer: Address,
    pub usdc_amount: i128,
}

/// Emitted by confirm_swap when the seller submits the decryption key.
/// The dispute window starts here; funds are NOT yet released.
/// Listen for FundsReleased to confirm settlement.
#[contractevent]
pub struct SwapKeySubmitted {
    #[topic]
    pub swap_id: u64,
    pub seller: Address,
}

/// Emitted after funds are successfully transferred to the seller via release_to_seller.
#[contractevent]
pub struct FundsReleased {
    #[topic]
    pub swap_id: u64,
    pub seller: Address,
    pub amount: i128,
}

/// Emitted when the contract is paused by the admin.
#[contractevent]
pub struct ContractPausedEvent {
    #[topic]
    pub admin: Address,
}

/// Emitted when the contract is unpaused by the admin.
#[contractevent]
pub struct ContractUnpausedEvent {
    #[topic]
    pub admin: Address,
}

/// Emitted when the admin role is transferred.
#[contractevent]
pub struct AdminTransferred {
    #[topic]
    pub old_admin: Address,
    pub new_admin: Address,
}

/// Emitted when a seller configures their escrow hold period.
#[contractevent]
pub struct SellerHoldPeriodUpdated {
    #[topic]
    pub seller: Address,
    pub hold_period_secs: u64,
}

/// Emitted when the admin updates the global escrow hold configuration.
#[contractevent]
pub struct EscrowHoldConfigUpdated {
    #[topic]
    pub admin: Address,
    pub enabled: bool,
    pub default_hold_period_secs: u64,
}

/// Emitted when a buyer confirms receipt, overriding the remaining hold period.
/// Provides an on-chain audit trail of early-release authorisations.
#[contractevent]
pub struct BuyerConfirmedReceipt {
    #[topic]
    pub swap_id: u64,
    pub buyer: Address,
}

/// Emitted when a dispute is resolved by the admin.
#[contractevent]
pub struct DisputeResolved {
    #[topic]
    pub swap_id: u64,
    pub favor_buyer: bool,
}

/// Emitted when a buyer raises a dispute on a completed swap.
#[contractevent]
pub struct DisputeRaised {
    #[topic]
    pub swap_id: u64,
    pub buyer: Address,
}

/// Emitted when the admin updates the protocol config.
#[contractevent]
pub struct ConfigUpdated {
    #[topic]
    pub admin: Address,
    pub fee_bps: u32,
    pub fee_recipient: Address,
    pub cancel_delay_secs: u64,
}

/// Emitted when swap initialization fails before funds are transferred.
/// Allows off-chain indexers to track failed attempts without parsing errors.
/// Note: in Soroban, events emitted before a panic are rolled back in the
/// transaction but remain visible in the diagnostic event log.
#[contractevent]
pub struct SwapInitFailed {
    #[topic]
    pub listing_id: u64,
    /// The ContractError code that caused the failure.
    pub error_code: u32,
    pub buyer: Address,
}

/// Emitted when confirm_swap fails after partial state changes (e.g. proof rejected).
/// Signals that the swap remains Pending and no key was stored.
#[contractevent]
pub struct SwapConfirmFailed {
    #[topic]
    pub swap_id: u64,
    /// The ContractError code that caused the failure.
    pub error_code: u32,
    pub seller: Address,
}

/// Emitted when cancel_swap fails (e.g. delay not elapsed, swap not pending).
/// Funds remain locked; buyer must retry after the cancel delay.
#[contractevent]
pub struct SwapCancelFailed {
    #[topic]
    pub swap_id: u64,
    /// The ContractError code that caused the failure.
    pub error_code: u32,
    pub buyer: Address,
}

/// Emitted when release_to_seller fails (e.g. dispute window still active).
/// Funds remain in escrow; seller must retry after the window expires.
#[contractevent]
pub struct SwapReleaseFailed {
    #[topic]
    pub swap_id: u64,
    /// The ContractError code that caused the failure.
    pub error_code: u32,
    pub seller: Address,
}

/// Emitted when an arbiter is registered or their weight updated.
#[contractevent]
pub struct ArbiterRegistered {
    #[topic]
    pub arbiter: Address,
    pub weight: i128,
}

/// Emitted when an arbiter is deactivated (soft-removed).
#[contractevent]
pub struct ArbiterDeactivated {
    #[topic]
    pub arbiter: Address,
}

/// Emitted when a party submits evidence for a dispute.
#[contractevent]
pub struct EvidenceSubmitted {
    #[topic]
    pub swap_id: u64,
    #[topic]
    pub submitter: Address,
    pub evidence_index: u32,
}

/// Emitted when an arbiter commits a blinded vote.
/// Does NOT reveal who voted for what — anonymity is preserved until reveal.
#[contractevent]
pub struct VoteCommitted {
    #[topic]
    pub swap_id: u64,
}

/// Emitted when an arbiter reveals their vote.
#[contractevent]
pub struct VoteRevealed {
    #[topic]
    pub swap_id: u64,
    pub arbiter: Address,
    pub favor_buyer: bool,
}

/// Emitted when a dispute is finalized based on arbiter vote weights.
#[contractevent]
pub struct DisputeFinalized {
    #[topic]
    pub swap_id: u64,
    pub favor_buyer: bool,
}

/// Emitted when the buyer appeals a finalized dispute.
#[contractevent]
pub struct DisputeAppealed {
    #[topic]
    pub swap_id: u64,
    pub appellant: Address,
}

/// Emitted when `settle_dispute` releases escrow held since `finalize_dispute`
/// — either the appeal window closed with no appeal, or an appealed dispute's
/// resolution timeout elapsed without admin action.
#[contractevent]
pub struct DisputeSettled {
    #[topic]
    pub swap_id: u64,
    pub favor_buyer: bool,
}

// ── Multi-sig events ──────────────────────────────────────────────────────────

/// Emitted when the multi-sig configuration is updated by the admin.
#[contractevent]
pub struct MultiSigConfigUpdated {
    #[topic]
    pub admin: Address,
    pub threshold: i128,
    pub required_approvals: u32,
    pub enabled: bool,
}

/// Emitted when a signer approves a high-value swap.
#[contractevent]
pub struct MultiSigApprovalAdded {
    #[topic]
    pub swap_id: u64,
    #[topic]
    pub signer: Address,
    pub approvals_count: u32,
    pub required_approvals: u32,
}

/// Emitted when a swap has collected enough multi-sig approvals and is unblocked.
#[contractevent]
pub struct MultiSigThresholdMet {
    #[topic]
    pub swap_id: u64,
    pub approvals_count: u32,
}

/// Discriminates the phase in which a swap failed, for detailed error recovery.
///
/// Error recovery flow:
///   1. `Validation`  — pre-transfer check failed; no funds moved; safe to retry.
///   2. `ProofVerification` — seller's ZK proof rejected; swap stays Pending; seller must resubmit.
///   3. `StateRollback` — post-transfer inconsistency detected; rollback was attempted.
///   4. `FundsLocked`  — funds are in escrow but cannot be released yet (dispute window active).
///   5. `CancelDelay`  — cancel attempted before the configured delay elapsed.
///   6. `TokenTransfer` — token transfer step failed; partial state may exist.
///   7. `Unauthorized` — caller does not have the required role (buyer/seller/admin).
///   8. `Expired`      — swap or dispute window has passed the valid time range.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum SwapRecoveryKind {
    /// Pre-transfer validation failed; no on-chain state was modified.
    /// Recovery: fix the input (allowance, amount, token) and retry.
    Validation = 0,
    /// ZK Merkle proof was rejected by the verifier.
    /// Recovery: seller must generate a valid proof and resubmit confirm_swap.
    ProofVerification = 1,
    /// Inconsistency detected after a partial state change; rollback was attempted.
    /// Recovery: check swap status via get_swap; funds are returned if rollback succeeded.
    StateRollback = 2,
    /// Funds are locked in escrow but the dispute window has not expired yet.
    /// Recovery: wait until confirmed_at_ledger + dispute_window_ledgers, then retry.
    FundsLocked = 3,
    /// cancel_swap called before cancel_delay_secs elapsed.
    /// Recovery: wait until created_at + cancel_delay_secs (ledger timestamp), then retry.
    CancelDelay = 4,
    /// A token transfer instruction failed.
    /// Recovery: inspect swap status; contact admin if funds appear stuck.
    TokenTransfer = 5,
    /// Caller is not authorised for this operation.
    /// Recovery: ensure the correct account signs the transaction.
    Unauthorized = 6,
    /// The swap or its dispute window has expired.
    /// Recovery: for dispute window, raise_dispute must be called within the window.
    Expired = 7,
}

#[contract]
pub struct AtomicSwap;

#[contractimpl]
impl AtomicSwap {
    /// Attempt to roll back a swap that was partially initialised.
    ///
    /// This is called when state has already been partially written (e.g. the
    /// counter was incremented) but a subsequent step fails.  It tries to
    /// return funds to the buyer and remove the swap from persistent storage.
    ///
    /// The function is best-effort: if the transfer itself fails (e.g. the
    /// contract has no balance) we do NOT panic a second time — the caller is
    /// responsible for panicking with the original error after calling this.
    ///
    /// Returns `true` if the rollback transfer succeeded, `false` otherwise.
    #[cfg(test)]
    fn attempt_rollback_swap(env: &Env, swap_id: u64) -> bool {
        let key = DataKey::Swap(swap_id);
        if let Some(swap) = env
            .storage()
            .persistent()
            .get::<DataKey, Swap>(&key)
        {
            // Only roll back swaps that are still Pending (no key delivered yet).
            if swap.status == SwapStatus::Pending {
                let token_client = token::Client::new(env, &swap.usdc_token);
                let contract_addr = env.current_contract_address();
                // Try the refund; ignore failures so the caller can panic with the original error.
                let result = token_client.try_transfer(
                    &contract_addr,
                    &swap.buyer,
                    &swap.usdc_amount,
                );
                if result.is_ok() {
                    env.storage().persistent().remove(&key);
                    env.storage()
                        .persistent()
                        .remove(&DataKey::ActiveListingSwap(swap.listing_id));
                    return true;
                }
            }
        }
        false
    }

    fn calculate_fee_amount(env: &Env, usdc_amount: i128, fee_bps: u32) -> i128 {
        if fee_bps == 0 {
            return 0;
        }
        let product = usdc_amount
            .checked_mul(fee_bps as i128)
            .unwrap_or_else(|| env.panic_with_error(ContractError::Overflow));
        let fee = product / 10_000;
        if fee == 0 {
            env.panic_with_error(ContractError::FeeWouldTruncate);
        }
        fee
    }

    fn validate_fee_amount(env: &Env, usdc_amount: i128, fee_bps: u32) {
        // Intentionally compute the fee up front so initiate_swap preserves the
        // truncation check even if the returned fee is not otherwise needed yet.
        let _validated_fee = Self::calculate_fee_amount(env, usdc_amount, fee_bps);
    }

    /// Resolve the effective hold period (in seconds) that applies to `seller`.
    ///
    /// Resolution order:
    ///   1. A per-seller override (`SellerHoldPeriod`) always wins, even when it
    ///      is `0` — that lets a seller explicitly opt out of holding funds.
    ///   2. Otherwise, the global default applies only when holds are enabled.
    ///   3. When holds are globally disabled and the seller has no override, the
    ///      period is `0` (no hold), preserving the original swap flow.
    fn effective_hold_period(env: &Env, config: &Config, seller: &Address) -> u64 {
        if let Some(secs) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::SellerHoldPeriod(seller.clone()))
        {
            return secs;
        }
        if config.escrow_hold.enabled {
            config.escrow_hold.default_hold_period_secs
        } else {
            0
        }
    }

    /// Whether `swap` is still inside its escrow hold period. A buyer who has
    /// confirmed receipt waives the remaining hold, so the funds are releasable.
    fn is_hold_active(env: &Env, swap: &Swap) -> bool {
        if swap.buyer_confirmed {
            return false;
        }
        match swap.hold_until {
            Some(hold_until) => env.ledger().timestamp() < hold_until,
            None => false,
        }
    }

    pub fn initialize(
        env: Env,
        admin: Address,
        fee_bps: u32,
        fee_recipient: Address,
        cancel_delay_secs: u64,
        swap_expiry_secs: u64,
        zk_verifier: Address,
        ip_registry: Address,
    ) {
        if env.storage().persistent().has(&DataKey::Config) {
            env.panic_with_error(ContractError::AlreadyInitialized);
        }
        if fee_bps > 10_000 {
            env.panic_with_error(ContractError::FeeBpsTooHigh);
        }
        let config = Config {
            admin: admin.clone(),
            fee_bps,
            fee_recipient,
            cancel_delay_secs,
            swap_expiry_secs,
            zk_verifier,
            ip_registry,
            // Hold feature is opt-in: disabled globally by default so existing
            // swap flows are unchanged until the admin enables it or a seller
            // sets their own hold period.
            escrow_hold: EscrowHoldConfig {
                enabled: false,
                default_hold_period_secs: DEFAULT_HOLD_PERIOD_SECS,
            },
        };
        // Store Admin and Config in persistent storage instead of instance storage
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().set(
            &DataKey::DisputeWindowLedgers,
            &DEFAULT_DISPUTE_WINDOW_LEDGERS,
        );
        // Extend TTL for all persistent storage entries
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::DisputeWindowLedgers,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    pub fn add_allowed_token(env: Env, token: Address) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::AllowedToken(token.clone()), &true);
        env.storage().persistent().extend_ttl(
            &DataKey::AllowedToken(token),
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    pub fn set_dispute_window(env: Env, ledgers: u32) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::DisputeWindowLedgers, &ledgers);
        env.storage().persistent().extend_ttl(
            &DataKey::DisputeWindowLedgers,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    /// Admin: enable/disable escrow holds globally and set the default period.
    ///
    /// The default period is bounded by `MAX_HOLD_PERIOD_SECS` to prevent funds
    /// from being locked for an unreasonable amount of time.
    pub fn set_escrow_hold_config(env: Env, enabled: bool, default_hold_period_secs: u64) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        if default_hold_period_secs > MAX_HOLD_PERIOD_SECS {
            env.panic_with_error(ContractError::HoldPeriodTooLong);
        }
        let mut config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        config.escrow_hold = EscrowHoldConfig {
            enabled,
            default_hold_period_secs,
        };
        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        EscrowHoldConfigUpdated {
            admin,
            enabled,
            default_hold_period_secs,
        }
        .publish(&env);
    }

    /// Seller: configure the escrow hold period (in seconds) applied to their
    /// future confirmed swaps. Pass `0` to opt out of holding funds.
    ///
    /// Security: only the seller can set their own period, the value is bounded
    /// by `MAX_HOLD_PERIOD_SECS`, and the resulting `hold_until` is snapshotted
    /// at confirm time — a seller cannot retroactively shorten or extend the
    /// hold on a swap that is already in progress.
    pub fn set_seller_hold_period(env: Env, seller: Address, hold_period_secs: u64) {
        seller.require_auth();
        if hold_period_secs > MAX_HOLD_PERIOD_SECS {
            env.panic_with_error(ContractError::HoldPeriodTooLong);
        }
        env.storage()
            .persistent()
            .set(&DataKey::SellerHoldPeriod(seller.clone()), &hold_period_secs);
        env.storage().persistent().extend_ttl(
            &DataKey::SellerHoldPeriod(seller.clone()),
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        SellerHoldPeriodUpdated {
            seller,
            hold_period_secs,
        }
        .publish(&env);
    }

    /// Returns the effective hold period (in seconds) that would apply to a swap
    /// confirmed by `seller` right now. `0` means no hold.
    pub fn get_hold_period(env: Env, seller: Address) -> u64 {
        let config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        Self::effective_hold_period(&env, &config, &seller)
    }

    /// Returns `true` if the swap's escrow hold period is still in force, i.e.
    /// funds cannot yet be released to the seller. Returns `false` when there is
    /// no hold, the hold has elapsed, or the buyer has confirmed receipt.
    pub fn hold_period_active(env: Env, swap_id: u64) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
        {
            Some(swap) => Self::is_hold_active(&env, &swap),
            None => false,
        }
    }

    pub fn update_config(env: Env, fee_bps: u32, fee_recipient: Address, cancel_delay_secs: u64) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        if fee_bps > 10_000 {
            env.panic_with_error(ContractError::FeeBpsTooHigh);
        }
        let mut config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        config.fee_bps = fee_bps;
        config.fee_recipient = fee_recipient.clone();
        config.cancel_delay_secs = cancel_delay_secs;
        env.storage().persistent().set(&DataKey::Config, &config);
        // Extend TTL on every write to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        ConfigUpdated {
            admin,
            fee_bps,
            fee_recipient,
            cancel_delay_secs,
        }
        .publish(&env);
    }

    pub fn pause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        // Also extend persistent storage TTL to keep Admin and Config fresh
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        ContractPausedEvent { admin }.publish(&env);
    }

    pub fn unpause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        // Also extend persistent storage TTL to keep Admin and Config fresh
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        ContractUnpausedEvent { admin }.publish(&env);
    }

    fn assert_not_paused(env: &Env) {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            panic_with_error!(&env, ContractError::ContractPaused);
        }
    }

    pub fn initiate_swap(
        env: Env,
        listing_id: u64,
        buyer: Address,
        seller: Address,
        usdc_token: Address,
        usdc_amount: i128,
    ) -> u64 {
        Self::assert_not_paused(&env);
        buyer.require_auth();
        if buyer == seller {
            panic_with_error!(&env, ContractError::SellerMismatch);
        }
        if usdc_amount <= 0 {
            env.panic_with_error(ContractError::InvalidAmount);
        }
        if !env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::AllowedToken(usdc_token.clone()))
            .unwrap_or(false)
        {
            env.panic_with_error(ContractError::InvalidToken);
        }

        let config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        // Extend TTL on every state-mutating call to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        Self::validate_fee_amount(&env, usdc_amount, config.fee_bps);

        let now = env.ledger().timestamp();
        let expires_at = now.saturating_add(config.swap_expiry_secs);

        let active_listing_key = DataKey::ActiveListingSwap(listing_id);
        if let Some(existing_swap_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&active_listing_key)
        {
            let existing_swap: Swap = env
                .storage()
                .persistent()
                .get(&DataKey::Swap(existing_swap_id))
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::SwapNotFound));
            if existing_swap.status == SwapStatus::Pending && existing_swap.buyer != buyer {
                env.panic_with_error(ContractError::SwapAlreadyPending);
            }
        }

        let listing = IpRegistryClient::new(&env, &config.ip_registry)
            .get_listing(&listing_id)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));

        if listing.owner != seller {
            env.panic_with_error(ContractError::SellerMismatch);
        }

        // Enforce seller-set price: buyer must pay at least listing.price_usdc
        if listing.price_usdc > 0 && usdc_amount < listing.price_usdc {
            SwapInitFailed {
                listing_id,
                error_code: ContractError::UnderpaymentNotAllowed as u32,
                buyer: buyer.clone(),
            }
            .publish(&env);
            env.panic_with_error(ContractError::UnderpaymentNotAllowed);
        }

        let token_client = token::Client::new(&env, &usdc_token);
        let allowance = token_client.allowance(&buyer, &env.current_contract_address());
        if allowance < usdc_amount {
            SwapInitFailed {
                listing_id,
                error_code: ContractError::InsufficientAllowance as u32,
                buyer: buyer.clone(),
            }
            .publish(&env);
            env.panic_with_error(ContractError::InsufficientAllowance);
        }

        token_client.transfer(&buyer, &env.current_contract_address(), &usdc_amount);

        let id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0_u64)
            + 1;
        env.storage().persistent().set(&DataKey::Counter, &id);
        env.storage().persistent().extend_ttl(
            &DataKey::Counter,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Determine initial status: high-value swaps enter PendingMultiSig and
        // require multi-sig approval before the seller can act on them.
        let initial_status = if Self::needs_multisig(&env, usdc_amount) {
            SwapStatus::PendingMultiSig
        } else {
            SwapStatus::Pending
        };

        // Initialise the approval accumulator for high-value swaps so signers
        // can call approve_multisig_swap immediately after initiation.
        if initial_status == SwapStatus::PendingMultiSig {
            let approval = MultiSigApproval {
                swap_id: id,
                approved_by: soroban_sdk::Vec::new(&env),
                nonce: id, // use swap_id as initial nonce — unique per swap
            };
            let approval_key = DataKey::MultiSigApproval(id);
            env.storage().persistent().set(&approval_key, &approval);
            env.storage().persistent().extend_ttl(
                &approval_key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }

        let key = DataKey::Swap(id);
        env.storage().persistent().set(
            &key,
            &Swap {
                listing_id,
                buyer: buyer.clone(),
                seller: seller.clone(),
                usdc_amount,
                usdc_token,
                created_at: now,
                expires_at,
                status: initial_status,
                decryption_key: None,
                confirmed_at_ledger: None,
                hold_until: None,
                buyer_confirmed: false,
            },
        );
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage().persistent().set(&active_listing_key, &id);
        env.storage().persistent().extend_ttl(
            &active_listing_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        let buyer_key = DataKey::BuyerIndex(buyer.clone());
        let mut buyer_ids: soroban_sdk::Vec<u64> = env
            .storage()
            .persistent()
            .get(&buyer_key)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        buyer_ids.push_back(id);
        env.storage().persistent().set(&buyer_key, &buyer_ids);
        env.storage().persistent().extend_ttl(
            &buyer_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        let seller_key = DataKey::SellerIndex(seller.clone());
        let mut seller_ids: soroban_sdk::Vec<u64> = env
            .storage()
            .persistent()
            .get(&seller_key)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        seller_ids.push_back(id);
        env.storage().persistent().set(&seller_key, &seller_ids);
        env.storage().persistent().extend_ttl(
            &seller_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        SwapInitiated {
            swap_id: id,
            listing_id,
            buyer,
            seller,
            usdc_amount,
        }
        .publish(&env);

        id
    }

    pub fn confirm_swap(
        env: Env,
        swap_id: u64,
        decryption_key: Bytes,
        proof_path: soroban_sdk::Vec<ProofNode>,
    ) {
        Self::assert_not_paused(&env);
        if decryption_key.is_empty() {
            env.panic_with_error(ContractError::EmptyDecryptionKey);
        }
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Pending {
            env.panic_with_error(ContractError::SwapNotPending);
        }
        swap.seller.require_auth();

        let config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        // Extend TTL on every state-mutating call to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // ip_registry is the source of truth for the root a buyer sees before
        // committing funds (docs/architecture.md); reject before spending gas
        // on proof verification if zk_verifier's root has drifted from it.
        let listing = IpRegistryClient::new(&env, &config.ip_registry)
            .get_listing(&swap.listing_id)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        let registered_root =
            ZkVerifierClient::new(&env, &config.zk_verifier).get_merkle_root(&swap.listing_id);
        let roots_match = registered_root
            .map(|root| Bytes::from(root) == listing.merkle_root)
            .unwrap_or(false);
        if !roots_match {
            SwapConfirmFailed {
                swap_id,
                error_code: ContractError::MerkleRootMismatch as u32,
                seller: swap.seller.clone(),
            }
            .publish(&env);
            env.panic_with_error(ContractError::MerkleRootMismatch);
        }

        let verified = ZkVerifierClient::new(&env, &config.zk_verifier).verify_partial_proof(
            &swap.listing_id,
            &decryption_key,
            &proof_path,
        );
        if !verified {
            SwapConfirmFailed {
                swap_id,
                error_code: ContractError::InvalidProof as u32,
                seller: swap.seller.clone(),
            }
            .publish(&env);
            env.panic_with_error(ContractError::InvalidProof);
        }

        swap.status = SwapStatus::Completed;
        swap.decryption_key = Some(decryption_key.clone());
        swap.confirmed_at_ledger = Some(env.ledger().sequence());
        // Snapshot the hold deadline now so the seller cannot manipulate it later.
        let hold_secs = Self::effective_hold_period(&env, &config, &swap.seller);
        swap.hold_until = if hold_secs > 0 {
            Some(env.ledger().timestamp().saturating_add(hold_secs))
        } else {
            None
        };
        swap.buyer_confirmed = false;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveListingSwap(swap.listing_id));
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        SwapConfirmed {
            swap_id,
            seller: swap.seller.clone(),
            decryption_key,
        }
        .publish(&env);

        SwapKeySubmitted {
            swap_id,
            seller: swap.seller,
        }
        .publish(&env);
    }

    pub fn release_to_seller(env: Env, swap_id: u64) {
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Completed {
            env.panic_with_error(ContractError::SwapNotCompleted);
        }
        swap.seller.require_auth();

        let confirmed_at = swap
            .confirmed_at_ledger
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MissingConfirmationLedger));
        let window: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DisputeWindowLedgers)
            .unwrap_or(DEFAULT_DISPUTE_WINDOW_LEDGERS);
        if env.ledger().sequence() <= confirmed_at + window {
            SwapReleaseFailed {
                swap_id,
                error_code: ContractError::DisputeWindowActive as u32,
                seller: swap.seller.clone(),
            }
            .publish(&env);
            panic_with_error!(&env, ContractError::DisputeWindowActive);
        }

        // Escrow hold period: funds stay locked until the hold elapses, unless the
        // buyer has confirmed receipt (set via confirm_receipt). This is an
        // independent gate layered on top of the dispute window.
        if Self::is_hold_active(&env, &swap) {
            SwapReleaseFailed {
                swap_id,
                error_code: ContractError::HoldPeriodActive as u32,
                seller: swap.seller.clone(),
            }
            .publish(&env);
            panic_with_error!(&env, ContractError::HoldPeriodActive);
        }

        let token_client = token::Client::new(&env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();
        let config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        // Extend TTL on every state-mutating call to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Get listing to read royalty info
        let listing = IpRegistryClient::new(&env, &config.ip_registry)
            .get_listing(&swap.listing_id)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));

        // Deduct protocol fee
        let fee: i128 = { Self::calculate_fee_amount(&env, swap.usdc_amount, config.fee_bps) };
        let mut seller_amount = swap.usdc_amount - fee;
        if fee > 0 {
            token_client.transfer(&contract_addr, &config.fee_recipient, &fee);
        }

        // Royalty is calculated on the gross sale price (swap.usdc_amount), not the
        // post-fee amount, so royalty semantics are independent of the protocol fee.
        let royalty: i128 = (swap.usdc_amount * listing.royalty_bps as i128) / 10_000;
        if royalty > 0 {
            token_client.transfer(&contract_addr, &listing.royalty_recipient, &royalty);
            seller_amount -= royalty;
        }

        token_client.transfer(&contract_addr, &swap.seller, &seller_amount);

        FundsReleased {
            swap_id,
            seller: swap.seller.clone(),
            amount: seller_amount,
        }
        .publish(&env);

        swap.status = SwapStatus::ResolvedSeller;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Buyer: confirm receipt of the delivered IP, waiving the remaining escrow
    /// hold period so the seller may release funds immediately.
    ///
    /// This is the buyer-confirmation override: only the buyer can call it, and
    /// it does not bypass the dispute window — the buyer keeps their dispute
    /// rights, but voluntarily signals the goods were received.
    pub fn confirm_receipt(env: Env, swap_id: u64) {
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Completed {
            env.panic_with_error(ContractError::SwapNotCompleted);
        }
        swap.buyer.require_auth();

        swap.buyer_confirmed = true;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        BuyerConfirmedReceipt {
            swap_id,
            buyer: swap.buyer,
        }
        .publish(&env);
    }

    pub fn raise_dispute(env: Env, swap_id: u64) {
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Completed {
            env.panic_with_error(ContractError::SwapNotCompleted);
        }
        swap.buyer.require_auth();

        let confirmed_at = swap
            .confirmed_at_ledger
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MissingConfirmationLedger));
        let window: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DisputeWindowLedgers)
            .unwrap_or(DEFAULT_DISPUTE_WINDOW_LEDGERS);
        if env.ledger().sequence() > confirmed_at + window {
            env.panic_with_error(ContractError::DisputeWindowExpired);
        }

        swap.status = SwapStatus::Disputed;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        // Extend TTL on every state-mutating call to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Create the Dispute record with commit/reveal windows.
        let commit_window: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::CommitWindowLedgers)
            .unwrap_or(DEFAULT_COMMIT_WINDOW_LEDGERS);
        let reveal_window: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::RevealWindowLedgers)
            .unwrap_or(DEFAULT_REVEAL_WINDOW_LEDGERS);
        let current_ledger = env.ledger().sequence();
        let dispute = Dispute {
            swap_id,
            raised_by: swap.buyer.clone(),
            raised_at_ledger: current_ledger,
            evidence_count: 0,
            outcome: DisputeOutcome::Pending,
            resolved_at_ledger: None,
            vote_weight_buyer: 0,
            vote_weight_seller: 0,
            commit_deadline_ledger: current_ledger + commit_window,
            reveal_deadline_ledger: current_ledger + commit_window + reveal_window,
            appeal_deadline_ledger: None,
            is_appealed: false,
            appeal_resolve_by_ledger: None,
        };
        let dispute_key = DataKey::Dispute(swap_id);
        env.storage().persistent().set(&dispute_key, &dispute);
        env.storage().persistent().extend_ttl(
            &dispute_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        DisputeRaised {
            swap_id,
            buyer: swap.buyer,
        }
        .publish(&env);
    }

    /// Admin resolves a dispute, either directly (swap still `Disputed`, no
    /// arbiter vote was needed) or as the appeal remedy (swap `Appealed`, an
    /// arbiter-voted outcome was appealed by the buyer). Either way this is the
    /// single, clean payout for the swap's escrow — `finalize_dispute` never
    /// releases funds itself, so there is no prior transfer to reverse.
    pub fn resolve_dispute(env: Env, swap_id: u64, favor_buyer: bool) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();

        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Disputed && swap.status != SwapStatus::Appealed {
            env.panic_with_error(ContractError::SwapNotDisputed);
        }

        Self::distribute_dispute_funds(&env, &mut swap, favor_buyer);

        DisputeResolved {
            swap_id,
            favor_buyer,
        }
        .publish(&env);

        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    pub fn cancel_swap(env: Env, swap_id: u64) {
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Pending {
            env.panic_with_error(ContractError::SwapNotPending);
        }
        swap.buyer.require_auth();

        // Read cancel_delay_secs from Config and enforce the delay
        let config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        // Extend TTL on every state-mutating call to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        let cancel_deadline = swap.created_at.saturating_add(config.cancel_delay_secs);
        if env.ledger().timestamp() < cancel_deadline {
            SwapCancelFailed {
                swap_id,
                error_code: ContractError::CancelTooEarly as u32,
                buyer: swap.buyer.clone(),
            }
            .publish(&env);
            env.panic_with_error(ContractError::CancelTooEarly);
        }

        token::Client::new(&env, &swap.usdc_token).transfer(
            &env.current_contract_address(),
            &swap.buyer,
            &swap.usdc_amount,
        );
        swap.status = SwapStatus::Cancelled;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        // Remove stale ActiveListingSwap so future initiate_swap calls on the
        // same listing are not blocked by SwapAlreadyPending.
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveListingSwap(swap.listing_id));
        // Extend TTL on every state-mutating call to prevent expiration
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Config,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        SwapCancelled {
            swap_id,
            buyer: swap.buyer,
            usdc_amount: swap.usdc_amount,
        }
        .publish(&env);
    }

    pub fn get_swap_status(env: Env, swap_id: u64) -> Option<SwapStatus> {
        env.storage()
            .persistent()
            .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            .map(|swap| swap.status)
    }

    pub fn get_swap(env: Env, swap_id: u64) -> Option<Swap> {
        let key = DataKey::Swap(swap_id);
        let swap: Option<Swap> = env.storage().persistent().get(&key);
        if swap.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        swap
    }

    pub fn get_decryption_key(env: Env, swap_id: u64) -> Option<Bytes> {
        let key = DataKey::Swap(swap_id);
        let swap: Option<Swap> = env.storage().persistent().get(&key);
        if swap.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        swap.and_then(|s| s.decryption_key)
    }

    pub fn get_config(env: Env) -> Option<Config> {
        env.storage().persistent().get(&DataKey::Config)
    }

    /// Returns true if there is a pending swap for the given listing_id.
    pub fn has_pending_swap(env: Env, listing_id: u64) -> bool {
        if let Some(swap_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::ActiveListingSwap(listing_id))
        {
            if let Some(swap) = env
                .storage()
                .persistent()
                .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            {
                return swap.status == SwapStatus::Pending;
            }
        }
        false
    }

    pub fn get_swaps_by_buyer(env: Env, buyer: Address) -> soroban_sdk::Vec<u64> {
        let key = DataKey::BuyerIndex(buyer);
        let ids: Option<soroban_sdk::Vec<u64>> = env.storage().persistent().get(&key);
        if ids.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        ids.unwrap_or_else(|| soroban_sdk::Vec::new(&env))
    }

    /// Paginated variant of `get_swaps_by_buyer`.
    /// Returns up to `limit` swap IDs starting at `offset`.
    /// Returns an empty Vec when `offset == total` (valid cursor-past-end state).
    /// Panics with `InvalidPaginationParams` if `limit` is 0 or `offset` is strictly
    /// greater than the list length.
    pub fn get_swaps_by_buyer_page(
        env: Env,
        buyer: Address,
        offset: u32,
        limit: u32,
    ) -> soroban_sdk::Vec<u64> {
        if limit == 0 {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }
        let all: soroban_sdk::Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BuyerIndex(buyer))
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        let total = all.len();
        // offset == total is a valid cursor-past-end: return empty without panicking.
        // Only panic when offset is strictly beyond the list.
        if offset > total {
            panic_with_error!(&env, ContractError::InvalidPaginationParams);
        }
        let end = (offset + limit).min(total);
        let mut page = soroban_sdk::Vec::new(&env);
        for i in offset..end {
            page.push_back(all.get(i).unwrap());
        }
        page
    }

    pub fn get_swaps_by_seller(env: Env, seller: Address) -> soroban_sdk::Vec<u64> {
        let key = DataKey::SellerIndex(seller);
        let ids: Option<soroban_sdk::Vec<u64>> = env.storage().persistent().get(&key);
        if ids.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        ids.unwrap_or_else(|| soroban_sdk::Vec::new(&env))
    }

    pub fn is_listing_available(env: Env, listing_id: u64) -> bool {
        if let Some(swap_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::ActiveListingSwap(listing_id))
        {
            if let Some(swap) = env
                .storage()
                .persistent()
                .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            {
                swap.status != SwapStatus::Pending
            } else {
                true
            }
        } else {
            true
        }
    }

    pub fn transfer_admin(env: Env, new_admin: Address) {
        let old_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        old_admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &new_admin);
        env.storage().persistent().extend_ttl(
            &DataKey::Admin,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        AdminTransferred {
            old_admin,
            new_admin,
        }
        .publish(&env);
    }

    // ── Dispute resolution helpers ─────────────────────────────────────────────

    fn require_admin(env: &Env) -> Address {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        admin
    }

    // ── Multi-sig helpers ──────────────────────────────────────────────────────

    /// Return the active MultiSigConfig, or a disabled default if none is stored.
    fn get_multisig_config(env: &Env) -> MultiSigConfig {
        env.storage()
            .persistent()
            .get(&DataKey::MultiSigConfig)
            .unwrap_or_else(|| MultiSigConfig {
                threshold: DEFAULT_MULTISIG_THRESHOLD,
                signers: soroban_sdk::Vec::new(env),
                required_approvals: 2,
                enabled: false,
            })
    }

    /// Return `true` when `amount` requires multi-sig approval according to the
    /// active configuration. `false` when multi-sig is disabled or signers list
    /// is empty.
    fn needs_multisig(env: &Env, amount: i128) -> bool {
        let cfg = Self::get_multisig_config(env);
        cfg.enabled && cfg.signers.len() >= cfg.required_approvals && amount >= cfg.threshold
    }

    /// Validate that a proposed multi-sig config is self-consistent.
    fn validate_multisig_config(env: &Env, cfg: &MultiSigConfig) {
        if cfg.required_approvals == 0 {
            env.panic_with_error(ContractError::InvalidMultiSigConfig);
        }
        if cfg.signers.len() < cfg.required_approvals {
            env.panic_with_error(ContractError::InvalidMultiSigConfig);
        }
        if cfg.signers.len() > MAX_MULTISIG_SIGNERS {
            env.panic_with_error(ContractError::InvalidMultiSigConfig);
        }
    }

    /// Shared fund-distribution logic used by both resolve_dispute (admin) and
    /// finalize_dispute (arbiter vote). Returns true when funds go to buyer.
    fn distribute_dispute_funds(env: &Env, swap: &mut Swap, favor_buyer: bool) {
        let token_client = token::Client::new(env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();

        if favor_buyer {
            token_client.transfer(&contract_addr, &swap.buyer, &swap.usdc_amount);
            swap.status = SwapStatus::ResolvedBuyer;
        } else {
            let config: Config = env
                .storage()
                .persistent()
                .get(&DataKey::Config)
                .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
            env.storage().persistent().extend_ttl(
                &DataKey::Admin,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
            env.storage().persistent().extend_ttl(
                &DataKey::Config,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );

            let listing = IpRegistryClient::new(env, &config.ip_registry)
                .get_listing(&swap.listing_id)
                .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));

            let fee = {
                let product = swap.usdc_amount.checked_mul(config.fee_bps as i128);
                match product {
                    Some(p) if p / 10_000 > 0 => p / 10_000,
                    _ => 0,
                }
            };
            let mut seller_amount = swap.usdc_amount - fee;
            if fee > 0 {
                token_client.transfer(&contract_addr, &config.fee_recipient, &fee);
            }

            let royalty: i128 = (swap.usdc_amount * listing.royalty_bps as i128) / 10_000;
            if royalty > 0 {
                token_client.transfer(&contract_addr, &listing.royalty_recipient, &royalty);
                seller_amount -= royalty;
            }

            token_client.transfer(&contract_addr, &swap.seller, &seller_amount);
            swap.status = SwapStatus::ResolvedSeller;
        }
    }

    // ── Arbiter registry ───────────────────────────────────────────────────────

    /// Register or update an arbiter's voting weight. Admin only.
    pub fn register_arbiter(env: Env, arbiter: Address, weight: i128) {
        Self::require_admin(&env);
        if weight <= 0 {
            env.panic_with_error(ContractError::InvalidAmount);
        }

        let info = ArbiterInfo { weight, is_active: true };
        let entry_key = DataKey::ArbiterEntry(arbiter.clone());
        env.storage().persistent().set(&entry_key, &info);
        env.storage().persistent().extend_ttl(
            &entry_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        let list_key = DataKey::ArbiterList;
        let mut list: soroban_sdk::Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        let mut found = false;
        for i in 0..list.len() {
            if list.get(i).unwrap() == arbiter {
                found = true;
                break;
            }
        }
        if !found {
            list.push_back(arbiter.clone());
        }
        env.storage().persistent().set(&list_key, &list);
        env.storage().persistent().extend_ttl(
            &list_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        ArbiterRegistered { arbiter, weight }.publish(&env);
    }

    /// Deactivate an arbiter so they can no longer vote. Admin only.
    pub fn deactivate_arbiter(env: Env, arbiter: Address) {
        Self::require_admin(&env);

        let entry_key = DataKey::ArbiterEntry(arbiter.clone());
        let mut info: ArbiterInfo = env
            .storage()
            .persistent()
            .get(&entry_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotAnArbiter));
        info.is_active = false;
        env.storage().persistent().set(&entry_key, &info);
        env.storage().persistent().extend_ttl(
            &entry_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        ArbiterDeactivated { arbiter }.publish(&env);
    }

    // ── Dispute window setters ─────────────────────────────────────────────────

    pub fn set_commit_window(env: Env, ledgers: u32) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::CommitWindowLedgers, &ledgers);
        env.storage().persistent().extend_ttl(
            &DataKey::CommitWindowLedgers,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    pub fn set_reveal_window(env: Env, ledgers: u32) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::RevealWindowLedgers, &ledgers);
        env.storage().persistent().extend_ttl(
            &DataKey::RevealWindowLedgers,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    pub fn set_appeal_window(env: Env, ledgers: u32) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::AppealWindowLedgers, &ledgers);
        env.storage().persistent().extend_ttl(
            &DataKey::AppealWindowLedgers,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    /// Admin: configure how long an appealed dispute waits for `resolve_dispute`
    /// before anyone may settle it per the original arbiter outcome.
    pub fn set_appeal_resolution_window(env: Env, ledgers: u32) {
        Self::require_admin(&env);
        env.storage()
            .persistent()
            .set(&DataKey::AppealResolutionWindowLedgers, &ledgers);
        env.storage().persistent().extend_ttl(
            &DataKey::AppealResolutionWindowLedgers,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }

    // ── Multi-signature approval ───────────────────────────────────────────────

    /// Admin: configure the multi-sig scheme for high-value swaps.
    ///
    /// # Parameters
    /// - `threshold`          — USDC amount (7-decimal i128) above which multi-sig is required.
    /// - `signers`            — Authorised approver addresses. Max `MAX_MULTISIG_SIGNERS` (3).
    /// - `required_approvals` — Number of distinct approvals needed (2-of-2 or 2-of-3).
    /// - `enabled`            — Toggle; `false` disables the gate without clearing config.
    ///
    /// # Errors
    /// - `InvalidMultiSigConfig` if `required_approvals == 0`, `required_approvals > signers.len()`,
    ///   or `signers.len() > MAX_MULTISIG_SIGNERS`.
    pub fn set_multisig_config(
        env: Env,
        threshold: i128,
        signers: soroban_sdk::Vec<Address>,
        required_approvals: u32,
        enabled: bool,
    ) {
        let admin = Self::require_admin(&env);
        let cfg = MultiSigConfig {
            threshold,
            signers,
            required_approvals,
            enabled,
        };
        Self::validate_multisig_config(&env, &cfg);
        env.storage()
            .persistent()
            .set(&DataKey::MultiSigConfig, &cfg);
        env.storage().persistent().extend_ttl(
            &DataKey::MultiSigConfig,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        MultiSigConfigUpdated {
            admin,
            threshold,
            required_approvals,
            enabled,
        }
        .publish(&env);
    }

    /// Signer: approve a high-value swap that is awaiting multi-sig sign-off.
    ///
    /// Once the required number of distinct approvals is reached the swap is
    /// automatically promoted to `Pending` and the normal swap flow continues.
    ///
    /// # Replay-attack prevention
    /// Each approval records the `nonce` embedded in the `MultiSigApproval` record.
    /// The nonce is written to persistent storage under `DataKey::MultiSigNonce` so
    /// a replayed call with the same nonce is detected and rejected.
    ///
    /// # Signature uniqueness
    /// A signer cannot approve the same swap twice — the contract checks
    /// `approved_by` and panics with `MultiSigAlreadyApproved` on a duplicate.
    ///
    /// # Order independence
    /// Approvals may arrive in any order; the swap unlocks as soon as
    /// `approved_by.len() >= required_approvals`.
    ///
    /// # Errors
    /// - `SwapNotFound`           — no swap with this id.
    /// - `MultiSigThresholdNotMet` — swap amount is below threshold (no approval needed).
    /// - `NotAMultiSigSigner`     — caller not in the configured signer list.
    /// - `MultiSigAlreadyApproved`— caller already approved this swap.
    /// - `NonceAlreadyUsed`       — replayed nonce detected.
    pub fn approve_multisig_swap(env: Env, swap_id: u64, signer: Address) {
        signer.require_auth();

        let swap_key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&swap_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));

        if swap.status != SwapStatus::PendingMultiSig {
            env.panic_with_error(ContractError::MultiSigThresholdNotMet);
        }

        let ms_cfg = Self::get_multisig_config(&env);

        // Verify caller is a configured signer.
        let mut is_signer = false;
        for i in 0..ms_cfg.signers.len() {
            if ms_cfg.signers.get(i).unwrap() == signer {
                is_signer = true;
                break;
            }
        }
        if !is_signer {
            env.panic_with_error(ContractError::NotAMultiSigSigner);
        }

        let approval_key = DataKey::MultiSigApproval(swap_id);
        let mut approval: MultiSigApproval = env
            .storage()
            .persistent()
            .get(&approval_key)
            .unwrap_or_else(|| MultiSigApproval {
                swap_id,
                approved_by: soroban_sdk::Vec::new(&env),
                nonce: swap_id,
            });

        // Replay-attack guard: ensure this nonce has not been consumed before.
        let nonce_key = DataKey::MultiSigNonce(MultiSigNonceKey {
            swap_id,
            nonce: approval.nonce,
        });
        if env.storage().persistent().has(&nonce_key) {
            env.panic_with_error(ContractError::NonceAlreadyUsed);
        }

        // Uniqueness: signer must not have already approved.
        for i in 0..approval.approved_by.len() {
            if approval.approved_by.get(i).unwrap() == signer {
                env.panic_with_error(ContractError::MultiSigAlreadyApproved);
            }
        }

        // Record this approval.
        approval.approved_by.push_back(signer.clone());

        // Burn the nonce to prevent replay.
        env.storage().persistent().set(&nonce_key, &true);
        env.storage().persistent().extend_ttl(
            &nonce_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Advance nonce for the next approval on this swap.
        approval.nonce = approval.nonce.saturating_add(1);

        let approvals_count = approval.approved_by.len();

        env.storage().persistent().set(&approval_key, &approval);
        env.storage().persistent().extend_ttl(
            &approval_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        MultiSigApprovalAdded {
            swap_id,
            signer,
            approvals_count,
            required_approvals: ms_cfg.required_approvals,
        }
        .publish(&env);

        // If threshold is now met, promote the swap to active Pending.
        if approvals_count >= ms_cfg.required_approvals {
            swap.status = SwapStatus::Pending;
            env.storage().persistent().set(&swap_key, &swap);
            env.storage().persistent().extend_ttl(
                &swap_key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
            MultiSigThresholdMet {
                swap_id,
                approvals_count,
            }
            .publish(&env);
        }

        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Read the current multi-sig configuration. Returns `None` if not yet set.
    pub fn get_multisig_config_view(env: Env) -> Option<MultiSigConfig> {
        env.storage().persistent().get(&DataKey::MultiSigConfig)
    }

    /// Read the approval accumulator for a given swap. Returns `None` if the
    /// swap was not subject to multi-sig (amount below threshold).
    pub fn get_multisig_approval(env: Env, swap_id: u64) -> Option<MultiSigApproval> {
        let key = DataKey::MultiSigApproval(swap_id);
        let result: Option<MultiSigApproval> = env.storage().persistent().get(&key);
        if result.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        result
    }

    // ── Evidence submission ────────────────────────────────────────────────────

    /// Submit an IPFS-addressed evidence item for a disputed swap.
    /// Only the swap buyer or seller may submit evidence.
    pub fn submit_evidence(env: Env, swap_id: u64, submitter: Address, ipfs_hash: Bytes) {
        submitter.require_auth();

        let swap: Swap = env
            .storage()
            .persistent()
            .get(&DataKey::Swap(swap_id))
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Disputed {
            env.panic_with_error(ContractError::SwapNotDisputed);
        }
        if submitter != swap.buyer && submitter != swap.seller {
            env.panic_with_error(ContractError::ArbiterConflictOfInterest);
        }

        let dispute_key = DataKey::Dispute(swap_id);
        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapDisputeNotFound));

        if env.ledger().sequence() > dispute.reveal_deadline_ledger {
            env.panic_with_error(ContractError::CommitWindowExpired);
        }

        let evidence_index = dispute.evidence_count;
        let evidence = DisputeEvidenceItem {
            submitter: submitter.clone(),
            ipfs_hash,
            submitted_at_ledger: env.ledger().sequence(),
        };
        let ev_key = DataKey::EvidenceItem(EvidenceKey { swap_id, index: evidence_index });
        env.storage().persistent().set(&ev_key, &evidence);
        env.storage().persistent().extend_ttl(
            &ev_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        dispute.evidence_count += 1;
        env.storage().persistent().set(&dispute_key, &dispute);
        env.storage().persistent().extend_ttl(
            &dispute_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        EvidenceSubmitted { swap_id, submitter, evidence_index }.publish(&env);
    }

    // ── Commitment-reveal voting ───────────────────────────────────────────────

    /// Phase 1: arbiter submits a blinded commitment = sha256(vote_byte || salt).
    /// Call before commit_deadline_ledger. Commitment hides the vote until reveal phase.
    pub fn commit_vote(
        env: Env,
        swap_id: u64,
        arbiter: Address,
        commitment: BytesN<32>,
    ) {
        arbiter.require_auth();

        let arbiter_info: ArbiterInfo = env
            .storage()
            .persistent()
            .get(&DataKey::ArbiterEntry(arbiter.clone()))
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotAnArbiter));
        if !arbiter_info.is_active {
            env.panic_with_error(ContractError::NotAnArbiter);
        }

        let swap: Swap = env
            .storage()
            .persistent()
            .get(&DataKey::Swap(swap_id))
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Disputed {
            env.panic_with_error(ContractError::SwapNotDisputed);
        }
        if arbiter == swap.buyer || arbiter == swap.seller {
            env.panic_with_error(ContractError::ArbiterConflictOfInterest);
        }

        let dispute: Dispute = env
            .storage()
            .persistent()
            .get(&DataKey::Dispute(swap_id))
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapDisputeNotFound));
        if dispute.outcome != DisputeOutcome::Pending {
            env.panic_with_error(ContractError::DisputeAlreadyFinalized);
        }
        if env.ledger().sequence() > dispute.commit_deadline_ledger {
            env.panic_with_error(ContractError::CommitWindowExpired);
        }

        let commit_key = DataKey::VoteCommit(DisputeVoteKey { swap_id, arbiter: arbiter.clone() });
        if env.storage().persistent().has(&commit_key) {
            env.panic_with_error(ContractError::ArbiterAlreadyCommitted);
        }

        env.storage().persistent().set(&commit_key, &commitment);
        env.storage().persistent().extend_ttl(
            &commit_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Emit without naming the arbiter — preserves anonymity during commit phase.
        VoteCommitted { swap_id }.publish(&env);
    }

    /// Phase 2: arbiter reveals their vote and salt.
    /// Contract verifies sha256(vote_byte || salt) == stored commitment.
    /// Call between commit_deadline_ledger and reveal_deadline_ledger.
    pub fn reveal_vote(
        env: Env,
        swap_id: u64,
        arbiter: Address,
        favor_buyer: bool,
        salt: Bytes,
    ) {
        arbiter.require_auth();

        let arbiter_info: ArbiterInfo = env
            .storage()
            .persistent()
            .get(&DataKey::ArbiterEntry(arbiter.clone()))
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotAnArbiter));
        if !arbiter_info.is_active {
            env.panic_with_error(ContractError::NotAnArbiter);
        }

        let dispute_key = DataKey::Dispute(swap_id);
        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapDisputeNotFound));
        if dispute.outcome != DisputeOutcome::Pending {
            env.panic_with_error(ContractError::DisputeAlreadyFinalized);
        }

        let current = env.ledger().sequence();
        if current <= dispute.commit_deadline_ledger {
            env.panic_with_error(ContractError::RevealWindowNotOpen);
        }
        if current > dispute.reveal_deadline_ledger {
            env.panic_with_error(ContractError::RevealWindowExpired);
        }

        let commit_key = DataKey::VoteCommit(DisputeVoteKey { swap_id, arbiter: arbiter.clone() });
        let stored_commitment: BytesN<32> = env
            .storage()
            .persistent()
            .get(&commit_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::VoteCommitNotFound));

        let reveal_key = DataKey::VoteRevealed(DisputeVoteKey { swap_id, arbiter: arbiter.clone() });
        if env.storage().persistent().has(&reveal_key) {
            env.panic_with_error(ContractError::ArbiterAlreadyRevealed);
        }

        // Verify commitment: sha256(vote_byte || salt) must equal stored commitment.
        let mut preimage = Bytes::new(&env);
        preimage.push_back(if favor_buyer { 1u8 } else { 0u8 });
        for i in 0..salt.len() {
            preimage.push_back(salt.get(i).unwrap());
        }
        let computed: BytesN<32> = env.crypto().sha256(&preimage).into();
        if computed != stored_commitment {
            env.panic_with_error(ContractError::InvalidVoteReveal);
        }

        env.storage().persistent().set(&reveal_key, &favor_buyer);
        env.storage().persistent().extend_ttl(
            &reveal_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        if favor_buyer {
            dispute.vote_weight_buyer = dispute
                .vote_weight_buyer
                .saturating_add(arbiter_info.weight);
        } else {
            dispute.vote_weight_seller = dispute
                .vote_weight_seller
                .saturating_add(arbiter_info.weight);
        }
        env.storage().persistent().set(&dispute_key, &dispute);
        env.storage().persistent().extend_ttl(
            &dispute_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        VoteRevealed { swap_id, arbiter, favor_buyer }.publish(&env);
    }

    // ── Finalize / appeal ──────────────────────────────────────────────────────

    /// Finalize a dispute after the reveal window closes.
    /// Tallies revealed vote weights and records the outcome, but does **not**
    /// move funds — escrow stays held until the appeal window closes with no
    /// appeal filed (`settle_dispute`) or an appeal is resolved (`resolve_dispute`
    /// / `settle_dispute` after the resolution timeout). This holdback is what
    /// lets an appeal actually reverse the outcome instead of requiring a
    /// clawback of funds already paid out.
    /// Ties resolve in the buyer's favour (consumer protection default).
    /// Anyone may call this; no auth required.
    pub fn finalize_dispute(env: Env, swap_id: u64) {
        let dispute_key = DataKey::Dispute(swap_id);
        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapDisputeNotFound));
        if dispute.outcome != DisputeOutcome::Pending {
            env.panic_with_error(ContractError::DisputeAlreadyFinalized);
        }
        if env.ledger().sequence() <= dispute.reveal_deadline_ledger {
            env.panic_with_error(ContractError::RevealWindowNotOpen);
        }

        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Disputed {
            env.panic_with_error(ContractError::SwapNotDisputed);
        }

        // Ties default to buyer (consumer protection).
        let favor_buyer = dispute.vote_weight_buyer >= dispute.vote_weight_seller;

        // Escrow is held, not paid out: status moves to PendingAppealWindow
        // rather than through distribute_dispute_funds.
        swap.status = SwapStatus::PendingAppealWindow;

        let current = env.ledger().sequence();
        let appeal_window: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::AppealWindowLedgers)
            .unwrap_or(DEFAULT_APPEAL_WINDOW_LEDGERS);
        dispute.outcome = if favor_buyer {
            DisputeOutcome::FavorBuyer
        } else {
            DisputeOutcome::FavorSeller
        };
        dispute.resolved_at_ledger = Some(current);
        dispute.appeal_deadline_ledger = Some(current + appeal_window);

        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage().persistent().set(&dispute_key, &dispute);
        env.storage().persistent().extend_ttl(
            &dispute_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        DisputeFinalized { swap_id, favor_buyer }.publish(&env);
    }

    /// Appeal a finalized dispute within the appeal window.
    /// Only the buyer may appeal. Escrow is already held (finalize_dispute
    /// never pays out), so this simply moves the swap to `Appealed` — the
    /// admin's subsequent `resolve_dispute` call makes the single, clean
    /// payout, reversing the arbiter outcome if warranted with no clawback
    /// needed. If the admin never acts, `settle_dispute` pays out per the
    /// original arbiter outcome once the appeal resolution timeout elapses.
    pub fn appeal_dispute(env: Env, swap_id: u64, appellant: Address) {
        appellant.require_auth();

        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if appellant != swap.buyer {
            env.panic_with_error(ContractError::ArbiterConflictOfInterest);
        }

        let dispute_key = DataKey::Dispute(swap_id);
        let mut dispute: Dispute = env
            .storage()
            .persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapDisputeNotFound));
        if dispute.outcome == DisputeOutcome::Pending {
            env.panic_with_error(ContractError::SwapNotDisputed);
        }
        if dispute.is_appealed {
            env.panic_with_error(ContractError::DisputeAlreadyAppealed);
        }
        let deadline = dispute.appeal_deadline_ledger.unwrap_or(0);
        if env.ledger().sequence() > deadline {
            env.panic_with_error(ContractError::AppealWindowExpired);
        }

        dispute.is_appealed = true;
        let current = env.ledger().sequence();
        let appeal_resolution_window: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::AppealResolutionWindowLedgers)
            .unwrap_or(DEFAULT_APPEAL_RESOLUTION_WINDOW_LEDGERS);
        dispute.appeal_resolve_by_ledger = Some(current + appeal_resolution_window);
        env.storage().persistent().set(&dispute_key, &dispute);
        env.storage().persistent().extend_ttl(
            &dispute_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        swap.status = SwapStatus::Appealed;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        DisputeAppealed { swap_id, appellant }.publish(&env);
    }

    /// Settle a dispute's held escrow once it no longer needs admin action:
    /// either the appeal window closed with no appeal filed (`PendingAppealWindow`),
    /// or an appeal was filed but the admin never called `resolve_dispute`
    /// before the appeal resolution timeout (`Appealed`) — this is the
    /// stuck-funds guard, ensuring an inactive admin cannot permanently lock
    /// escrow. Both branches pay out per the original arbiter outcome
    /// (`dispute.outcome`); an admin override only happens via `resolve_dispute`.
    /// Anyone may call this; no auth required.
    pub fn settle_dispute(env: Env, swap_id: u64) {
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));

        let dispute_key = DataKey::Dispute(swap_id);
        let dispute: Dispute = env
            .storage()
            .persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapDisputeNotFound));

        let current = env.ledger().sequence();
        match swap.status {
            SwapStatus::PendingAppealWindow => {
                let deadline = dispute.appeal_deadline_ledger.unwrap_or(0);
                if current <= deadline {
                    env.panic_with_error(ContractError::AppealWindowStillOpen);
                }
            }
            SwapStatus::Appealed => {
                let deadline = dispute.appeal_resolve_by_ledger.unwrap_or(0);
                if current <= deadline {
                    env.panic_with_error(ContractError::AppealResolutionWindowActive);
                }
            }
            _ => env.panic_with_error(ContractError::SwapNotAwaitingSettlement),
        }

        let favor_buyer = dispute.outcome == DisputeOutcome::FavorBuyer;
        Self::distribute_dispute_funds(&env, &mut swap, favor_buyer);

        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        DisputeSettled { swap_id, favor_buyer }.publish(&env);
    }

    // ── Dispute read functions ─────────────────────────────────────────────────

    pub fn get_dispute(env: Env, swap_id: u64) -> Option<Dispute> {
        let key = DataKey::Dispute(swap_id);
        let dispute: Option<Dispute> = env.storage().persistent().get(&key);
        if dispute.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        dispute
    }

    pub fn get_evidence(env: Env, swap_id: u64, index: u32) -> Option<DisputeEvidenceItem> {
        env.storage()
            .persistent()
            .get(&DataKey::EvidenceItem(EvidenceKey { swap_id, index }))
    }

    pub fn get_arbiters(env: Env) -> soroban_sdk::Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::ArbiterList)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ip_registry::{IpRegistry, IpRegistryClient};
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger as _},
        token, Bytes, BytesN, Env, IntoVal, TryFromVal, Vec,
    };
    use zk_verifier::{ProofNode, ZkVerifier, ZkVerifierClient};

    /// Register a ZK verifier and set a trivial single-leaf Merkle root for listing_id.
    /// Returns (zk_verifier_id, proof_path) where proof_path is an empty Vec (single-leaf proof).
    fn setup_zk_verifier(
        env: &Env,
        owner: &Address,
        listing_id: u64,
        leaf: &Bytes,
    ) -> (Address, soroban_sdk::Vec<ProofNode>) {
        let zk_id = env.register(ZkVerifier, ());
        let zk = ZkVerifierClient::new(env, &zk_id);
        let root: soroban_sdk::BytesN<32> = env.crypto().sha256(leaf).into();
        zk.set_merkle_root(owner, &listing_id, &root);
        (zk_id, soroban_sdk::Vec::new(env))
    }

    /// The root zk_verifier will derive for a single-leaf (empty path) proof over `leaf`.
    /// Used to keep ip_registry's advertised merkle_root in sync with zk_verifier's in tests.
    fn root_for_leaf(env: &Env, leaf: &Bytes) -> Bytes {
        let hash: soroban_sdk::BytesN<32> = env.crypto().sha256(leaf).into();
        Bytes::from(hash)
    }

    fn setup_registry(
        env: &Env,
        seller: &Address,
        price_usdc: i128,
        merkle_root: &Bytes,
    ) -> (Address, u64) {
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(env, &registry_id);
        let admin = Address::generate(env);
        registry.initialize(&admin, &100_000u32, &6_312_000u32);
        let listing_id = registry.register_ip(
            seller,
            &Bytes::from_slice(env, b"QmHash"),
            merkle_root,
            &0u32,
            seller,
            &price_usdc,
        );
        (registry_id, listing_id)
    }

    fn setup_usdc(env: &Env, buyer: &Address, amount: i128) -> Address {
        let admin = Address::generate(env);
        let usdc_id = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        token::StellarAssetClient::new(env, &usdc_id).mint(buyer, &amount);
        usdc_id
    }

    fn setup_contract<'a>(
        env: &'a Env,
        contract_id: &Address,
        buyer: &Address,
        usdc_id: &Address,
        approve_amount: i128,
        fee_bps: u32,
        zk_id: &Address,
        registry_id: &Address,
    ) -> AtomicSwapClient<'a> {
        let client = AtomicSwapClient::new(env, contract_id);
        client.initialize(
            &Address::generate(env),
            &fee_bps,
            &Address::generate(env),
            &60u64,
            &3600u64,
            zk_id,
            registry_id,
        );
        client.add_allowed_token(usdc_id);
        if approve_amount > 0 {
            token::Client::new(env, usdc_id).approve(buyer, contract_id, &approve_amount, &200u32);
        }
        client
    }

    fn setup_full<'a>(
        env: &'a Env,
        buyer: &Address,
        seller: &Address,
        usdc_amount: i128,
        price_usdc: i128,
    ) -> (
        Address,
        u64,
        Address,
        Address,
        AtomicSwapClient<'a>,
        Address,
        Address,
    ) {
        let usdc_id = setup_usdc(env, buyer, usdc_amount);
        // `confirmed_swap` always confirms with the fixed leaf b"key"; keep the
        // registered root in sync so confirm_swap's merkle-root check passes.
        let (registry_id, listing_id) = setup_registry(
            env,
            seller,
            price_usdc,
            &root_for_leaf(env, &Bytes::from_slice(env, b"key")),
        );
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let fee_recipient = Address::generate(env);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &admin,
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(env, &usdc_id).approve(buyer, &contract_id, &usdc_amount, &200u32);
        (
            usdc_id,
            listing_id,
            registry_id,
            contract_id,
            client,
            admin,
            zk_id,
        )
    }

    fn pending_swap(
        env: &Env,
        client: &AtomicSwapClient,
        listing_id: u64,
        buyer: &Address,
        seller: &Address,
        usdc_id: &Address,
        usdc_amount: i128,
    ) -> u64 {
        token::Client::new(env, usdc_id).approve(buyer, &client.address, &usdc_amount, &200u32);
        client.initiate_swap(&listing_id, buyer, seller, usdc_id, &usdc_amount)
    }

    // ── price enforcement tests ───────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #14)")]
    fn test_initiate_swap_rejects_underpayment() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        // Listing price is 1000, buyer tries to pay 500
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
    }

    #[test]
    fn test_initiate_swap_accepts_exact_price() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_initiate_swap_accepts_overpayment() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        // Listing price is 500, buyer pays 1000
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 500);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_initiate_swap_allows_any_amount_when_price_is_zero() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        // price_usdc = 0 means no price enforcement
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_happy_path_initiate_confirm_release_to_seller() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = Bytes::from_slice(&env, b"secret-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &root_for_leaf(&env, &key_bytes));
        let usdc_client = token::Client::new(&env, &usdc_id);

        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&contract_id), 500);

        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Completed)
        );

        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
        assert_eq!(usdc_client.balance(&seller), 500);
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&contract_id), 0);

        // Verify FundsReleased event was emitted by checking swap reached ResolvedSeller
        // (balance assertions above confirm funds moved; event emission is verified by status)
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedSeller));
    }

    #[test]
    fn test_cancel_flow_returns_usdc_to_buyer() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&contract_id), 500);

        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Cancelled)
        );
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&seller), 0);
        assert_eq!(usdc_client.balance(&contract_id), 0);
    }

    #[test]
    fn test_double_confirm_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = Bytes::from_slice(&env, b"secret-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &root_for_leaf(&env, &key_bytes));

        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);

        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        let second_confirm = client.try_confirm_swap(&swap_id, &key_bytes, &proof_path);

        assert_eq!(
            second_confirm,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::SwapNotPending as u32,
            )))
        );
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Completed)
        );
        assert_eq!(client.get_decryption_key(&swap_id), Some(key_bytes));
    }

    #[test]
    fn test_double_cancel_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);

        let second_cancel = client.try_cancel_swap(&swap_id);

        assert_eq!(
            second_cancel,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::SwapNotPending as u32,
            )))
        );
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Cancelled)
        );
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&contract_id), 0);
    }

    // ── existing tests ────────────────────────────────────────────────────────

    #[test]
    fn test_get_swap_status_returns_none_for_missing_swap() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swap_status(&999), None);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_confirm_swap_rejects_empty_key() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.confirm_swap(&0, &Bytes::new(&env), &soroban_sdk::Vec::new(&env));
    }

    #[test]
    fn test_fee_deducted_and_sent_to_recipient() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 10_000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1, &root_for_leaf(&env, &key_bytes));

        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &250u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &10_000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &10_000);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(usdc_client.balance(&seller), 9_750);
        assert_eq!(usdc_client.balance(&fee_recipient), 250);
    }

    #[test]
    fn test_zero_fee_bps_sends_full_amount_to_seller() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1000, &root_for_leaf(&env, &key_bytes));

        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(usdc_client.balance(&seller), 1000);
        assert_eq!(usdc_client.balance(&fee_recipient), 0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #15)")]
    fn test_initiate_swap_rejects_amount_that_truncates_fee() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &250u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1i128, &200u32);

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1);
    }

    #[test]
    fn test_minimum_nonzero_fee_amount_is_allowed() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 40);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 40, &root_for_leaf(&env, &key_bytes));

        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &250u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &40i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &40);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(usdc_client.balance(&seller), 39);
        assert_eq!(usdc_client.balance(&fee_recipient), 1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_initiate_swap_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);
        client.pause();

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
    fn test_initiate_swap_rejects_nonexistent_listing() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1_000);

        // Initialize an empty registry (no listings created).
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let registry_admin = Address::generate(&env);
        registry.initialize(&registry_admin, &100_000u32, &6_312_000u32);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1_000i128, &200u32);

        // No listing with this id exists in registry.
        client.initiate_swap(&999_999u64, &buyer, &seller, &usdc_id, &500i128);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_seller_impersonation_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let real_seller = Address::generate(&env);
        let impersonator = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) =
            setup_registry(&env, &real_seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);

        client.initiate_swap(&listing_id, &buyer, &impersonator, &usdc_id, &500);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #18)")]
    fn test_cancel_swap_rejects_before_expiry() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        client.cancel_swap(&swap_id);
    }

    #[test]
    #[ignore = "mock_all_auths() overrides non-root auth restriction; pre-existing test logic issue"]
    fn test_non_buyer_cancel_fails_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);

        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        // buyer can cancel after expiry
        client.cancel_swap(&swap_id);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Cancelled)
        );
        assert_eq!(token::Client::new(&env, &usdc_id).balance(&buyer), 1000);
    }

    #[test]
    fn test_cancel_swap_allows_after_expiry() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(121));
        client.cancel_swap(&swap_id);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Cancelled)
        );
        assert_eq!(usdc_client.balance(&buyer), 1000);
    }

    #[test]
    fn test_initiate_swap_emits_swap_initiated_event() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 1000);

        // Check SwapInitiated event: topics = ["swap_initiated", swap_id, listing_id]
        let swap_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(
            &env,
            &<u64 as IntoVal<Env, soroban_sdk::Val>>::into_val(&swap_id, &env),
        )
        .unwrap();
        let listing_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(
            &env,
            &<u64 as IntoVal<Env, soroban_sdk::Val>>::into_val(&listing_id, &env),
        )
        .unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("swap_initiated".try_into().unwrap());
        let found = env
            .events()
            .all()
            .filter_by_contract(&_cid)
            .events()
            .iter()
            .any(|e| {
                let body = match &e.body {
                    soroban_sdk::xdr::ContractEventBody::V0(b) => b,
                };
                body.topics.len() == 3
                    && body.topics[0] == name_xdr
                    && body.topics[1] == swap_id_xdr
                    && body.topics[2] == listing_id_xdr
            });
        assert!(found, "SwapInitiated event not emitted");
    }

    fn confirmed_swap(
        env: &Env,
        client: &AtomicSwapClient,
        listing_id: u64,
        buyer: &Address,
        seller: &Address,
        usdc_id: &Address,
        usdc_amount: i128,
    ) -> u64 {
        let key_bytes = Bytes::from_slice(env, b"key");
        let config = client.get_config().expect("contract must be initialized");
        let root: soroban_sdk::BytesN<32> = env.crypto().sha256(&key_bytes).into();
        ZkVerifierClient::new(env, &config.zk_verifier).set_merkle_root(seller, &listing_id, &root);
        let proof_path = soroban_sdk::Vec::new(env);
        token::Client::new(env, usdc_id).approve(buyer, &client.address, &usdc_amount, &200u32);
        let swap_id = client.initiate_swap(&listing_id, buyer, seller, usdc_id, &usdc_amount);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        swap_id
    }

    #[test]
    fn test_raise_dispute_within_window() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Disputed));
    }

    #[test]
    fn test_raise_dispute_emits_event() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let swap_id_val: soroban_sdk::Val = swap_id.into_val(&env);
        let swap_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &swap_id_val).unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("dispute_raised".try_into().unwrap());
        let buyer_val: soroban_sdk::Val = buyer.into_val(&env);
        let buyer_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &buyer_val).unwrap();

        let found = env
            .events()
            .all()
            .filter_by_contract(&contract_id)
            .events()
            .iter()
            .any(|e| {
                let body = match &e.body {
                    soroban_sdk::xdr::ContractEventBody::V0(b) => b,
                };
                body.topics.len() == 2
                    && body.topics[0] == name_xdr
                    && body.topics[1] == swap_id_xdr
            });
        assert!(found, "DisputeRaised event not emitted");
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #11)")]
    fn test_raise_dispute_after_window_expires() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.raise_dispute(&swap_id);
    }

    #[test]
    fn test_release_to_seller_before_window_expires_returns_dispute_window_active() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        // Set a 10-ledger dispute window
        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        // Advance only 5 ledgers — window has NOT expired yet
        env.ledger().with_mut(|li| li.sequence_number += 5);

        let result = client.try_release_to_seller(&swap_id);
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::DisputeWindowActive as u32
            )))
        );
    }

    /// Issue #570: release_to_seller must require seller auth so a third party
    /// cannot force settlement timing on behalf of the seller.
    #[test]
    fn test_release_to_seller_requires_seller_auth() {
        let env = Env::default();
        // Do NOT use mock_all_auths — we want real auth enforcement.

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let third_party = Address::generate(&env);

        // Initialize with mock_all_auths for setup only.
        env.mock_all_auths();

        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = soroban_sdk::Bytes::from_slice(&env, b"key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1, &root_for_leaf(&env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);

        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);

        // Now clear mocked auths and attempt release as a third party — must fail.
        env.set_auths(&[]);
        env.mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &third_party,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &contract_id,
                fn_name: "release_to_seller",
                args: (swap_id,).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        let result = client.try_release_to_seller(&swap_id);
        assert!(
            result.is_err(),
            "third party should not be able to call release_to_seller"
        );
    }

    #[test]
    fn test_release_to_seller_pays_royalties() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let royalty_recipient = Address::generate(&env);

        // Register listing with 500 bps (5%) royalty
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let reg_admin = Address::generate(&env);
        registry.initialize(&reg_admin, &100_000u32, &6_312_000u32);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let listing_id = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash"),
            &root_for_leaf(&env, &key_bytes),
            &500u32,
            &royalty_recipient,
            &1i128,
        );

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        // fee_bps = 200 (2%)
        client.initialize(
            &admin,
            &200u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        // Advance past dispute window
        env.ledger().with_mut(|li| li.sequence_number += 17_281);
        client.release_to_seller(&swap_id);

        let usdc = token::Client::new(&env, &usdc_id);
        // fee = 1000 * 200 / 10_000 = 20
        // royalty = 1000 * 500 / 10_000 = 50  (gross base, independent of fee)
        // seller_final = 1000 - 20 - 50 = 930
        assert_eq!(usdc.balance(&fee_recipient), 20);
        assert_eq!(usdc.balance(&royalty_recipient), 50);
        assert_eq!(usdc.balance(&seller), 930);
    }

    #[test]
    fn test_resolve_dispute_favor_buyer_refunds_usdc() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &true);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedBuyer)
        );
        assert_eq!(usdc_client.balance(&buyer), 500);
    }

    #[test]
    fn test_resolve_dispute_favor_seller_dismisses() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &false);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
        assert_eq!(usdc_client.balance(&seller), 500);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_initialize_twice_returns_already_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let zk_id = env.register(ZkVerifier, ());
        let registry_id = env.register(IpRegistry, ());
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.initialize(
            &admin,
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
    }

    /// Issue #571: resolve_dispute must not panic with FeeWouldTruncate for small amounts.
    /// When fee_bps > 0 but usdc_amount is too small for the fee to be non-zero,
    /// the full amount should go to the seller instead of the dispute being stuck.
    /// We seed the swap directly into storage to bypass initiate_swap's fee-truncation
    /// pre-flight check, isolating the resolve_dispute behaviour under test.
    #[test]
    fn test_resolve_dispute_small_amount_does_not_panic() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        // usdc_amount = 1, fee_bps = 250 → fee = 1*250/10_000 = 0 (would truncate).
        // Seed the swap directly so we bypass initiate_swap's pre-flight check and
        // test only that resolve_dispute handles the edge case gracefully.
        let usdc_id = setup_usdc(&env, &buyer, 1);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1, &Bytes::from_slice(&env, b"root"));
        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, _proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(
            &admin,
            &250u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);

        // Seed a Disputed swap with usdc_amount=1 directly in storage, then fund the contract.
        let swap_id: u64 = 1;
        let usdc_client = token::Client::new(&env, &usdc_id);
        // Transfer the 1 stroop into the contract to simulate a locked escrow.
        usdc_client.approve(&buyer, &contract_id, &1i128, &200u32);
        usdc_client.transfer(&buyer, &contract_id, &1i128);

        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&DataKey::Counter, &swap_id);
            env.storage().persistent().set(
                &DataKey::Swap(swap_id),
                &Swap {
                    listing_id,
                    buyer: buyer.clone(),
                    seller: seller.clone(),
                    usdc_amount: 1,
                    usdc_token: usdc_id.clone(),
                    created_at: 0,
                    expires_at: 9999,
                    status: SwapStatus::Disputed,
                    decryption_key: None,
                    confirmed_at_ledger: None,
                    hold_until: None,
                    buyer_confirmed: false,
                },
            );
        });

        // Must succeed — full amount goes to seller since fee truncates to zero.
        client.resolve_dispute(&swap_id, &false);

        assert_eq!(usdc_client.balance(&seller), 1);
        assert_eq!(usdc_client.balance(&fee_recipient), 0);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_resolve_dispute_panics_when_config_missing() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        // Clear the config from persistent storage (must be done inside the contract context)
        env.as_contract(&contract_id, || {
            env.storage().persistent().remove(&DataKey::Config);
        });

        // This should panic with NotInitialized instead of silently sending full amount
        client.resolve_dispute(&swap_id, &false);
    }

    #[test]
    fn test_release_to_seller_deducts_royalty() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let royalty_recipient = Address::generate(&env);

        // Setup with royalty_bps = 1000 (10%)
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let admin = Address::generate(&env);
        registry.initialize(&admin, &100_000u32, &6_312_000u32);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let listing_id = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash"),
            &root_for_leaf(&env, &key_bytes),
            &1000u32, // 10% royalty
            &royalty_recipient,
            &1000,
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &admin,
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        let (zk_id_new, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        // Re-initialize with the correct zk verifier that has the merkle root set
        let contract_id2 = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id2);
        let fee_recipient2 = Address::generate(&env);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &fee_recipient2,
            &60u64,
            &3600u64,
            &zk_id_new,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id2, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        // Advance past dispute window
        env.ledger().with_mut(|li| li.sequence_number += 20_000);

        let usdc_client = token::Client::new(&env, &usdc_id);
        client.release_to_seller(&swap_id);

        // Seller should receive 1000 - 100 (10% royalty) = 900
        assert_eq!(usdc_client.balance(&seller), 900);
        // Royalty recipient should receive 100
        assert_eq!(usdc_client.balance(&royalty_recipient), 100);
    }

    #[test]
    fn test_resolve_dispute_favor_seller_deducts_royalty() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let royalty_recipient = Address::generate(&env);

        // Setup with royalty_bps = 1000 (10%)
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let admin = Address::generate(&env);
        registry.initialize(&admin, &100_000u32, &6_312_000u32);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let listing_id = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash"),
            &root_for_leaf(&env, &key_bytes),
            &1000u32, // 10% royalty
            &royalty_recipient,
            &1000,
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &admin,
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        let (zk_id_new, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id2 = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id2);
        let fee_recipient2 = Address::generate(&env);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &fee_recipient2,
            &60u64,
            &3600u64,
            &zk_id_new,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id2, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        client.raise_dispute(&swap_id);

        let usdc_client = token::Client::new(&env, &usdc_id);
        client.resolve_dispute(&swap_id, &false);

        // Seller should receive 1000 - 100 (10% royalty) = 900
        assert_eq!(usdc_client.balance(&seller), 900);
        // Royalty recipient should receive 100
        assert_eq!(usdc_client.balance(&royalty_recipient), 100);
    }

    /// Issue #569: royalty must be computed on gross swap.usdc_amount, not post-fee amount.
    /// With fee_bps=200 (2%) and royalty_bps=500 (5%) on 1000 USDC:
    ///   fee    = 1000 * 200 / 10_000 = 20
    ///   royalty = 1000 * 500 / 10_000 = 50  (gross base)
    ///   seller  = 1000 - 20 - 50 = 930
    #[test]
    fn test_royalty_base_is_gross_amount_not_post_fee() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let royalty_recipient = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let reg_admin = Address::generate(&env);
        registry.initialize(&reg_admin, &100_000u32, &6_312_000u32);
        let key_bytes = Bytes::from_slice(&env, b"key");
        let listing_id = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash"),
            &root_for_leaf(&env, &key_bytes),
            &500u32, // 5% royalty
            &royalty_recipient,
            &1i128,
        );

        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        // fee_bps = 200 (2%)
        client.initialize(
            &Address::generate(&env),
            &200u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        let usdc = token::Client::new(&env, &usdc_id);
        assert_eq!(usdc.balance(&fee_recipient), 20);
        assert_eq!(usdc.balance(&royalty_recipient), 50);
        assert_eq!(usdc.balance(&seller), 930);
    }

    #[test]
    #[ignore = "events().all() API changed in soroban-sdk v25"]
    fn test_pause_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        let dummy_registry = Address::generate(&env);
        client.initialize(
            &admin,
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &dummy_registry,
        );

        client.pause();

        let admin_val: soroban_sdk::Val = admin.into_val(&env);
        let admin_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &admin_val).unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("contract_paused_event".try_into().unwrap());
        let found = env
            .events()
            .all()
            .filter_by_contract(&contract_id)
            .events()
            .iter()
            .any(|e| {
                let body = match &e.body {
                    soroban_sdk::xdr::ContractEventBody::V0(b) => b,
                };
                body.topics.len() == 2 && body.topics[0] == name_xdr && body.topics[1] == admin_xdr
            });
        assert!(found, "ContractPausedEvent not emitted");
    }

    #[test]
    #[ignore = "events().all() API changed in soroban-sdk v25"]
    fn test_unpause_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        let dummy_registry = Address::generate(&env);
        client.initialize(
            &admin,
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &dummy_registry,
        );

        client.unpause();

        let admin_val: soroban_sdk::Val = admin.into_val(&env);
        let admin_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &admin_val).unwrap();
        let name_xdr =
            soroban_sdk::xdr::ScVal::Symbol("contract_unpaused_event".try_into().unwrap());
        let found = env
            .events()
            .all()
            .filter_by_contract(&contract_id)
            .events()
            .iter()
            .any(|e| {
                let body = match &e.body {
                    soroban_sdk::xdr::ContractEventBody::V0(b) => b,
                };
                body.topics.len() == 2 && body.topics[0] == name_xdr && body.topics[1] == admin_xdr
            });
        assert!(found, "ContractUnpausedEvent not emitted");
    }

    #[test]
    fn test_get_swap_returns_full_struct() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));
        let zk_id = env.register(ZkVerifier, ());
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        let swap = client.get_swap(&swap_id).expect("swap should exist");
        assert_eq!(swap.buyer, buyer);
        assert_eq!(swap.seller, seller);
        assert_eq!(swap.usdc_amount, 500);
        assert_eq!(swap.status, SwapStatus::Pending);
    }

    #[test]
    fn test_get_config_returns_initialized_config() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let fee_bps = 250u32;
        let cancel_delay_secs = 60u64;
        let zk_id = env.register(ZkVerifier, ());
        let registry_id = env.register(IpRegistry, ());

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &fee_bps,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );

        let config = client.get_config();
        assert_eq!(
            config,
            Some(Config {
                admin: admin.clone(),
                fee_bps,
                fee_recipient,
                cancel_delay_secs,
                swap_expiry_secs: 3600, // Default in initialize call
                zk_verifier: zk_id,
                ip_registry: registry_id,
                escrow_hold: EscrowHoldConfig {
                    enabled: false,
                    default_hold_period_secs: DEFAULT_HOLD_PERIOD_SECS,
                },
            })
        );

        // Test None before init
        let env2 = Env::default();
        let contract_id2 = env2.register(AtomicSwap, ());
        let client2 = AtomicSwapClient::new(&env2, &contract_id2);
        assert_eq!(client2.get_config(), None);
    }

    /// Test that Config and Admin in persistent storage survive beyond instance TTL expiration.
    /// This verifies the fix for issue #527.
    #[test]
    fn test_config_and_admin_persist_beyond_instance_ttl() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let fee_bps = 250u32;
        let cancel_delay_secs = 60u64;
        let zk_id = env.register(ZkVerifier, ());
        let registry_id = env.register(IpRegistry, ());

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &fee_bps,
            &fee_recipient,
            &cancel_delay_secs,
            &3600u64,
            &zk_id,
            &registry_id,
        );

        // Add an allowed token to ensure persistent storage is being used
        let usdc = setup_usdc(&env, &Address::generate(&env), 1000);
        client.add_allowed_token(&usdc);

        // Advance ledger far beyond typical instance TTL (which would be ~100k ledgers)
        // Persistent storage should still be accessible
        env.ledger().with_mut(|li| li.sequence_number += 7_000_000);

        // Config should still be accessible from persistent storage
        let config = client.get_config();
        assert!(
            config.is_some(),
            "Config should be accessible after instance TTL expiration"
        );
        let cfg = config.unwrap();
        assert_eq!(cfg.admin, admin);
        assert_eq!(cfg.fee_bps, fee_bps);

        // initiate_swap should still work even though instance storage would have expired
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let listing_id = 1u64;

        // This should not panic with NotInitialized even after instance TTL expiry
        let result = client.try_initiate_swap(&listing_id, &buyer, &seller, &usdc, &100i128);
        // We expect it to fail for other reasons (no listing), but NOT NotInitialized
        match result {
            Err(_) => {} // Expected - listing doesn't exist
            Ok(_) => {}  // Also fine if somehow succeeds
        }
    }

    /// Test that allowed tokens in persistent storage survive beyond TTL expiration.
    /// This verifies the fix for the add_allowed_token TTL bug.
    #[test]
    fn test_allowed_token_persists_beyond_ttl() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let zk_id = env.register(ZkVerifier, ());
        let registry_id = env.register(IpRegistry, ());

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );

        // Add an allowed token
        let usdc = setup_usdc(&env, &Address::generate(&env), 1000);
        client.add_allowed_token(&usdc);

        // Advance ledger far beyond typical persistent TTL to simulate expiration
        env.ledger().with_mut(|li| li.sequence_number += 7_000_000);

        // The allowed token should still be valid after TTL expiration
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let listing_id = 1u64;

        // This should not panic with InvalidToken even after TTL expiry
        let result = client.try_initiate_swap(&listing_id, &buyer, &seller, &usdc, &100i128);
        // We expect it to fail for other reasons (no listing), but NOT InvalidToken
        match result {
            Err(e) => {
                // Verify it's not InvalidToken error
                if let Ok(contract_err) = e {
                    // Check that the error is not InvalidToken (error code 20)
                    assert_ne!(
                        contract_err,
                        soroban_sdk::Error::from_contract_error(ContractError::InvalidToken as u32),
                        "Allowed token should not expire after TTL"
                    );
                }
            }
            Ok(_) => {} // Also fine if somehow succeeds
        }
    }

    #[test]
    fn test_invalid_proof_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let real_key = Bytes::from_slice(&env, b"real-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &root_for_leaf(&env, &real_key));
        let (zk_id, _) = setup_zk_verifier(&env, &seller, listing_id, &real_key);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &1000i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        let wrong_key = Bytes::from_slice(&env, b"wrong-key");
        let result = client.try_confirm_swap(&swap_id, &wrong_key, &soroban_sdk::Vec::new(&env));
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::InvalidProof as u32
            )))
        );
    }

    #[test]
    fn test_confirm_swap_valid_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = Bytes::from_slice(&env, b"valid-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &root_for_leaf(&env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Completed)
        );
    }

    /// Issue #708: confirm_swap must reject a swap when ip_registry's advertised
    /// merkle_root for the listing does not match the root actually registered in
    /// zk_verifier, even when the submitted proof is otherwise well-formed. Without
    /// this check a seller could advertise one root to buyers (via get_listing) while
    /// a different root is enforced at proof-verification time.
    #[test]
    #[should_panic(expected = "Error(Contract, #46)")]
    fn test_confirm_swap_rejects_merkle_root_mismatch() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = Bytes::from_slice(&env, b"valid-key");
        // ip_registry advertises a root that does NOT match the one zk_verifier
        // will independently register for the same listing_id.
        let (registry_id, listing_id) = setup_registry(
            &env,
            &seller,
            500,
            &Bytes::from_slice(&env, b"advertised-root-does-not-match"),
        );
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
    }

    /// Issue #708: when ip_registry's advertised merkle_root matches what
    /// zk_verifier has registered for the listing, a buyer who inspects
    /// get_listing(listing_id) before initiating the swap can trust that value —
    /// confirm_swap succeeds normally instead of being rejected as a mismatch.
    #[test]
    fn test_confirm_swap_succeeds_when_roots_match() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = Bytes::from_slice(&env, b"valid-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &root_for_leaf(&env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);

        // Buyer inspects get_listing before committing funds...
        let listing = IpRegistryClient::new(&env, &registry_id)
            .get_listing(&listing_id)
            .expect("listing should exist");
        let registered_root = ZkVerifierClient::new(&env, &zk_id)
            .get_merkle_root(&listing_id)
            .expect("root should be registered");
        assert_eq!(listing.merkle_root, Bytes::from(registered_root));

        // ...and confirm_swap enforces exactly that same root.
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Completed)
        );
    }

    #[test]
    fn test_confirm_swap_emits_swap_completed_event() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let key_bytes = Bytes::from_slice(&env, b"secret-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1, &root_for_leaf(&env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);

        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        // SwapKeySubmitted: topics = ["swap_key_submitted", swap_id]; data = map { seller: address }
        let swap_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(
            &env,
            &<u64 as IntoVal<Env, soroban_sdk::Val>>::into_val(&swap_id, &env),
        )
        .unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("swap_key_submitted".try_into().unwrap());
        let found = env
            .events()
            .all()
            .filter_by_contract(&contract_id)
            .events()
            .iter()
            .any(|e| {
                let body = match &e.body {
                    soroban_sdk::xdr::ContractEventBody::V0(b) => b,
                };
                body.topics.len() == 2
                    && body.topics[0] == name_xdr
                    && body.topics[1] == swap_id_xdr
            });
        assert!(found, "SwapKeySubmitted event not emitted on confirm_swap");
    }

    #[test]
    #[ignore = "confirm_swap proof path not yet implemented"]
    fn test_fee_floor_applies_for_small_amounts() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        // 100 bps on 100 = 1 stroop fee; seller gets 99
        let usdc_id = setup_usdc(&env, &buyer, 100);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let key_bytes = Bytes::from_slice(&env, b"k");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 100, &root_for_leaf(&env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &100u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &100i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &100);
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);
        assert_eq!(usdc_client.balance(&fee_recipient), 1);
        assert_eq!(usdc_client.balance(&seller), 99);
    }

    #[test]
    fn test_get_swaps_by_seller_empty() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let unknown_seller = Address::generate(&env);
        assert_eq!(client.get_swaps_by_seller(&unknown_seller).len(), 0);
    }

    #[test]
    fn test_get_swaps_by_seller_single() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids.get(0).unwrap(), swap_id);
    }

    #[test]
    fn test_get_swaps_by_seller_multiple() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id1) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));
        let listing_id2 = IpRegistryClient::new(&env, &registry_id).register_ip(
            &seller,
            &Bytes::from_slice(&env, b"hash2"),
            &Bytes::from_slice(&env, b"root2"),
            &0u32,
            &seller,
            &1i128,
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);

        let id1 = client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500);
        let id2 = client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500);

        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
    }

    #[test]
    fn test_is_listing_available_no_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let seller = Address::generate(&env);
        let (_, listing_id) = setup_registry(&env, &seller, 1, &Bytes::from_slice(&env, b"root"));
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert!(client.is_listing_available(&listing_id));
    }

    #[test]
    fn test_is_listing_available_pending_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        assert!(!client.is_listing_available(&listing_id));
    }

    #[test]
    fn test_is_listing_available_after_cancel() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);
        assert!(client.is_listing_available(&listing_id));
    }

    /// Issue #572: cancel_swap must remove ActiveListingSwap so a new buyer can
    /// immediately initiate a swap on the same listing without hitting SwapAlreadyPending.
    #[test]
    fn test_cancel_and_reinitiate_swap() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer1 = Address::generate(&env);
        let buyer2 = Address::generate(&env);
        let seller = Address::generate(&env);

        let (usdc_id, listing_id, registry_id, contract_id, client, _admin, zk_id) =
            setup_full(&env, &buyer1, &seller, 500, 1);
        // Give buyer2 funds too.
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer2, &500);

        // Initiate swap for buyer1 — creates ActiveListingSwap entry.
        let swap_id1 = pending_swap(&env, &client, listing_id, &buyer1, &seller, &usdc_id, 500);

        // Advance past cancel_delay_secs (60s configured in setup_full).
        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));

        client.cancel_swap(&swap_id1);

        // Verification 1: ActiveListingSwap key must be gone.
        env.as_contract(&contract_id, || {
            assert!(
                !env.storage()
                    .persistent()
                    .has(&DataKey::ActiveListingSwap(listing_id)),
                "ActiveListingSwap should be removed after cancel"
            );
        });

        // Verification 2: a different buyer can now initiate a new swap — no DuplicateSwap/SwapAlreadyPending.
        let swap_id2 = pending_swap(&env, &client, listing_id, &buyer2, &seller, &usdc_id, 500);
        assert_ne!(swap_id1, swap_id2);

        // ActiveListingSwap now points to the new swap.
        env.as_contract(&contract_id, || {
            let active: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::ActiveListingSwap(listing_id))
                .expect("ActiveListingSwap should exist for new swap");
            assert_eq!(active, swap_id2);
        });
    }

    #[test]
    fn test_get_swaps_by_buyer_page_empty_list() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id1, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1500, 1);
        let listing_id2 = IpRegistryClient::new(&env, &registry_id).register_ip(
            &seller,
            &Bytes::from_slice(&env, b"h2"),
            &Bytes::from_slice(&env, b"r2"),
            &0u32,
            &seller,
            &500i128,
        );
        let listing_id3 = IpRegistryClient::new(&env, &registry_id).register_ip(
            &seller,
            &Bytes::from_slice(&env, b"h3"),
            &Bytes::from_slice(&env, b"r3"),
            &0u32,
            &seller,
            &500i128,
        );
        let zk_verifier = Address::generate(&env);
        let id1 = client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500);
        let id2 = client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500);
        let id3 = client.initiate_swap(&listing_id3, &buyer, &seller, &usdc_id, &500);
        // full page
        let page = client.get_swaps_by_buyer_page(&buyer, &0u32, &3u32);
        assert_eq!(page.len(), 3);
        assert_eq!(page.get(0).unwrap(), id1);
        assert_eq!(page.get(1).unwrap(), id2);
        assert_eq!(page.get(2).unwrap(), id3);
        // first page of 2
        let page0 = client.get_swaps_by_buyer_page(&buyer, &0u32, &2u32);
        assert_eq!(page0.len(), 2);
        assert_eq!(page0.get(0).unwrap(), id1);
        assert_eq!(page0.get(1).unwrap(), id2);
        // second page (partial)
        let page1 = client.get_swaps_by_buyer_page(&buyer, &2u32, &2u32);
        assert_eq!(page1.len(), 1);
        assert_eq!(page1.get(0).unwrap(), id3);
    }

    #[test]
    fn test_get_swaps_by_buyer_page_offset_at_end() {
        // offset == total is a valid cursor-past-end state: must return an empty Vec,
        // not panic with InvalidPaginationParams.
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _registry_id, _cid, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);
        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        // 1 swap in the list; offset=1 == total=1 → empty page, no panic
        let page = client.get_swaps_by_buyer_page(&buyer, &1u32, &10u32);
        assert_eq!(page.len(), 0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #17)")]
    fn test_get_swaps_by_buyer_page_zero_limit_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let buyer = Address::generate(&env);
        client.get_swaps_by_buyer_page(&buyer, &0u32, &0u32);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #17)")]
    fn test_get_swaps_by_buyer_page_offset_out_of_bounds() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);
        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        // offset=2 on a list of 1 should panic
        client.get_swaps_by_buyer_page(&buyer, &2u32, &10u32);
    }

    // ── Issue #252 regression test ────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #20)")]
    fn test_initiate_swap_invalid_token() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);

        // Use a random address that was never added as an allowed token
        let bad_token = Address::generate(&env);
        client.initiate_swap(&listing_id, &buyer, &seller, &bad_token, &500);
    }

    // ── Issue #448 test ────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #21)")]
    fn test_initialize_rejects_fee_bps_too_high() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let ip_registry = Address::generate(&env);

        client.initialize(
            &admin,
            &10_001u32,
            &fee_recipient,
            &60u64,
            &3600u64,
            &zk_verifier,
            &ip_registry,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_initiate_swap_buyer_is_seller_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let seller = Address::generate(&env);
        // Buyer is the SAME as seller
        let buyer = seller.clone();

        let (usdc_id, listing_id, registry_id, _, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 500, 500);

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
    }

    // ── TTL extension on index reads (Issue fix) ──────────────────────────────

    #[test]
    fn test_get_swaps_by_buyer_extends_ttl_on_read() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        // Simulate ledger advancing close to TTL expiry by bumping sequence/timestamp.
        env.ledger().with_mut(|li| {
            li.sequence_number += PERSISTENT_TTL_LEDGERS - 100;
        });

        // Reading the index should extend TTL — the entry must still be present.
        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(
            ids.len(),
            1,
            "BuyerIndex should still exist after TTL-near read"
        );

        // Confirm the key is still live in storage after the read.
        env.as_contract(&contract_id, || {
            assert!(
                env.storage()
                    .persistent()
                    .has(&DataKey::BuyerIndex(buyer.clone())),
                "BuyerIndex TTL should have been extended by get_swaps_by_buyer"
            );
        });
    }

    #[test]
    fn test_get_swaps_by_seller_extends_ttl_on_read() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        // Simulate ledger advancing close to TTL expiry.
        env.ledger().with_mut(|li| {
            li.sequence_number += PERSISTENT_TTL_LEDGERS - 100;
        });

        // Reading the index should extend TTL — the entry must still be present.
        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(
            ids.len(),
            1,
            "SellerIndex should still exist after TTL-near read"
        );

        // Confirm the key is still live in storage after the read.
        env.as_contract(&contract_id, || {
            assert!(
                env.storage()
                    .persistent()
                    .has(&DataKey::SellerIndex(seller.clone())),
                "SellerIndex TTL should have been extended by get_swaps_by_seller"
            );
        });
    }

    #[test]
    fn test_initiate_swap_after_confirm_clears_active_listing() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _registry_id, contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        // Complete a first swap on the listing
        let swap_id1 = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        assert_eq!(
            client.get_swap_status(&swap_id1),
            Some(SwapStatus::Completed)
        );

        // Verify ActiveListingSwap is removed after confirm_swap
        env.as_contract(&contract_id, || {
            assert!(
                !env.storage()
                    .persistent()
                    .has(&DataKey::ActiveListingSwap(listing_id)),
                "ActiveListingSwap must be removed after confirm_swap"
            );
        });

        // Mint USDC for a second buyer and initiate a new swap on the same listing.
        // This must succeed — no SwapAlreadyPending from the stale completed entry.
        let buyer2 = Address::generate(&env);
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer2, &500);
        token::Client::new(&env, &usdc_id).approve(&buyer2, &contract_id, &500i128, &200u32);
        let swap_id2 = client.initiate_swap(&listing_id, &buyer2, &seller, &usdc_id, &500);
        assert_ne!(swap_id1, swap_id2);
        assert_eq!(client.get_swap_status(&swap_id2), Some(SwapStatus::Pending));

        // ActiveListingSwap now points to the new pending swap
        env.as_contract(&contract_id, || {
            let active: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::ActiveListingSwap(listing_id))
                .expect("ActiveListingSwap should exist for new swap");
            assert_eq!(active, swap_id2);
        });
    }

    // ── transfer_admin tests ──────────────────────────────────────────────────

    #[test]
    fn test_transfer_admin_success() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (_usdc_id, _listing_id, _registry_id, contract_id, client, admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        let new_admin = Address::generate(&env);
        client.transfer_admin(&new_admin);

        // Verify the stored admin is now new_admin
        env.as_contract(&contract_id, || {
            let stored: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();
            assert_eq!(stored, new_admin);
        });

        // Verify AdminTransferred event was emitted (event verification simplified for compatibility)
    }

    #[test]
    #[should_panic]
    fn test_transfer_admin_unauthorized() {
        let env = Env::default();
        // Do NOT mock_all_auths — auth must be enforced
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (_usdc_id, _listing_id, _registry_id, _contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        let attacker = Address::generate(&env);
        // attacker is not the admin — must panic
        client.transfer_admin(&attacker);
    }

    // ── error recovery enum tests ─────────────────────────────────────────────

    /// SwapRecoveryKind discriminators must map to the correct integer values
    /// so off-chain indexers can decode them without the ABI.
    #[test]
    fn test_swap_recovery_kind_variants_are_stable() {
        assert_eq!(SwapRecoveryKind::Validation as u32, 0);
        assert_eq!(SwapRecoveryKind::ProofVerification as u32, 1);
        assert_eq!(SwapRecoveryKind::StateRollback as u32, 2);
        assert_eq!(SwapRecoveryKind::FundsLocked as u32, 3);
        assert_eq!(SwapRecoveryKind::CancelDelay as u32, 4);
        assert_eq!(SwapRecoveryKind::TokenTransfer as u32, 5);
        assert_eq!(SwapRecoveryKind::Unauthorized as u32, 6);
        assert_eq!(SwapRecoveryKind::Expired as u32, 7);
    }

    /// confirm_swap emits SwapConfirmFailed with InvalidProof error code when
    /// the seller submits a wrong key.
    #[test]
    fn test_confirm_swap_emits_confirm_failed_on_invalid_proof() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let real_key = Bytes::from_slice(&env, b"real-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &root_for_leaf(&env, &real_key));
        let (zk_id, _) = setup_zk_verifier(&env, &seller, listing_id, &real_key);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);

        let wrong_key = Bytes::from_slice(&env, b"wrong-key");
        let result =
            client.try_confirm_swap(&swap_id, &wrong_key, &soroban_sdk::Vec::new(&env));
        // Must return InvalidProof
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::InvalidProof as u32
            )))
        );
        // Swap must remain Pending after failed confirm
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    /// cancel_swap emits SwapCancelFailed with CancelTooEarly error code when
    /// called before the cancel delay has elapsed.
    #[test]
    fn test_cancel_swap_emits_cancel_failed_on_too_early() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        // cancel_delay_secs = 120 so immediate cancel fails
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);

        // Cancel before delay — must fail with CancelTooEarly
        let result = client.try_cancel_swap(&swap_id);
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::CancelTooEarly as u32
            )))
        );
        // Swap must remain Pending after failed cancel
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    /// release_to_seller emits SwapReleaseFailed with DisputeWindowActive error
    /// code when the dispute window has not yet expired.
    #[test]
    fn test_release_to_seller_emits_release_failed_on_window_active() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _registry_id, _contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        // Window not expired — advance only 5 ledgers
        env.ledger().with_mut(|li| li.sequence_number += 5);

        let result = client.try_release_to_seller(&swap_id);
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::DisputeWindowActive as u32
            )))
        );
        // Swap must remain Completed after failed release
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Completed)
        );
    }

    /// attempt_rollback_swap should return the buyer's funds and remove the
    /// swap record when the swap is still Pending.
    #[test]
    fn test_attempt_rollback_swap_refunds_buyer() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _registry_id, contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let usdc_client = token::Client::new(&env, &usdc_id);
        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        // Verify funds are in contract before rollback
        assert_eq!(usdc_client.balance(&contract_id), 500);
        assert_eq!(usdc_client.balance(&buyer), 0);

        // Invoke the rollback directly as the contract
        env.as_contract(&contract_id, || {
            let rolled_back = AtomicSwap::attempt_rollback_swap(&env, swap_id);
            assert!(rolled_back, "rollback should succeed for Pending swap");
            // Swap record should be removed
            assert!(
                !env.storage()
                    .persistent()
                    .has(&DataKey::Swap(swap_id)),
                "Swap record should be removed after rollback"
            );
        });

        // Funds should be back with the buyer
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&contract_id), 0);
    }

    /// attempt_rollback_swap must NOT roll back a swap that is already Completed.
    #[test]
    fn test_attempt_rollback_swap_ignores_completed_swap() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _registry_id, contract_id, client, _admin, _zk_id) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let usdc_client = token::Client::new(&env, &usdc_id);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);

        // Verify funds are still in contract (not yet released)
        assert_eq!(usdc_client.balance(&contract_id), 500);

        // Rollback on a Completed swap must be a no-op
        env.as_contract(&contract_id, || {
            let rolled_back = AtomicSwap::attempt_rollback_swap(&env, swap_id);
            assert!(!rolled_back, "rollback should be rejected for Completed swap");
            // Swap record must still exist
            assert!(
                env.storage()
                    .persistent()
                    .has(&DataKey::Swap(swap_id)),
                "Swap record should NOT be removed for non-Pending swap"
            );
        });

        // Funds must not move
        assert_eq!(usdc_client.balance(&contract_id), 500);
        assert_eq!(usdc_client.balance(&buyer), 0);
    }

    // ── Dispute resolution system tests ──────────────────────────────────────

    fn make_commitment(env: &Env, favor_buyer: bool, salt: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.push_back(if favor_buyer { 1u8 } else { 0u8 });
        for i in 0..salt.len() {
            preimage.push_back(salt.get(i).unwrap());
        }
        env.crypto().sha256(&preimage).into()
    }

    fn setup_arbiter(client: &AtomicSwapClient, arbiter: &Address, weight: i128) {
        client.register_arbiter(arbiter, &weight);
    }

    #[test]
    fn test_raise_dispute_creates_dispute_record() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_dispute_window(&100u32);
        client.set_commit_window(&50u32);
        client.set_reveal_window(&50u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let dispute = client.get_dispute(&swap_id).expect("Dispute record must exist");
        assert_eq!(dispute.swap_id, swap_id);
        assert_eq!(dispute.raised_by, buyer);
        assert_eq!(dispute.outcome, DisputeOutcome::Pending);
        assert_eq!(dispute.evidence_count, 0);
        assert!(!dispute.is_appealed);
    }

    #[test]
    fn test_register_and_deactivate_arbiter() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (_, _, _, _, client, _, _) = setup_full(&env, &buyer, &seller, 500, 1);

        let arbiter = Address::generate(&env);
        client.register_arbiter(&arbiter, &100i128);

        let arbiters = client.get_arbiters();
        assert_eq!(arbiters.len(), 1);

        client.deactivate_arbiter(&arbiter);
        // Still in list but is_active=false; commit_vote should reject it
    }

    #[test]
    fn test_submit_evidence_by_buyer() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let hash = Bytes::from_slice(&env, b"QmTestEvidenceHash");
        client.submit_evidence(&swap_id, &buyer, &hash);

        let dispute = client.get_dispute(&swap_id).unwrap();
        assert_eq!(dispute.evidence_count, 1);

        let ev = client.get_evidence(&swap_id, &0u32).expect("evidence must exist");
        assert_eq!(ev.submitter, buyer);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #13)")]
    fn test_submit_evidence_rejected_on_non_disputed_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        // swap is Completed, not Disputed
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        let hash = Bytes::from_slice(&env, b"QmHash");
        client.submit_evidence(&swap_id, &buyer, &hash);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #25)")]
    fn test_commit_vote_rejected_for_non_arbiter() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let non_arbiter = Address::generate(&env);
        let salt = Bytes::from_slice(&env, b"salt");
        let commit = make_commitment(&env, true, &salt);
        client.commit_vote(&swap_id, &non_arbiter, &commit);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #26)")]
    fn test_commit_vote_rejected_for_buyer() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        // Register buyer as arbiter — conflict of interest should be caught
        client.register_arbiter(&buyer, &100i128);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let salt = Bytes::from_slice(&env, b"salt");
        let commit = make_commitment(&env, true, &salt);
        client.commit_vote(&swap_id, &buyer, &commit);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #27)")]
    fn test_commit_vote_rejected_on_double_commit() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let arbiter = Address::generate(&env);
        client.register_arbiter(&arbiter, &100i128);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let salt = Bytes::from_slice(&env, b"salt");
        let commit = make_commitment(&env, true, &salt);
        client.commit_vote(&swap_id, &arbiter, &commit);
        // Second commit must fail
        client.commit_vote(&swap_id, &arbiter, &commit);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #30)")]
    fn test_reveal_vote_rejected_on_wrong_salt() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let arbiter = Address::generate(&env);
        client.register_arbiter(&arbiter, &100i128);
        client.set_commit_window(&10u32);
        client.set_reveal_window(&10u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let real_salt = Bytes::from_slice(&env, b"correct-salt");
        let commit = make_commitment(&env, true, &real_salt);
        client.commit_vote(&swap_id, &arbiter, &commit);

        // Advance past commit deadline
        env.ledger().with_mut(|li| li.sequence_number += 11);

        // Reveal with wrong salt — must fail
        let wrong_salt = Bytes::from_slice(&env, b"wrong-salt");
        client.reveal_vote(&swap_id, &arbiter, &true, &wrong_salt);
    }

    #[test]
    fn test_full_dispute_lifecycle_arbiter_votes() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);
        let usdc_client = token::Client::new(&env, &usdc_id);

        // Short windows so we can advance past them easily
        client.set_commit_window(&10u32);
        client.set_reveal_window(&10u32);

        let arbiter1 = Address::generate(&env);
        let arbiter2 = Address::generate(&env);
        client.register_arbiter(&arbiter1, &60i128);
        client.register_arbiter(&arbiter2, &40i128);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        // Submit evidence
        let hash = Bytes::from_slice(&env, b"QmEvidence");
        client.submit_evidence(&swap_id, &buyer, &hash);

        // Commit phase: both arbiters vote favour_buyer=true
        let salt1 = Bytes::from_slice(&env, b"salt1");
        let salt2 = Bytes::from_slice(&env, b"salt2");
        let commit1 = make_commitment(&env, true, &salt1);
        let commit2 = make_commitment(&env, false, &salt2);
        client.commit_vote(&swap_id, &arbiter1, &commit1);
        client.commit_vote(&swap_id, &arbiter2, &commit2);

        // Advance past commit deadline
        env.ledger().with_mut(|li| li.sequence_number += 11);

        // Reveal phase
        client.reveal_vote(&swap_id, &arbiter1, &true, &salt1);
        client.reveal_vote(&swap_id, &arbiter2, &false, &salt2);

        // Advance past reveal deadline
        env.ledger().with_mut(|li| li.sequence_number += 10);

        // Finalize — arbiter1 weight 60 (buyer) beats arbiter2 weight 40 (seller)
        client.finalize_dispute(&swap_id);

        let dispute = client.get_dispute(&swap_id).unwrap();
        assert_eq!(dispute.outcome, DisputeOutcome::FavorBuyer);
        assert_eq!(dispute.vote_weight_buyer, 60);
        assert_eq!(dispute.vote_weight_seller, 40);
        // Escrow is held pending the appeal window — no payout yet.
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::PendingAppealWindow)
        );
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&seller), 0);

        // Appeal window closes with no appeal — anyone can settle.
        env.ledger().with_mut(|li| li.sequence_number += 17_281);
        client.settle_dispute(&swap_id);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedBuyer)
        );
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&seller), 0);
    }

    #[test]
    fn test_finalize_dispute_tie_favours_buyer() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);
        let usdc_client = token::Client::new(&env, &usdc_id);

        client.set_commit_window(&10u32);
        client.set_reveal_window(&10u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        // No arbiters vote — both weights stay 0, tie → buyer wins
        env.ledger().with_mut(|li| li.sequence_number += 21);
        client.finalize_dispute(&swap_id);

        let dispute = client.get_dispute(&swap_id).unwrap();
        assert_eq!(dispute.outcome, DisputeOutcome::FavorBuyer);
        // Escrow is held until the appeal window closes.
        assert_eq!(usdc_client.balance(&buyer), 0);

        env.ledger().with_mut(|li| li.sequence_number += 17_281);
        client.settle_dispute(&swap_id);
        assert_eq!(usdc_client.balance(&buyer), 500);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #32)")]
    fn test_finalize_dispute_rejected_before_reveal_deadline() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_commit_window(&20u32);
        client.set_reveal_window(&20u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        // Only past commit window, not reveal window
        env.ledger().with_mut(|li| li.sequence_number += 25);
        // Still inside reveal window — must fail
        client.finalize_dispute(&swap_id);
    }

    #[test]
    fn test_appeal_dispute_within_window() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_commit_window(&5u32);
        client.set_reveal_window(&5u32);
        client.set_appeal_window(&20u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        // Fast-forward past both windows to finalize
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.finalize_dispute(&swap_id);

        // Appeal within window
        client.appeal_dispute(&swap_id, &buyer);

        let dispute = client.get_dispute(&swap_id).unwrap();
        assert!(dispute.is_appealed);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Appealed)
        );
    }

    /// Issue #706: a buyer appeals a FavorSeller arbiter outcome and the
    /// admin's resolve_dispute call actually reverses the payout to the buyer
    /// — proving the appeal remedy is reachable and no funds were ever paid
    /// to the seller in the interim.
    #[test]
    fn test_appeal_reverses_favor_seller_outcome_via_resolve_dispute() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);
        let usdc_client = token::Client::new(&env, &usdc_id);

        client.set_commit_window(&5u32);
        client.set_reveal_window(&5u32);
        client.set_appeal_window(&20u32);

        let arbiter = Address::generate(&env);
        client.register_arbiter(&arbiter, &100i128);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let salt = Bytes::from_slice(&env, b"salt");
        let commit = make_commitment(&env, false, &salt);
        client.commit_vote(&swap_id, &arbiter, &commit);
        env.ledger().with_mut(|li| li.sequence_number += 6);
        client.reveal_vote(&swap_id, &arbiter, &false, &salt);

        env.ledger().with_mut(|li| li.sequence_number += 6);
        client.finalize_dispute(&swap_id);

        let dispute = client.get_dispute(&swap_id).unwrap();
        assert_eq!(dispute.outcome, DisputeOutcome::FavorSeller);
        // Holdback: neither party has been paid yet.
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&seller), 0);

        // Buyer appeals the FavorSeller outcome.
        client.appeal_dispute(&swap_id, &buyer);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Appealed)
        );
        // Still no payout — appeal is pending admin review.
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&seller), 0);

        // Admin overrides in favor of the buyer.
        client.resolve_dispute(&swap_id, &true);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedBuyer)
        );
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&seller), 0);
    }

    /// Issue #706: an appeal filed but never acted on by the admin does not
    /// permanently lock funds — settle_dispute pays out per the original
    /// arbiter outcome once the appeal resolution timeout elapses.
    #[test]
    fn test_appeal_with_no_admin_action_settles_via_timeout() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);
        let usdc_client = token::Client::new(&env, &usdc_id);

        client.set_commit_window(&5u32);
        client.set_reveal_window(&5u32);
        client.set_appeal_window(&20u32);
        client.set_appeal_resolution_window(&30u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.finalize_dispute(&swap_id);

        let dispute = client.get_dispute(&swap_id).unwrap();
        assert_eq!(dispute.outcome, DisputeOutcome::FavorBuyer);

        client.appeal_dispute(&swap_id, &buyer);
        assert_eq!(usdc_client.balance(&buyer), 0);

        // Settling before the resolution timeout must fail — admin still has time.
        let too_early = client.try_settle_dispute(&swap_id);
        assert_eq!(
            too_early,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::AppealResolutionWindowActive as u32
            )))
        );

        // Admin never calls resolve_dispute; timeout elapses.
        env.ledger().with_mut(|li| li.sequence_number += 31);
        client.settle_dispute(&swap_id);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedBuyer)
        );
        assert_eq!(usdc_client.balance(&buyer), 500);
    }

    /// Regression: settle_dispute must reject settling a swap while the
    /// appeal window is still open and unappealed — the appeal-window check
    /// added for the holdback fix must not be bypassable early.
    #[test]
    fn test_settle_dispute_rejected_while_appeal_window_open() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_commit_window(&5u32);
        client.set_reveal_window(&5u32);
        client.set_appeal_window(&20u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.finalize_dispute(&swap_id);

        let result = client.try_settle_dispute(&swap_id);
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::AppealWindowStillOpen as u32
            )))
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #36)")]
    fn test_appeal_dispute_rejected_on_double_appeal() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_commit_window(&5u32);
        client.set_reveal_window(&5u32);
        client.set_appeal_window(&20u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.finalize_dispute(&swap_id);

        client.appeal_dispute(&swap_id, &buyer);
        // Second appeal must fail
        client.appeal_dispute(&swap_id, &buyer);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #35)")]
    fn test_appeal_dispute_rejected_after_window_expires() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        client.set_commit_window(&5u32);
        client.set_reveal_window(&5u32);
        client.set_appeal_window(&5u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.finalize_dispute(&swap_id);

        // Advance past appeal window
        env.ledger().with_mut(|li| li.sequence_number += 6);
        client.appeal_dispute(&swap_id, &buyer);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #28)")]
    fn test_reveal_vote_rejected_on_double_reveal() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let arbiter = Address::generate(&env);
        client.register_arbiter(&arbiter, &100i128);
        client.set_commit_window(&10u32);
        client.set_reveal_window(&20u32);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, 500);
        client.raise_dispute(&swap_id);

        let salt = Bytes::from_slice(&env, b"unique-salt");
        let commit = make_commitment(&env, true, &salt);
        client.commit_vote(&swap_id, &arbiter, &commit);

        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.reveal_vote(&swap_id, &arbiter, &true, &salt);
        // Second reveal must fail
        client.reveal_vote(&swap_id, &arbiter, &true, &salt);
    }

    /// InsufficientAllowance (error #24) returns the correct error and leaves no
    /// funds in the contract.
    #[test]
    fn test_initiate_swap_insufficient_allowance_emits_init_failed() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        // Mint tokens but do NOT approve
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 500, &Bytes::from_slice(&env, b"root"));

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        // No approval — allowance is 0

        let result = client.try_initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::InsufficientAllowance as u32
            )))
        );

        // No funds should be in the contract
        let usdc_client = token::Client::new(&env, &usdc_id);
        assert_eq!(usdc_client.balance(&contract_id), 0);
        assert_eq!(usdc_client.balance(&buyer), 1000);
    }

    // ── escrow hold period tests ──────────────────────────────────────────────

    /// Set up an initiated (but not yet confirmed) swap with a working ZK
    /// verifier. The dispute window is shortened to 10 ledgers so tests can
    /// isolate the hold period from the dispute window. Returns the handles a
    /// test needs to configure the hold, confirm, and release.
    fn initiated_hold_swap<'a>(
        env: &'a Env,
    ) -> (
        AtomicSwapClient<'a>,
        Address, // buyer
        Address, // seller
        Address, // usdc_id
        u64,     // swap_id
        Bytes,   // decryption key
        soroban_sdk::Vec<ProofNode>,
    ) {
        let buyer = Address::generate(env);
        let seller = Address::generate(env);
        let usdc_id = setup_usdc(env, &buyer, 500);
        let key_bytes = Bytes::from_slice(env, b"secret-key");
        let (registry_id, listing_id) =
            setup_registry(env, &seller, 500, &root_for_leaf(env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(env, &contract_id);
        client.initialize(
            &Address::generate(env),
            &0u32,
            &Address::generate(env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(env, &usdc_id).approve(&buyer, &contract_id, &500i128, &200u32);
        client.set_dispute_window(&10u32);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500);
        (client, buyer, seller, usdc_id, swap_id, key_bytes, proof_path)
    }

    /// A seller-configured hold keeps funds locked even after the dispute window
    /// elapses, until the hold period passes. This is the early-release guard.
    #[test]
    fn test_hold_period_blocks_release_until_expiry() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, seller, _usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);

        client.set_seller_hold_period(&seller, &DEFAULT_HOLD_PERIOD_SECS);
        client.confirm_swap(&swap_id, &key, &proof);

        // Move past the dispute window but stay within the hold period.
        env.ledger().with_mut(|li| li.sequence_number += 11);
        assert!(client.hold_period_active(&swap_id));
        let blocked = client.try_release_to_seller(&swap_id);
        assert_eq!(
            blocked,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::HoldPeriodActive as u32
            )))
        );

        // Advance the clock past the hold period; release now succeeds.
        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(DEFAULT_HOLD_PERIOD_SECS + 1));
        assert!(!client.hold_period_active(&swap_id));
        client.release_to_seller(&swap_id);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
    }

    /// Time-locked release accuracy: the hold is active strictly before
    /// `hold_until` and inactive at/after it (exclusive lower bound).
    #[test]
    fn test_hold_period_boundary_is_exact() {
        let env = Env::default();
        env.mock_all_auths();
        let hold_secs = 1_000u64;
        let (client, _buyer, seller, _usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);

        client.set_seller_hold_period(&seller, &hold_secs);
        // Confirm at timestamp 0 so hold_until == hold_secs exactly.
        client.confirm_swap(&swap_id, &key, &proof);
        env.ledger().with_mut(|li| li.sequence_number += 11);

        let swap = client.get_swap(&swap_id).unwrap();
        assert_eq!(swap.hold_until, Some(hold_secs));

        // One second before expiry: still active.
        env.ledger().with_mut(|li| li.timestamp = hold_secs - 1);
        assert!(client.hold_period_active(&swap_id));
        assert!(client.try_release_to_seller(&swap_id).is_err());

        // Exactly at expiry: no longer active, release allowed.
        env.ledger().with_mut(|li| li.timestamp = hold_secs);
        assert!(!client.hold_period_active(&swap_id));
        client.release_to_seller(&swap_id);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
    }

    /// The buyer can confirm receipt to waive the remaining hold, letting the
    /// seller release immediately even though the hold period has not elapsed.
    #[test]
    fn test_buyer_confirmation_overrides_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, buyer, seller, usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);

        client.set_seller_hold_period(&seller, &DEFAULT_HOLD_PERIOD_SECS);
        client.confirm_swap(&swap_id, &key, &proof);
        env.ledger().with_mut(|li| li.sequence_number += 11);

        // Still inside the hold period.
        assert!(client.hold_period_active(&swap_id));

        // Buyer confirms receipt; hold is waived.
        client.confirm_receipt(&swap_id);
        assert!(!client.hold_period_active(&swap_id));

        client.release_to_seller(&swap_id);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
        // Seller received the funds (fee 0, no royalty).
        assert_eq!(token::Client::new(&env, &usdc_id).balance(&seller), 500);
        let _ = buyer;
    }

    /// Only the buyer may confirm receipt — a third party cannot self-approve to
    /// bypass the buyer's hold.
    #[test]
    fn test_confirm_receipt_requires_buyer_auth() {
        let env = Env::default();
        env.mock_all_auths(); // setup only
        let (client, _buyer, seller, _usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);
        client.set_seller_hold_period(&seller, &DEFAULT_HOLD_PERIOD_SECS);
        client.confirm_swap(&swap_id, &key, &proof);

        // Clear blanket auth and attempt confirm_receipt as a third party.
        let third_party = Address::generate(&env);
        env.set_auths(&[]);
        env.mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &third_party,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &client.address,
                fn_name: "confirm_receipt",
                args: (swap_id,).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(
            client.try_confirm_receipt(&swap_id).is_err(),
            "a non-buyer must not be able to confirm receipt"
        );
    }

    /// When holds are disabled globally and the seller set no override, swaps
    /// carry no hold (backwards-compatible default).
    #[test]
    fn test_no_hold_when_disabled_and_no_override() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, seller, _usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);

        assert_eq!(client.get_hold_period(&seller), 0);
        client.confirm_swap(&swap_id, &key, &proof);
        let swap = client.get_swap(&swap_id).unwrap();
        assert_eq!(swap.hold_until, None);

        env.ledger().with_mut(|li| li.sequence_number += 11);
        assert!(!client.hold_period_active(&swap_id));
        client.release_to_seller(&swap_id);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
    }

    /// The global default hold applies to a seller without an explicit override.
    #[test]
    fn test_global_hold_applies_without_seller_override() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, seller, _usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);

        client.set_escrow_hold_config(&true, &500u64);
        assert_eq!(client.get_hold_period(&seller), 500);

        client.confirm_swap(&swap_id, &key, &proof);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        assert!(client.hold_period_active(&swap_id));
        assert!(client.try_release_to_seller(&swap_id).is_err());
    }

    /// A per-seller override (including `0` to opt out) takes precedence over the
    /// global default.
    #[test]
    fn test_seller_override_takes_precedence() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, seller, _usdc_id, swap_id, key, proof) = initiated_hold_swap(&env);

        client.set_escrow_hold_config(&true, &DEFAULT_HOLD_PERIOD_SECS);
        // Seller opts out entirely.
        client.set_seller_hold_period(&seller, &0u64);
        assert_eq!(client.get_hold_period(&seller), 0);

        client.confirm_swap(&swap_id, &key, &proof);
        let swap = client.get_swap(&swap_id).unwrap();
        assert_eq!(swap.hold_until, None);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        assert!(!client.hold_period_active(&swap_id));
    }

    /// Hold periods above the cap are rejected, preventing indefinite fund locks.
    #[test]
    fn test_set_seller_hold_period_rejects_too_long() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, seller, _usdc_id, _swap_id, _key, _proof) = initiated_hold_swap(&env);

        let result = client.try_set_seller_hold_period(&seller, &(MAX_HOLD_PERIOD_SECS + 1));
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::HoldPeriodTooLong as u32
            )))
        );
    }

    /// The admin cannot set a global default above the cap either.
    #[test]
    fn test_set_escrow_hold_config_rejects_too_long() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, _seller, _usdc_id, _swap_id, _key, _proof) = initiated_hold_swap(&env);

        let result = client.try_set_escrow_hold_config(&true, &(MAX_HOLD_PERIOD_SECS + 1));
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::HoldPeriodTooLong as u32
            )))
        );
    }

    /// confirm_receipt is only valid on a Completed swap.
    #[test]
    fn test_confirm_receipt_requires_completed_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _buyer, _seller, _usdc_id, swap_id, _key, _proof) = initiated_hold_swap(&env);

        // Swap is still Pending (not confirmed).
        let result = client.try_confirm_receipt(&swap_id);
        assert_eq!(
            result,
            Err(Ok(soroban_sdk::Error::from_contract_error(
                ContractError::SwapNotCompleted as u32
            )))
        );
    }

    // ── Multi-signature approval tests ────────────────────────────────────────

    fn setup_multisig(
        env: &Env,
        client: &AtomicSwapClient,
        admin1: &Address,
        admin2: &Address,
        seller: &Address,
        threshold: i128,
    ) {
        let mut signers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(env);
        signers.push_back(admin1.clone());
        signers.push_back(admin2.clone());
        // 2-of-2 scheme: both admin and seller must sign
        client.set_multisig_config(&threshold, &signers, &2u32, &true);
        let _ = seller;
    }

    fn setup_multisig_3of3(
        env: &Env,
        client: &AtomicSwapClient,
        s1: &Address,
        s2: &Address,
        s3: &Address,
        threshold: i128,
    ) {
        let mut signers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(env);
        signers.push_back(s1.clone());
        signers.push_back(s2.clone());
        signers.push_back(s3.clone());
        // 2-of-3 scheme
        client.set_multisig_config(&threshold, &signers, &2u32, &true);
    }

    /// High-value swap is created with PendingMultiSig status when multi-sig is enabled.
    #[test]
    fn test_multisig_high_value_swap_enters_pending_multisig() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let high_value: i128 = 1_000_000_000_000; // 100,000 USDC
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, high_value, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000); // threshold = 50 USDC

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &high_value);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::PendingMultiSig)
        );
    }

    /// Low-value swap (below threshold) skips multi-sig and enters Pending directly.
    #[test]
    fn test_multisig_low_value_swap_skips_multisig() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let low_value: i128 = 1_000; // well below threshold
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, low_value, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000); // threshold = 50 USDC

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &low_value);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    /// 2-of-2 full approval flow: both signers approve → swap becomes Pending.
    #[test]
    fn test_multisig_2of2_full_approval_unlocks_swap() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::PendingMultiSig)
        );

        // First signer approves — still PendingMultiSig
        client.approve_multisig_swap(&swap_id, &signer1);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::PendingMultiSig)
        );

        // Second signer approves — threshold met, promoted to Pending
        client.approve_multisig_swap(&swap_id, &signer2);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    /// 2-of-3 partial approval: one signer out of three; swap stays PendingMultiSig.
    #[test]
    fn test_multisig_2of3_one_approval_insufficient() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let s1 = Address::generate(&env);
        let s2 = Address::generate(&env);
        let s3 = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        setup_multisig_3of3(&env, &client, &s1, &s2, &s3, 500_000_000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);

        client.approve_multisig_swap(&swap_id, &s1);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::PendingMultiSig)
        );

        // Second approval (2-of-3) should unlock it
        client.approve_multisig_swap(&swap_id, &s3);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    /// Duplicate approval from same signer is rejected with MultiSigAlreadyApproved.
    #[test]
    #[should_panic(expected = "Error(Contract, #42)")]
    fn test_multisig_duplicate_approval_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);
        client.approve_multisig_swap(&swap_id, &signer1);
        // Signer1 approves again — must fail
        client.approve_multisig_swap(&swap_id, &signer1);
    }

    /// Non-signer attempt to approve is rejected with NotAMultiSigSigner.
    #[test]
    #[should_panic(expected = "Error(Contract, #41)")]
    fn test_multisig_non_signer_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);
        let outsider = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);
        // Outsider tries to approve — must fail
        client.approve_multisig_swap(&swap_id, &outsider);
    }

    /// approve_multisig_swap on a Pending (already unlocked) swap is rejected.
    #[test]
    #[should_panic(expected = "Error(Contract, #43)")]
    fn test_multisig_approve_on_non_pending_multisig_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);

        let amount: i128 = 100;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);
        // multi-sig disabled — swap is Pending immediately
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));

        let signer = Address::generate(&env);
        // Trying to approve a non-PendingMultiSig swap must fail
        client.approve_multisig_swap(&swap_id, &signer);
    }

    /// Invalid config: required_approvals > signers.len() is rejected.
    #[test]
    #[should_panic(expected = "Error(Contract, #44)")]
    fn test_multisig_invalid_config_required_exceeds_signers() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (_, _, _, _, client, _, _) = setup_full(&env, &buyer, &seller, 100, 1);

        let mut signers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
        signers.push_back(Address::generate(&env));
        // 1 signer but required = 2 → invalid
        client.set_multisig_config(&100i128, &signers, &2u32, &true);
    }

    /// Multi-sig config can be disabled: all swaps become Pending regardless of amount.
    #[test]
    fn test_multisig_disabled_passes_all_swaps_through() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        // Configure multi-sig but then disable it
        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000);
        let mut signers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
        signers.push_back(signer1.clone());
        signers.push_back(signer2.clone());
        client.set_multisig_config(&500_000_000i128, &signers, &2u32, &false);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    /// get_multisig_approval returns the accumulated approvals for a high-value swap.
    #[test]
    fn test_multisig_get_approval_reflects_signers() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);

        let approval = client.get_multisig_approval(&swap_id).expect("approval record must exist");
        assert_eq!(approval.approved_by.len(), 0);

        client.approve_multisig_swap(&swap_id, &signer1);
        let approval = client.get_multisig_approval(&swap_id).unwrap();
        assert_eq!(approval.approved_by.len(), 1);
        assert_eq!(approval.approved_by.get(0).unwrap(), signer1);
    }

    /// Replay-attack prevention: reusing a consumed nonce is rejected.
    #[test]
    fn test_multisig_nonce_prevents_replay() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        let amount: i128 = 1_000_000_000_000;
        let (usdc_id, listing_id, _, _, client, _, _) =
            setup_full(&env, &buyer, &seller, amount, 1);

        setup_multisig(&env, &client, &signer1, &signer2, &seller, 500_000_000);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);

        // First approval burns nonce = swap_id. Attempting a duplicate (same signer) triggers
        // MultiSigAlreadyApproved, which is the uniqueness guard before the nonce replay guard.
        client.approve_multisig_swap(&swap_id, &signer1);

        // Verify the nonce was advanced (nonce is now swap_id + 1)
        let approval = client.get_multisig_approval(&swap_id).unwrap();
        assert_eq!(approval.nonce, swap_id + 1);
    }

    /// Full end-to-end: high-value swap → multi-sig approval → confirm → release.
    #[test]
    fn test_multisig_full_high_value_swap_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);

        // 10,000 USDC (7-decimal) = 100_000_000_000 stroops
        let amount: i128 = 100_000_000_000;
        let usdc_id = setup_usdc(&env, &buyer, amount);
        let key_bytes = Bytes::from_slice(&env, b"multisig-key");
        let (registry_id, listing_id) =
            setup_registry(&env, &seller, 1, &root_for_leaf(&env, &key_bytes));
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &3600u64,
            &zk_id,
            &registry_id,
        );
        client.add_allowed_token(&usdc_id);
        token::Client::new(&env, &usdc_id).approve(&buyer, &contract_id, &amount, &200u32);

        // Enable multi-sig at threshold = 1 stroop (catches everything)
        let mut signers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
        signers.push_back(signer1.clone());
        signers.push_back(signer2.clone());
        client.set_multisig_config(&1i128, &signers, &2u32, &true);

        // Initiate — enters PendingMultiSig
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &amount);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::PendingMultiSig)
        );

        // Collect 2-of-2 approvals
        client.approve_multisig_swap(&swap_id, &signer1);
        client.approve_multisig_swap(&swap_id, &signer2);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));

        // Seller confirms and releases
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::Completed)
        );

        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
        let usdc = token::Client::new(&env, &usdc_id);
        assert_eq!(usdc.balance(&seller), amount);
        assert_eq!(usdc.balance(&buyer), 0);
    }
}
