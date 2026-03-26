#![no_std]
use ip_registry::IpRegistryClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Bytes, Env,
};

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;
/// Default dispute window: ~24 hours at ~5s/ledger
const DEFAULT_DISPUTE_WINDOW_LEDGERS: u32 = 17_280;

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum ContractError {
    EmptyDecryptionKey = 1,
    SwapNotFound = 2,
    InvalidAmount = 3,
    ContractPaused = 4,
    NotInitialized = 5,
    SwapNotPending = 6,
    SwapAlreadyPending = 7,
    SellerMismatch = 8,
    SwapNotCancellable = 9,
    DisputeWindowExpired = 10,
    SwapNotCompleted = 11,
    SwapNotDisputed = 12,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum SwapStatus {
    Pending,
    Completed,
    Cancelled,
    /// Buyer raised a dispute; awaiting admin arbitration.
    Disputed,
    /// Admin resolved in buyer's favour — USDC refunded.
    ResolvedBuyer,
    /// Admin resolved in seller's favour — no further action.
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
#[derive(Clone)]
pub struct Swap {
    pub listing_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub usdc_amount: i128,
    pub usdc_token: Address,
    pub zk_verifier: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: SwapStatus,
    pub decryption_key: Option<Bytes>,
    /// Ledger sequence number at which confirm_swap was called.
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
    /// Number of ledgers after confirmation during which buyer may raise a dispute.
    DisputeWindowLedgers,
}

/// Emitted when a buyer initiates a swap.
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

/// Emitted when a seller confirms a swap and releases the decryption key.
#[contractevent]
pub struct SwapConfirmed {
    #[topic]
    pub swap_id: u64,
    pub seller: Address,
    pub decryption_key: Bytes,
}

/// Emitted when a buyer cancels an expired swap and reclaims USDC.
#[contractevent]
pub struct SwapCancelled {
    #[topic]
    pub swap_id: u64,
    pub buyer: Address,
    pub usdc_amount: i128,
}

#[contract]
pub struct AtomicSwap;

#[contractimpl]
impl AtomicSwap {
    /// One-time initialisation: store protocol fee config and admin.
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_bps: u32,
        fee_recipient: Address,
        cancel_delay_secs: u64,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            env.panic_with_error(ContractError::NotInitialized);
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

    /// Admin: update the dispute window length (in ledgers).
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

    /// Pause the contract — blocks initiate_swap and confirm_swap. Admin only.
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
    }

    /// Unpause the contract. Admin only.
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

    /// Buyer initiates swap by locking USDC into the contract.
    /// Cross-calls ip_registry to verify seller owns the listing.
    #[allow(clippy::too_many_arguments)]
    pub fn initiate_swap(
        env: Env,
        listing_id: u64,
        buyer: Address,
        seller: Address,
        usdc_token: Address,
        usdc_amount: i128,
        zk_verifier: Address,
        ip_registry: Address,
    ) -> u64 {
        Self::assert_not_paused(&env);
        buyer.require_auth();
        if usdc_amount <= 0 {
            env.panic_with_error(ContractError::InvalidAmount);
        }
        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
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
                .unwrap();
            if existing_swap.status == SwapStatus::Pending && existing_swap.buyer != buyer {
                env.panic_with_error(ContractError::SwapAlreadyPending);
            }
        }

        let listing = IpRegistryClient::new(&env, &ip_registry).get_listing(&listing_id);
        if listing.owner != seller {
            env.panic_with_error(ContractError::SellerMismatch);
        }

        token::Client::new(&env, &usdc_token).transfer(
            &buyer,
            &env.current_contract_address(),
            &usdc_amount,
        );

        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::Counter)
            .unwrap_or(0)
            + 1;
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
                zk_verifier,
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
        env.storage()
            .persistent()
            .extend_ttl(&active_listing_key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        // Update buyer index
        let buyer_key = DataKey::BuyerIndex(buyer.clone());
        let mut buyer_ids: soroban_sdk::Vec<u64> = env
            .storage()
            .persistent()
            .get(&buyer_key)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        buyer_ids.push_back(id);
        env.storage().persistent().set(&buyer_key, &buyer_ids);
        env.storage()
            .persistent()
            .extend_ttl(&buyer_key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        // Update seller index
        let seller_key = DataKey::SellerIndex(seller.clone());
        let mut seller_ids: soroban_sdk::Vec<u64> = env
            .storage()
            .persistent()
            .get(&seller_key)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        seller_ids.push_back(id);
        env.storage().persistent().set(&seller_key, &seller_ids);
        env.storage()
            .persistent()
            .extend_ttl(&seller_key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

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

    /// Seller confirms swap by submitting the decryption key.
    /// USDC is NOT released immediately — it stays in the contract during the dispute window.
    /// After the window expires, call `release_to_seller` to finalise the payout.
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

        SwapConfirmed {
            swap_id,
            seller: swap.seller,
            decryption_key,
        }
        .publish(&env);
    }

    /// Release USDC to the seller after the dispute window has expired.
    /// Callable by anyone once the window is closed and the swap is in Completed status.
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

        let confirmed_at = swap.confirmed_at_ledger.expect("confirmed_at_ledger missing");
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

        if let Some(config) = env.storage().instance().get::<DataKey, Config>(&DataKey::Config) {
            let fee: i128 = swap.usdc_amount * config.fee_bps as i128 / 10_000;
            let seller_amount = swap.usdc_amount - fee;
            if fee > 0 {
                usdc.transfer(&contract_addr, &config.fee_recipient, &fee);
            }
            usdc.transfer(&contract_addr, &swap.seller, &seller_amount);
        } else {
            usdc.transfer(&contract_addr, &swap.seller, &swap.usdc_amount);
        }

        swap.status = SwapStatus::ResolvedSeller;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Buyer raises a dispute after confirm_swap if the decryption key is invalid.
    /// Must be called within the dispute window (ledgers since confirmation).
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

        let confirmed_at = swap.confirmed_at_ledger.expect("confirmed_at_ledger missing");
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

    /// Admin resolves a disputed swap.
    /// If `favor_buyer` is true, USDC is refunded to the buyer.
    /// If false, the dispute is dismissed and funds are released to the seller.
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

        if favor_buyer {
            token::Client::new(&env, &swap.usdc_token).transfer(
                &env.current_contract_address(),
                &swap.buyer,
                &swap.usdc_amount,
            );
            swap.status = SwapStatus::ResolvedBuyer;
        } else {
            let usdc = token::Client::new(&env, &swap.usdc_token);
            let contract_addr = env.current_contract_address();
            if let Some(config) =
                env.storage().instance().get::<DataKey, Config>(&DataKey::Config)
            {
                let fee: i128 = swap.usdc_amount * config.fee_bps as i128 / 10_000;
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

    /// Buyer cancels and reclaims USDC if seller never confirms (after expiry).
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
        if env.ledger().timestamp() < swap.expires_at {
            env.panic_with_error(ContractError::SwapNotCancellable);
        }
        swap.buyer.require_auth();
        token::Client::new(&env, &swap.usdc_token).transfer(
            &env.current_contract_address(),
            &swap.buyer,
            &swap.usdc_amount,
        );
        swap.status = SwapStatus::Cancelled;
        env.storage().persistent().set(&key, &swap);
        env.storage()
            .events()
            .publish((symbol_short!("cancelled"), swap_id), swap.buyer.clone());
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

    /// Returns the current status of a swap, or None if it doesn't exist.
    pub fn get_swap_status(env: Env, swap_id: u64) -> Option<SwapStatus> {
        env.storage()
            .persistent()
            .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            .map(|swap| swap.status)
    }

    /// Returns the decryption key once the swap is completed.
    pub fn get_decryption_key(env: Env, swap_id: u64) -> Option<Bytes> {
        env.storage()
            .persistent()
            .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            .and_then(|swap| swap.decryption_key)
    }

    /// Returns all swap IDs initiated by the given buyer, in insertion order.
    pub fn get_swaps_by_buyer(env: Env, buyer: Address) -> soroban_sdk::Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::BuyerIndex(buyer))
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env))
    }

    /// Returns all swap IDs where the given address is the seller, in insertion order.
    pub fn get_swaps_by_seller(env: Env, seller: Address) -> soroban_sdk::Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::SellerIndex(seller))
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    extern crate std;
    use ip_registry::{IpRegistry, IpRegistryClient};
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, token, Bytes, Env};

    fn setup_registry(env: &Env, seller: &Address) -> (Address, u64) {
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(env, &registry_id);
        let listing_id = registry.register_ip(
            seller,
            &Bytes::from_slice(env, b"QmHash"),
            &Bytes::from_slice(env, b"root"),
        );
        (registry_id, listing_id)
    }

    fn setup_usdc(env: &Env, buyer: &Address, amount: i128) -> Address {
        let admin = Address::generate(env);
        let usdc_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
        token::StellarAssetClient::new(env, &usdc_id).mint(buyer, &amount);
        usdc_id
    }

    /// Full environment helper: returns (usdc_id, listing_id, registry_id, contract_id, client, admin).
    fn setup_full<'a>(
        env: &'a Env,
        buyer: &Address,
        seller: &Address,
        usdc_amount: i128,
    ) -> (Address, u64, Address, Address, AtomicSwapClient<'a>, Address) {
        let usdc_id = setup_usdc(env, buyer, usdc_amount);
        let (registry_id, listing_id) = setup_registry(env, seller);
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let fee_recipient = Address::generate(env);
        client.initialize(&admin, &0u32, &fee_recipient, &60u64);
        (usdc_id, listing_id, registry_id, contract_id, client, admin)
    }

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
        client.confirm_swap(&0, &Bytes::new(&env));
    }

    #[test]
    fn test_decryption_key_accessible_after_confirmation() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &100u32, &fee_recipient, &60u64);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        let key = Bytes::from_slice(&env, b"super-secret-key");
        client.confirm_swap(&swap_id, &key);

        assert_eq!(client.get_decryption_key(&swap_id), Some(key));
        // Funds still in escrow — seller has 0 until release
        let usdc_client = token::Client::new(&env, &usdc_id);
        assert_eq!(usdc_client.balance(&seller), 0);

        // Advance past dispute window and release
        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        // fee = 500 * 100 / 10000 = 5; seller gets 495
        assert_eq!(usdc_client.balance(&seller), 495);
        assert_eq!(usdc_client.balance(&fee_recipient), 5);
    }

    #[test]
    fn test_fee_deducted_and_sent_to_recipient() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 10_000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &250u32, &fee_recipient, &60u64);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &10_000, &zk_verifier, &registry_id,
        );
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
        let zk_verifier = Address::generate(&env);
        let fee_recipient = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &fee_recipient, &60u64);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &1000, &zk_verifier, &registry_id,
        );
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));

        client.set_dispute_window(&10u32);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.release_to_seller(&swap_id);

        assert_eq!(usdc_client.balance(&seller), 1000);
        assert_eq!(usdc_client.balance(&fee_recipient), 0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_initiate_swap_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let admin = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64);
        client.pause();

        client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_confirm_swap_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let admin = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64);
        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        client.pause();
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));
    }

    #[test]
    fn test_unpause_restores_initiate_swap() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let admin = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64);
        client.pause();
        client.unpause();

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #8)")]
    fn test_seller_impersonation_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let real_seller = Address::generate(&env);
        let impersonator = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &real_seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &60u64);

        client.initiate_swap(
            &listing_id, &buyer, &impersonator, &usdc_id, &500, &zk_verifier, &registry_id,
        );
    }

    #[test]
    fn test_get_swaps_by_buyer_empty() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let stranger = Address::generate(&env);
        assert_eq!(client.get_swaps_by_buyer(&stranger).len(), 0);
    }

    #[test]
    fn test_get_swaps_by_buyer_single() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
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
        let zk_verifier = Address::generate(&env);
        let (usdc_id, listing_id1, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1500);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id2 = registry.register_ip(
            &seller, &Bytes::from_slice(&env, b"QmHash2"), &Bytes::from_slice(&env, b"root2"),
        );
        let listing_id3 = registry.register_ip(
            &seller, &Bytes::from_slice(&env, b"QmHash3"), &Bytes::from_slice(&env, b"root3"),
        );

        let id1 = client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
        let id2 = client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
        let id3 = client.initiate_swap(&listing_id3, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);

        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
        assert_eq!(ids.get(2).unwrap(), id3);
    }

    #[test]
    fn test_buyer_index_isolation() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer_a = Address::generate(&env);
        let buyer_b = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let (usdc_id, listing_id_a, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer_a, &seller, 500);
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer_b, &500);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_b = registry.register_ip(
            &seller, &Bytes::from_slice(&env, b"QmHash2"), &Bytes::from_slice(&env, b"root2"),
        );

        let id_a = client.initiate_swap(&listing_id_a, &buyer_a, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
        let id_b = client.initiate_swap(&listing_id_b, &buyer_b, &seller, &usdc_id, &500, &zk_verifier, &registry_id);

        let ids_a = client.get_swaps_by_buyer(&buyer_a);
        assert_eq!(ids_a.len(), 1);
        assert_eq!(ids_a.get(0).unwrap(), id_a);

        let ids_b = client.get_swaps_by_buyer(&buyer_b);
        assert_eq!(ids_b.len(), 1);
        assert_eq!(ids_b.get(0).unwrap(), id_b);
    }

    #[test]
    fn test_buyer_index_consistency_roundtrip() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let (usdc_id, listing_id1, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 1000);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id2 = registry.register_ip(
            &seller, &Bytes::from_slice(&env, b"QmHash2"), &Bytes::from_slice(&env, b"root2"),
        );

        client.initiate_swap(&listing_id1, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
        client.initiate_swap(&listing_id2, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);

        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 2);
        for i in 0..ids.len() {
            let id = ids.get(i).unwrap();
            assert!(client.get_swap_status(&id).is_some());
        }
    }

    // ── seller index tests ────────────────────────────────────────────────────

    #[test]
    fn test_get_swaps_by_seller_empty() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        let stranger = Address::generate(&env);
        assert_eq!(client.get_swaps_by_seller(&stranger).len(), 0);
    }

    #[test]
    fn test_get_swaps_by_seller_single() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids.get(0).unwrap(), swap_id);
    }

    // ── cancel tests ──────────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_cancel_swap_rejects_before_expiry() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &120u64);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        client.cancel_swap(&swap_id);
    }

    #[test]
    fn test_cancel_swap_allows_after_expiry() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let usdc_id = setup_usdc(&env, &buyer, 1000);
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(&Address::generate(&env), &0u32, &Address::generate(&env), &120u64);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        env.ledger().with_mut(|li| li.timestamp = li.timestamp.saturating_add(121));
        client.cancel_swap(&swap_id);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Cancelled));
        assert_eq!(usdc_client.balance(&buyer), 1000);
    }

    // ── dispute window tests ──────────────────────────────────────────────────

    fn confirmed_swap(
        env: &Env,
        client: &AtomicSwapClient,
        listing_id: u64,
        buyer: &Address,
        seller: &Address,
        usdc_id: &Address,
        registry_id: &Address,
    ) -> u64 {
        let zk_verifier = Address::generate(env);
        let swap_id = client.initiate_swap(
            &listing_id, buyer, seller, usdc_id, &500, &zk_verifier, registry_id,
        );
        client.confirm_swap(&swap_id, &Bytes::from_slice(env, b"bad-key"));
        swap_id
    }

    #[test]
    fn test_raise_dispute_within_window() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        client.raise_dispute(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Disputed));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_raise_dispute_after_window_expires() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        client.set_dispute_window(&10u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        env.ledger().with_mut(|li| li.sequence_number += 11);
        client.raise_dispute(&swap_id);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #11)")]
    fn test_raise_dispute_on_pending_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        client.raise_dispute(&swap_id);
    }

    #[test]
    fn test_resolve_dispute_favor_buyer_refunds_usdc() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        assert_eq!(usdc_client.balance(&seller), 0);

        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &true);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedBuyer));
        assert_eq!(usdc_client.balance(&buyer), 500);
        assert_eq!(usdc_client.balance(&seller), 0);
    }

    #[test]
    fn test_resolve_dispute_favor_seller_dismisses() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);
        let usdc_client = token::Client::new(&env, &usdc_id);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        client.raise_dispute(&swap_id);
        client.resolve_dispute(&swap_id, &false);

        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::ResolvedSeller));
        assert_eq!(usdc_client.balance(&seller), 500);
        assert_eq!(usdc_client.balance(&buyer), 0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #12)")]
    fn test_resolve_dispute_on_non_disputed_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        client.resolve_dispute(&swap_id, &true);
    }

    #[test]
    fn test_dispute_window_boundary_exact_last_ledger() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        client.set_dispute_window(&5u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        env.ledger().with_mut(|li| li.sequence_number += 5);

        client.raise_dispute(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Disputed));
    }

    #[test]
    fn test_set_dispute_window_updates_config() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let (usdc_id, listing_id, registry_id, _cid, client, _admin) =
            setup_full(&env, &buyer, &seller, 500);

        client.set_dispute_window(&1u32);
        let swap_id = confirmed_swap(&env, &client, listing_id, &buyer, &seller, &usdc_id, &registry_id);
        env.ledger().with_mut(|li| li.sequence_number += 1);
        client.raise_dispute(&swap_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Disputed));
    }
}
