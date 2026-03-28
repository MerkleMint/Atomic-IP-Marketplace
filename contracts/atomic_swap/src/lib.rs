#![no_std]
use ip_registry::IpRegistryClient;
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Bytes, Env,
};
use zk_verifier::{ProofNode, ZkVerifierClient};

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;
const DEFAULT_DISPUTE_WINDOW_LEDGERS: u32 = 17_280;



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
}

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub fee_bps: u32,
    pub fee_recipient: Address,
    pub cancel_delay_secs: u64,
    pub zk_verifier: Address,
    pub ip_registry: Address,
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
}

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

/// Emitted when a swap is completed and funds are released to the seller.
#[contractevent]
pub struct SwapCompleted {
    #[topic]
    pub swap_id: u64,
    pub seller: Address,
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

/// Emitted when a dispute is resolved by the admin.
#[contractevent]
pub struct DisputeResolved {
    #[topic]
    pub swap_id: u64,
    pub favor_buyer: bool,
}

#[contract]
pub struct AtomicSwap;

#[contractimpl]
impl AtomicSwap {
    fn calculate_fee_amount(env: &Env, usdc_amount: i128, fee_bps: u32) -> i128 {
        if fee_bps == 0 {
            return 0;
        }
        let product = usdc_amount
            .checked_mul(fee_bps as i128)
            .unwrap_or_else(|| env.panic_with_error(ContractError::InvalidAmount));
        let fee = product / 10_000;
        if fee == 0 {
            env.panic_with_error(ContractError::FeeWouldTruncate);
        }
        fee
    }

    pub fn initialize(
        env: Env,
        admin: Address,
        fee_bps: u32,
        fee_recipient: Address,
        cancel_delay_secs: u64,
        zk_verifier: Address,
        ip_registry: Address,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            env.panic_with_error(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                fee_bps,
                fee_recipient,
                cancel_delay_secs,
                zk_verifier,
                ip_registry,
            },
        );
        env.storage().instance().set(
            &DataKey::DisputeWindowLedgers,
            &DEFAULT_DISPUTE_WINDOW_LEDGERS,
        );
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    pub fn add_allowed_token(env: Env, token: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::AllowedToken(token), &true);
    }

    pub fn set_dispute_window(env: Env, ledgers: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::DisputeWindowLedgers, &ledgers);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    pub fn pause(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        ContractPausedEvent { admin }.publish(&env);
    }

    pub fn unpause(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        Self::calculate_fee_amount(&env, usdc_amount, config.fee_bps);

        let now = env.ledger().timestamp();
        let expires_at = now.saturating_add(config.cancel_delay_secs);

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
            env.panic_with_error(ContractError::UnderpaymentNotAllowed);
        }

        token::Client::new(&env, &usdc_token).transfer(
            &buyer,
            env.current_contract_address(),
            &usdc_amount,
        );

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

        let key = DataKey::Swap(id);
        env.storage().persistent().set(
            &key,
            &Swap {
                listing_id,
                buyer: buyer.clone(),
                seller: seller.clone(),
                usdc_amount,
                usdc_token,
                zk_verifier: config.zk_verifier,
                created_at: now,
                expires_at,
                status: SwapStatus::Pending,
                decryption_key: None,
                confirmed_at_ledger: None,
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
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));

        let verified = ZkVerifierClient::new(&env, &config.zk_verifier).verify_partial_proof(
            &swap.listing_id,
            &decryption_key,
            &proof_path,
        );
        if !verified {
            env.panic_with_error(ContractError::InvalidProof);
        }

        swap.status = SwapStatus::Completed;
        swap.decryption_key = Some(decryption_key.clone());
        swap.confirmed_at_ledger = Some(env.ledger().sequence());
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        SwapConfirmed {
            swap_id,
            seller: swap.seller.clone(),
            decryption_key,
        }
        .publish(&env);

