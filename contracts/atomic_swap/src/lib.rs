#![no_std]
use ip_registry::IpRegistryClient;
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Bytes, Env,
};

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;
const DEFAULT_DISPUTE_WINDOW_LEDGERS: u32 = 17_280;

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
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
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
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::DisputeWindowLedgers, &DEFAULT_DISPUTE_WINDOW_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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

    #[allow(clippy::too_many_arguments)]
    pub fn initiate_swap(
        env: Env,
        listing_id: u64,
        buyer: Address,
        seller: Address,
        usdc_token: Address,
        usdc_amount: i128,
        ip_registry: Address,
    ) -> u64 {
        Self::assert_not_paused(&env);
        buyer.require_auth();

        // Task 2: guard against zero-amount swaps
        if usdc_amount <= 0 {
            env.panic_with_error(ContractError::InvalidAmount);
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

        let listing = IpRegistryClient::new(&env, &ip_registry)
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
            &env.current_contract_address(),
            &usdc_amount,
        );

        let id: u64 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0) + 1;
        env.storage().instance().set(&DataKey::Counter, &id);

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

        // Task 2 (event): emit SwapInitiated
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

    pub fn confirm_swap(env: Env, swap_id: u64, decryption_key: Bytes) {
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

        // Task 2 (event): emit SwapConfirmed
        SwapConfirmed {
            swap_id,
            seller: swap.seller,
            decryption_key,
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
            .expect("confirmed_at_ledger missing");
        let window: u32 = env
            .storage()
            .instance()
            .get(&DataKey::DisputeWindowLedgers)
            .unwrap_or(DEFAULT_DISPUTE_WINDOW_LEDGERS);
        assert!(
            env.ledger().sequence() > confirmed_at + window,
            "dispute window has not yet expired"
        );

        let usdc = token::Client::new(&env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();
        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));

        let fee = Self::calculate_fee_amount(&env, swap.usdc_amount, config.fee_bps);
        let seller_amount = swap.usdc_amount - fee;
        if fee > 0 {
            usdc.transfer(&contract_addr, &config.fee_recipient, &fee);
        }
        usdc.transfer(&contract_addr, &swap.seller, &seller_amount);

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
            .expect("confirmed_at_ledger missing");
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

        let usdc = token::Client::new(&env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();

        if favor_buyer {
            usdc.transfer(&contract_addr, &swap.buyer, &swap.usdc_amount);
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
                    usdc.transfer(&contract_addr, &config.fee_recipient, &fee);
                }
                usdc.transfer(&contract_addr, &swap.seller, &seller_amount);
            } else {
                usdc.transfer(&contract_addr, &swap.seller, &swap.usdc_amount);
            }
            swap.status = SwapStatus::ResolvedSeller;
        }

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
        swap.buyer.require_auth();
        if swap.status != SwapStatus::Pending {
            env.panic_with_error(ContractError::SwapNotPending);
        }
        if env.ledger().timestamp() < swap.expires_at {

            env.panic_with_error(ContractError::SwapNotCancellable);
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

        // Task 2 (event): emit SwapCancelled
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

    /// Task 1: Returns the full Swap struct for a given swap_id, or None if not found.
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
        testutils::{Address as _, Events as _, Ledger as _},
        token, Bytes, Env,
    };

    // ── shared helpers ────────────────────────────────────────────────────────

    fn setup_registry(env: &Env, seller: &Address, price_usdc: i128) -> (Address, u64) {
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(env, &registry_id);
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

    /// Full setup: usdc, registry, contract, initialized client.
    fn setup_full<'a>(
        env: &'a Env,
        buyer: &Address,
        seller: &Address,
        usdc_amount: i128,
        price_usdc: i128,
    ) -> (Address, u64, Address, Address, AtomicSwapClient<'a>, Address) {
        let usdc_id = setup_usdc(env, buyer, usdc_amount);
        let (registry_id, listing_id) = setup_registry(env, seller, price_usdc);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let fee_recipient = Address::generate(env);
        client.initialize(&admin, &0u32, &fee_recipient, &60u64);
        (usdc_id, listing_id, registry_id, contract_id, client, admin)
    }

    fn pending_swap(
        env: &Env,
        client: &AtomicSwapClient,
        listing_id: u64,
        buyer: &Address,
        seller: &Address,
        usdc_id: &Address,
        registry_id: &Address,
        usdc_amount: i128,
    ) -> u64 {
        client.initiate_swap(&listing_id, buyer, seller, usdc_id, &usdc_amount, registry_id)
    }

    fn confirmed_swap(
        env: &Env,
        client: &AtomicSwapClient,
        listing_id: u64,
        buyer: &Address,
        seller: &Address,
        usdc_id: &Address,
        registry_id: &Address,
    ) -> u64 {
        let swap_id = pending_swap(env, client, listing_id, buyer, seller, usdc_id, registry_id, 500);
        client.confirm_swap(&swap_id, &Bytes::from_slice(env, b"bad-key"));
        swap_id
    }

    // ── Task 1: get_swap returns full struct ──────────────────────────────────

    #[test]
    fn test_get_swap_returns_full_struct() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &registry_id,
        );

        let swap = client.get_swap(&swap_id).expect("swap should exist");
        assert_eq!(swap.buyer, buyer);
        assert_eq!(swap.seller, seller);
        assert_eq!(swap.usdc_amount, 500);
        assert_eq!(swap.listing_id, listing_id);
        assert_eq!(swap.status, SwapStatus::Pending);
        assert!(swap.decryption_key.is_none());
        assert!(swap.confirmed_at_ledger.is_none());
    }

    #[test]
    fn test_get_swap_returns_none_for_missing() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swap(&999u64), None);
    }

    // ── Task 2: zero-amount guard ─────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")]
    fn test_initiate_swap_rejects_zero_amount() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 0, 0);

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &0, &registry_id);
    }

    // ── Task 2: event emission tests ──────────────────────────────────────────

    #[test]
    fn test_initiate_swap_emits_swap_initiated_event() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &registry_id);

        // Verify at least one event was emitted by the atomic_swap contract
        let all = env.events().all();
        let contract_events = all.filter_by_contract(&contract_id);
        assert!(!contract_events.events().is_empty(), "SwapInitiated event not emitted");
    }

    #[test]
    fn test_confirm_swap_emits_swap_confirmed_event() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"secret-key"));
        // Events from the confirm_swap call should include SwapConfirmed
        let contract_events = env.events().all().filter_by_contract(&contract_id);
        assert!(!contract_events.events().is_empty(), "SwapConfirmed event not emitted");
    }

    #[test]
    fn test_cancel_swap_emits_swap_cancelled_event() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        env.ledger().with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);
        // Events from the cancel_swap call should include SwapCancelled
        let contract_events = env.events().all().filter_by_contract(&contract_id);
        assert!(!contract_events.events().is_empty(), "SwapCancelled event not emitted");
    }

    // ── price enforcement ─────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #14)")]
    fn test_initiate_swap_rejects_underpayment() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 1000);
        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &registry_id);
    }

    #[test]
    fn test_initiate_swap_accepts_exact_price() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 1000);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000, &registry_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_initiate_swap_accepts_overpayment() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 500);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000, &registry_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    fn test_initiate_swap_allows_any_amount_when_price_is_zero() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 0);
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1, &registry_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    // ── happy path ────────────────────────────────────────────────────────────

    #[test]
    fn test_happy_path_initiate_confirm_release_to_seller() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
        assert_eq!(usdc_client.balance(&buyer), 0);
        assert_eq!(usdc_client.balance(&contract_id), 500);

        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"secret-key"));
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Completed));

        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedSeller));
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
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        env.ledger().with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Cancelled));
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&seller), 0);
        assert_eq!(usdc_client.balance(&contract_id), 0);
    }

    // ── double-action guards ──────────────────────────────────────────────────

    #[test]
    fn test_double_confirm_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"secret-key"));

        let second = client.try_confirm_swap(&swap_id, &Bytes::from_slice(&env, b"another-key"));
        assert_eq!(
            second,
            Err(Ok(soroban_sdk::Error::from_contract_error(ContractError::SwapNotPending as u32)))
        );
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Completed));
        assert_eq!(client.get_decryption_key(&swap_id), Some(Bytes::from_slice(&env, b"secret-key")));
    }

    #[test]
    fn test_double_cancel_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, contract_id, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        env.ledger().with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));
        client.cancel_swap(&swap_id);

        let second = client.try_cancel_swap(&swap_id);
        assert_eq!(
            second,
            Err(Ok(soroban_sdk::Error::from_contract_error(ContractError::SwapNotPending as u32)))
        );
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Cancelled));
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&contract_id), 0);
    }

    #[test]
    fn test_duplicate_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let buyer2 = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000, 0);
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer2, &500);

        pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);

        let result = client.try_initiate_swap(&listing_id, &buyer2, &seller, &usdc_id, &500, &registry_id);
        assert!(result.is_err());
    }

    // ── status / query ────────────────────────────────────────────────────────

    #[test]
    fn test_get_swap_status_returns_none_for_missing_swap() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swap_status(&999u64), None);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_confirm_swap_rejects_empty_key() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.confirm_swap(&0u64, &Bytes::new(&env));
    }

    #[test]
    fn test_confirm_swap_returns_error_for_missing_swap() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let result = client.try_confirm_swap(&999u64, &Bytes::from_slice(&env, b"key"));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_swap_status_returns_none_for_missing() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swap_status(&42u64), None);
    }

    #[test]
    fn test_decryption_key_accessible_after_confirmation() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"my-key"));
        assert_eq!(client.get_decryption_key(&swap_id), Some(Bytes::from_slice(&env, b"my-key")));
    }

    // ── fee tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_fee_deducted_and_sent_to_recipient() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 10_000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &250u32, &fee_recipient, &60u64);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &10_000, &registry_id);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &fee_recipient, &60u64);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000, &registry_id);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &250u32, &fee_recipient, &60u64);

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1, &registry_id);
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
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &250u32, &fee_recipient, &60u64);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &40, &registry_id);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));
        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(usdc_client.balance(&seller), 39);
        assert_eq!(usdc_client.balance(&fee_recipient), 1);
    }

    // ── pause / unpause ───────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_initiate_swap_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);
        client.pause();

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &registry_id);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_confirm_swap_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        client.pause();
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));
    }

    #[test]
    fn test_unpause_restores_initiate_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        client.pause();
        client.unpause();

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &registry_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    // ── cancel ────────────────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_cancel_swap_rejects_before_expiry() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let attacker = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

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
        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_id, &registry_id,
        );

        env.ledger().with_mut(|li| li.timestamp = li.timestamp.saturating_add(61));

        // Only authorize the attacker, not the buyer
        env.set_auths(&[]);
        env.mock_auths(&[soroban_sdk::auth::MockAuth {
            address: &attacker,
            invoke: &soroban_sdk::auth::MockAuthInvoke {
                contract: &contract_id,
                fn_name: "cancel_swap",
                args: (swap_id,).into_val(&env),
                sub_invokes: &[],
            },
        }]);

        let result = client.try_cancel_swap(&swap_id);
        assert!(result.is_err(), "non-buyer cancel should fail with auth error");
        // USDC should not have been refunded
        assert_eq!(token::Client::new(&env, &usdc_id).balance(&buyer), 500);
    }

    #[test]
    fn test_cancel_swap_allows_after_expiry() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &120u64);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &registry_id);
        env.ledger().with_mut(|li| li.timestamp = li.timestamp.saturating_add(121));
        client.cancel_swap(&swap_id);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Cancelled));
        assert_eq!(usdc_client.balance(&buyer), 1000);
    }

    // ── seller impersonation ──────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_seller_impersonation_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let real_seller = Address::generate(&env);
        let impersonator = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &real_seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        client.initiate_swap(&listing_id, &buyer, &impersonator, &usdc_id, &500, &registry_id);
    }

    // ── dispute ───────────────────────────────────────────────────────────────

    #[test]
    fn test_raise_dispute_within_window() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
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
            setup_full(&env, &buyer, &seller, 500, 0);

        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.raise_dispute(&swap_id);
    }

    #[test]
    fn test_raise_dispute_on_pending_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        let result = client.try_raise_dispute(&swap_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_dispute_window_boundary_exact_last_ledger() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        // At exactly confirmed_at + window the window is still open (> not >=)
        env.ledger().with_mut(|li| li.sequence_number += 10);
        client.raise_dispute(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Disputed));
    }

    #[test]
    fn test_resolve_dispute_favor_buyer_refunds_usdc() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &true);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedBuyer));
        assert_eq!(usdc_client.balance(&buyer), 500);
    }

    #[test]
    fn test_resolve_dispute_favor_seller_dismisses() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &false);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedSeller));
        assert_eq!(usdc_client.balance(&seller), 500);
    }

    #[test]
    fn test_resolve_dispute_on_non_disputed_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        let result = client.try_resolve_dispute(&swap_id, &true);
        assert!(result.is_err());
    }

    // ── buyer / seller index ──────────────────────────────────────────────────

    #[test]
    fn test_get_swaps_by_buyer_empty() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swaps_by_buyer(&Address::generate(&env)).len(), 0);
    }

    #[test]
    fn test_get_swaps_by_buyer_single() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids.get(0).unwrap(), swap_id);
    }

    #[test]
    fn test_get_swaps_by_buyer_multiple() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id1) = setup_registry(&env, &seller, 0);
        let listing_id2 = IpRegistryClient::new(&env, &registry_id).register_ip(
            &seller,
            &Bytes::from_slice(&env, b"hash2"),
            &Bytes::from_slice(&env, b"root2"),
            &0u32,
            &seller,
            &0i128,
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        let id1 = client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500, &registry_id);
        let id2 = client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500, &registry_id);

        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
    }

    #[test]
    fn test_get_swaps_by_seller_empty() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swaps_by_seller(&Address::generate(&env)).len(), 0);
    }

    #[test]
    fn test_get_swaps_by_seller_single() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
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
        let (registry_id, listing_id1) = setup_registry(&env, &seller, 0);
        let listing_id2 = IpRegistryClient::new(&env, &registry_id).register_ip(
            &seller,
            &Bytes::from_slice(&env, b"hash2"),
            &Bytes::from_slice(&env, b"root2"),
            &0u32,
            &seller,
            &0i128,
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        let id1 = client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500, &registry_id);
        let id2 = client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500, &registry_id);

        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
    }

    #[test]
    fn test_buyer_index_consistency_roundtrip() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        let ids = client.get_swaps_by_buyer(&buyer);
        assert!(ids.iter().any(|id| id == swap_id));
    }

    #[test]
    fn test_seller_index_consistency_roundtrip() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        let swap_id = pending_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id, 500);
        let ids = client.get_swaps_by_seller(&seller);
        assert!(ids.iter().any(|id| id == swap_id));
    }

    #[test]
    fn test_buyer_index_isolation() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer1 = Address::generate(&env);
        let buyer2 = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer1, 500);
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer2, &500);
        let (registry_id, listing_id) = setup_registry(&env, &seller, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        let id1 = client.initiate_swap(&listing_id, &buyer1, &seller, &usdc_id, &500, &registry_id);
        // buyer2 can't initiate on same listing while pending — just check buyer1's index
        let ids = client.get_swaps_by_buyer(&buyer1);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(client.get_swaps_by_buyer(&buyer2).len(), 0);
    }

    #[test]
    fn test_seller_index_isolation() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller1 = Address::generate(&env);
        let seller2 = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 500);
        let (registry_id, listing_id) = setup_registry(&env, &seller1, 0);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        client.initiate_swap(&listing_id, &buyer, &seller1, &usdc_id, &500, &registry_id);
        assert_eq!(client.get_swaps_by_seller(&seller2).len(), 0);
    }

    // ── swap count ────────────────────────────────────────────────────────────

    #[test]
    fn test_swap_count() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id1) = setup_registry(&env, &seller, 0);
        let listing_id2 = IpRegistryClient::new(&env, &registry_id).register_ip(
            &seller,
            &Bytes::from_slice(&env, b"h2"),
            &Bytes::from_slice(&env, b"r2"),
            &0u32,
            &seller,
            &0i128,
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        let id1 = client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500, &registry_id);
        let id2 = client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500, &registry_id);
        assert_eq!(id1, 1u64);
        assert_eq!(id2, 2u64);
    }

    // ── set_dispute_window ────────────────────────────────────────────────────

    #[test]
    fn test_set_dispute_window_updates_config() {
        let env = Env::default();
        env.mock_all_auths();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500, 0);

        client.set_dispute_window(&5u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        // window = 5, advance by 6 → release should succeed
        env.ledger().with_mut(|li| li.sequence_number += 6);
        client.release_to_seller(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedSeller));
    }
}