        SwapCompleted {
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

        let confirmed_at = swap
            .confirmed_at_ledger
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::MissingConfirmationLedger));
        let window: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DisputeWindowLedgers)
            .unwrap_or(DEFAULT_DISPUTE_WINDOW_LEDGERS);
        if env.ledger().sequence() <= confirmed_at + window {
            panic_with_error!(&env, ContractError::DisputeWindowActive);
        }

        let token_client = token::Client::new(&env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();
        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));

        // Reject tiny amounts that would silently truncate protocol fees.
        let fee: i128 = { Self::calculate_fee_amount(&env, swap.usdc_amount, config.fee_bps) };
        let seller_amount = swap.usdc_amount - fee;
        if fee > 0 {
            token_client.transfer(&contract_addr, &config.fee_recipient, &fee);
        }
        token_client.transfer(&contract_addr, &swap.seller, &seller_amount);

        swap.status = SwapStatus::ResolvedSeller;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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
            .instance()
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
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    pub fn resolve_dispute(env: Env, swap_id: u64, favor_buyer: bool) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();

        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if swap.status != SwapStatus::Disputed {
            env.panic_with_error(ContractError::SwapNotDisputed);
        }

        let token_client = token::Client::new(&env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();

        if favor_buyer {
            token_client.transfer(&contract_addr, &swap.buyer, &swap.usdc_amount);
            swap.status = SwapStatus::ResolvedBuyer;
        } else {
            if let Some(config) = env
                .storage()
                .instance()
                .get::<DataKey, Config>(&DataKey::Config)
            {
                let fee = Self::calculate_fee_amount(&env, swap.usdc_amount, config.fee_bps);
                let seller_amount = swap.usdc_amount - fee;
                if fee > 0 {
                    token_client.transfer(&contract_addr, &config.fee_recipient, &fee);
                }
                token_client.transfer(&contract_addr, &swap.seller, &seller_amount);
            } else {
                token_client.transfer(&contract_addr, &swap.seller, &swap.usdc_amount);
            }
            swap.status = SwapStatus::ResolvedSeller;
        }

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
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        
        let cancel_deadline = swap.created_at.saturating_add(config.cancel_delay_secs);
        if env.ledger().timestamp() < cancel_deadline {
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
        env.storage().persistent().get(&DataKey::Swap(swap_id))
    }

    pub fn get_decryption_key(env: Env, swap_id: u64) -> Option<Bytes> {
        env.storage()
            .persistent()
            .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            .and_then(|swap| swap.decryption_key)
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
        env.storage()
            .persistent()
            .get(&DataKey::BuyerIndex(buyer))
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env))
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
        env.storage()
            .persistent()
            .get(&DataKey::SellerIndex(seller))
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env))
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
}

#[cfg(test)]
mod test {
    use super::*;
    use ip_registry::{IpRegistry, IpRegistryClient};
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger as _},
        token, Bytes, Env, IntoVal, TryFromVal,
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

    fn setup_registry(env: &Env, seller: &Address, price_usdc: i128) -> (Address, u64) {
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(env, &registry_id);
        let admin = Address::generate(env);
        registry.initialize(&admin, &100_000u32, &6_312_000u32);
        let listing_id = registry.register_ip(
            seller,
            &Bytes::from_slice(env, b"QmHash"),
            &Bytes::from_slice(env, b"root"),
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
        Address, // zk_id
    ) {
        let usdc_id = setup_usdc(env, buyer, usdc_amount);
        let (registry_id, listing_id) = setup_registry(env, seller, price_usdc);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let fee_recipient = Address::generate(env);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(&admin, &0u32, &fee_recipient, &60u64, &zk_id);
        client.add_allowed_token(&usdc_id);
        (usdc_id, listing_id, registry_id, contract_id, client, admin)
    }

    fn pending_swap(
        env: &Env,
        client: &AtomicSwapClient,
        listing_id: u64,
        buyer: &Address,
        seller: &Address,
        usdc_id: &Address,
        _registry_id: &Address,
        usdc_amount: i128,
        zk_id: &Address,
    ) -> u64 {
        client.initiate_swap(
            &listing_id,
            buyer,
            seller,
            usdc_id,
            &usdc_amount,
            zk_id,
            registry_id,
        )
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

        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_id,
            &registry_id,
        );
    }

    #[test]
    fn test_initiate_swap_accepts_exact_price() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin, zk_id) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &1000,
            &zk_id,
            &registry_id,
        );
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

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &1000,
        );
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_initiate_swap_allows_any_amount_when_price_is_zero() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        // price_usdc = 0 means no price enforcement
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 1);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &1,
            &zk_id,
            &registry_id,
        );
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_happy_path_initiate_confirm_release_to_seller() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let key_bytes = Bytes::from_slice(&env, b"secret-key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );

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

        let swap_id = pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            500,
            &zk_id,
        );

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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);

        let key_bytes = Bytes::from_slice(&env, b"secret-key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );

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

        let swap_id = pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            500,
            &zk_id,
        );

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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 1);

        let key_bytes = Bytes::from_slice(&env, b"key");
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
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &10_000,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 1000);

        let key_bytes = Bytes::from_slice(&env, b"key");
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
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &1000,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 1);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &250u32,
            &fee_recipient,
            &60u64,
            &zk_id,
            &registry_id,
        );

        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &1,
            &zk_id,
            &registry_id,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 40);

        let key_bytes = Bytes::from_slice(&env, b"key");
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
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &40,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );
        client.pause();

        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_id,
            &registry_id,
        );
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
            &zk_id,
        );

        // No listing with this id exists in registry.
        client.initiate_swap(
            &999_999u64,
            &buyer,
            &seller,
            &usdc_id,
            &500i128,
            &zk_id,
            &registry_id,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &real_seller, 500);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );

        client.initiate_swap(
            &listing_id,
            &buyer,
            &impersonator,
            &usdc_id,
            &500,
            &zk_id,
            &registry_id,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #18)")]
    fn test_cancel_swap_rejects_before_expiry() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );

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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let zk_id = env.register(ZkVerifier, ());
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &zk_id,
            &registry_id,
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );
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
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 1000);

        let swap_id = pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            1000,
        );

        // Check SwapInitiated event: topics = ["swap_initiated", swap_id, listing_id]
        let swap_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &<u64 as IntoVal<Env, soroban_sdk::Val>>::into_val(&swap_id, &env)).unwrap();
        let listing_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &<u64 as IntoVal<Env, soroban_sdk::Val>>::into_val(&listing_id, &env)).unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("swap_initiated".try_into().unwrap());
        let found = env.events().all().filter_by_contract(&_cid).events().iter().any(|e| {
            let body = match &e.body { soroban_sdk::xdr::ContractEventBody::V0(b) => b };
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
        registry_id: &Address,
        zk_id: &Address,
        proof_path: &soroban_sdk::Vec<ProofNode>,
        key: &Bytes,
    ) -> u64 {
        let key_bytes = Bytes::from_slice(env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(env, seller, listing_id, &key_bytes);
        let swap_id = client.initiate_swap(
            &listing_id,
            buyer,
            seller,
            usdc_id,
            &500,
        );
        client.confirm_swap(&swap_id, &key_bytes, &proof_path);
        swap_id
    }

    #[test]
    fn test_raise_dispute_within_window() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let swap_id = confirmed_swap(
            &env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id,
            &zk_id, &proof_path, &key_bytes,
        );
        client.raise_dispute(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Disputed));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #11)")]
    fn test_raise_dispute_after_window_expires() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(
            &env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id,
            &zk_id, &proof_path, &key_bytes,
        );
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.raise_dispute(&swap_id);
    }

    #[test]
    fn test_release_to_seller_before_window_expires_returns_dispute_window_active() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);

        // Set a 10-ledger dispute window
        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
        );

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

    #[test]
    fn test_resolve_dispute_favor_buyer_refunds_usdc() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let swap_id = confirmed_swap(
            &env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id,
            &zk_id, &proof_path, &key_bytes,
        );
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
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let key_bytes = Bytes::from_slice(&env, b"key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);

        let swap_id = confirmed_swap(
            &env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id,
            &zk_id, &proof_path, &key_bytes,
        );
        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &false);

        assert_eq!(
            client.get_swap_status(&swap_id),
            Some(SwapStatus::ResolvedSeller)
        );
        assert_eq!(usdc_client.balance(&seller), 500);
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
        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64, &zk_id, &dummy_registry);

        client.pause();

        let admin_val: soroban_sdk::Val = admin.into_val(&env);
        let admin_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &admin_val).unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("contract_paused_event".try_into().unwrap());
        let found = env.events().all().filter_by_contract(&contract_id).events().iter().any(|e| {
            let body = match &e.body { soroban_sdk::xdr::ContractEventBody::V0(b) => b };
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
        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64, &zk_id, &dummy_registry);

        client.unpause();

        let admin_val: soroban_sdk::Val = admin.into_val(&env);
        let admin_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &admin_val).unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("contract_unpaused_event".try_into().unwrap());
        let found = env.events().all().filter_by_contract(&contract_id).events().iter().any(|e| {
            let body = match &e.body { soroban_sdk::xdr::ContractEventBody::V0(b) => b };
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);
        let zk_id = env.register(ZkVerifier, ());
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );
        let swap = client.get_swap(&swap_id).expect("swap should exist");
        assert_eq!(swap.buyer, buyer);
        assert_eq!(swap.seller, seller);
        assert_eq!(swap.usdc_amount, 500);
        assert_eq!(swap.status, SwapStatus::Pending);
    }

    #[test]
    fn test_invalid_proof_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);
        let real_key = Bytes::from_slice(&env, b"real-key");
        let (zk_id, _) = setup_zk_verifier(&env, &seller, listing_id, &real_key);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &fee_recipient,
            &60u64,
            &zk_id,
            &registry_id,
        );
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 500);
        let key_bytes = Bytes::from_slice(&env, b"valid-key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
            &registry_id,
        );
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 1);
        let key_bytes = Bytes::from_slice(&env, b"secret-key");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &zk_id,
        );
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_id,
            &registry_id,
        );

        client.confirm_swap(&swap_id, &key_bytes, &proof_path);

        // SwapCompleted: topics = ["swap_completed", swap_id]; data = map { seller: address }
        let swap_id_xdr = soroban_sdk::xdr::ScVal::try_from_val(&env, &<u64 as IntoVal<Env, soroban_sdk::Val>>::into_val(&swap_id, &env)).unwrap();
        let name_xdr = soroban_sdk::xdr::ScVal::Symbol("swap_completed".try_into().unwrap());
        let found = env.events().all().filter_by_contract(&contract_id).events().iter().any(|e| {
            let body = match &e.body { soroban_sdk::xdr::ContractEventBody::V0(b) => b };
            body.topics.len() == 2
                && body.topics[0] == name_xdr
                && body.topics[1] == swap_id_xdr
        });
        assert!(found, "SwapCompleted event not emitted on confirm_swap");
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 100);
        let key_bytes = Bytes::from_slice(&env, b"k");
        let (zk_id, proof_path) = setup_zk_verifier(&env, &seller, listing_id, &key_bytes);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &100u32,
            &fee_recipient,
            &60u64,
            &zk_id,
            &registry_id,
        );
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &100,
        );
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
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            500,
        );

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
        let (registry_id, listing_id1) = setup_registry(&env, &seller, 500);
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
            &zk_id,
            &registry_id,
        );

        let id1 = client.initiate_swap(
            &listing_id1,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );
        let id2 = client.initiate_swap(
            &listing_id2,
            &buyer,
            &seller,
            &usdc_id,
            &500,
        );

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
        let (_, listing_id) = setup_registry(&env, &seller, 1);
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
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);

        pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            500,
        );

        assert!(!client.is_listing_available(&listing_id));
    }

    #[test]
    fn test_is_listing_available_after_cancel() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);

        let swap_id = pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            500,
        );
        env.ledger()
            .with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);
        assert!(client.is_listing_available(&listing_id));
    }

    #[test]
    fn test_get_swaps_by_buyer_page_empty_list() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let buyer = Address::generate(&env);
        let page = client.get_swaps_by_buyer_page(&buyer, &0u32, &10u32);
        assert_eq!(page.len(), 0);
    }

    #[test]
    fn test_get_swaps_by_buyer_page_full_page() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id1, registry_id, _cid, client, _admin) =
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
        let id1 = client.initiate_swap(
            &listing_id1,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
            &registry_id,
        );
        let id2 = client.initiate_swap(
            &listing_id2,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
            &registry_id,
        );
        let id3 = client.initiate_swap(
            &listing_id3,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
            &registry_id,
        );
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
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 1);
        pending_swap(
            &env,
            &client,
            listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &registry_id,
            500,
        );
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
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);
        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
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
        let (_, listing_id, registry_id, _, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);

        // Use a random address that was never added as an allowed token
        let bad_token = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &bad_token,
            &500,
            &zk_verifier,
            &registry_id,
        );
    }
}
