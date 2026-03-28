#![no_std]
use ip_registry::IpRegistryClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, token,
    Address, Bytes, Env,
};

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;

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
    CounterOverflow = 10,
    AlreadyInitialized = 11,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum SwapStatus {
    Pending,
    Completed,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub fee_bps: u32,
    pub fee_recipient: Address,
    pub cancel_delay_secs: u64,
    pub ip_registry: Address,
    pub allowed_tokens: soroban_sdk::Vec<Address>,
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
        ip_registry: Address,
        allowed_tokens: soroban_sdk::Vec<Address>,
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
                ip_registry,
                allowed_tokens,
            },
        );
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Pause the contract — blocks initiate_swap and confirm_swap. Admin only.
pub fn pause(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Unpause the contract. Admin only.
pub fn unpause(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin)
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
    ) -> u64 {
        Self::assert_not_paused(&env);
        buyer.require_auth();
        if usdc_amount <= 0 {
            env.panic_with_error(ContractError::InvalidAmount);
        }
        let config: Config = env.storage().instance().get(&DataKey::Config)
            .unwrap_or_else(|| env.panic_with_error(ContractError::NotInitialized));

        // Validate usdc_token is in the allowed list
        if !config.allowed_tokens.contains(&usdc_token) {
            env.panic_with_error(ContractError::InvalidAmount);
        }

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

        // Verify seller owns the listing in ip_registry
        let listing = IpRegistryClient::new(&env, &config.ip_registry)
            .get_listing(&listing_id)
            .unwrap_or_else(|| env.panic_with_error(ContractError::SwapNotFound));
        if listing.owner != seller {
            env.panic_with_error(ContractError::SellerMismatch);
        }

        token::Client::new(&env, &usdc_token).transfer(
            &buyer,
            &env.current_contract_address(),
            &usdc_amount,
        );
        let prev: u64 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0);
        let id: u64 = prev
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CounterOverflow));
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

        // Maintain buyer index
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

        // Maintain seller index
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

    /// Seller confirms swap by submitting the decryption key; USDC released atomically.
    ///
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `swap_id` - The ID of the swap to confirm.
    /// * `decryption_key` - The decryption key for the off-chain data.
    ///
    /// # Returns
    /// This function does not return a value.
    ///
    /// # Panics
    /// * Panics if the `decryption_key` is empty (`ContractError::EmptyDecryptionKey`).
    /// * Panics if the swap does not exist.
    /// * Panics if the swap status is not `Pending`.
    /// * Panics if the caller is not the seller.
    /// * Panics if the token transfer fails.
    /// If a Config is present, a basis-point fee is deducted and sent to fee_recipient.
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

        let usdc = token::Client::new(&env, &swap.usdc_token);
        let contract_addr = env.current_contract_address();

        if let Some(config) = env
            .storage()
            .instance()
            .get::<DataKey, Config>(&DataKey::Config)
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

        swap.status = SwapStatus::Completed;
        swap.decryption_key = Some(decryption_key.clone());
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

    /// Buyer cancels and reclaims USDC if seller never confirms.
    ///
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `swap_id` - The ID of the swap to cancel.
    ///
    /// # Returns
    /// This function does not return a value.
    ///
    /// # Panics
    /// * Panics if the swap does not exist.
    /// * Panics if the swap status is not `Pending`.
    /// * Panics if the caller is not the buyer.
    /// * Panics if the token transfer fails.
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

    /// Retrieves the current status of a given swap.
    ///
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `swap_id` - The ID of the swap.
    ///
    /// # Returns
    /// Returns `Some(SwapStatus)` if the swap exists, or `None` if it does not.
    ///
    /// # Panics
    /// This view function does not panic under normal conditions.
    pub fn get_swap_status(env: Env, swap_id: u64) -> Option<SwapStatus> {
        env.storage()
            .persistent()
            .get::<DataKey, Swap>(&DataKey::Swap(swap_id))
            .map(|swap| swap.status)
    }

    /// Returns the decryption key once the swap is completed.
    ///
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `swap_id` - The ID of the swap.
    ///
    /// # Returns
    /// Returns `Some(Bytes)` containing the decryption key if the swap exists and is completed.
    /// Returns `None` if the swap does not exist or the key has not been submitted yet.
    ///
    /// # Panics
    /// This view function does not panic under normal conditions.
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
    ///
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `seller` - The address of the seller.
    ///
    /// # Returns
    /// A `Vec<u64>` of swap IDs in the order they were created. Returns an empty
    /// vec if the seller has no swaps. Never panics.
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
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Bytes, Env,
    };

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
        let usdc_id = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        token::StellarAssetClient::new(env, &usdc_id).mint(buyer, &amount);
        usdc_id
    }

    #[test]
    fn test_get_swap_status_returns_none_for_missing_swap() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swap_status(&999), None);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
    fn test_confirm_swap_returns_error_for_missing_swap() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.confirm_swap(&999, &Bytes::from_slice(&env, b"key"));
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
        let usdc_client = token::Client::new(&env, &usdc_id);
        let (registry_id, listing_id) = setup_registry(&env, &seller);

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);

        // 100 bps = 1%
        let mut allowed = soroban_sdk::Vec::new(&env);
        allowed.push_back(usdc_id.clone());
        client.initialize(&Address::generate(&env), &100u32, &fee_recipient, &60u64, &registry_id, &allowed);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let key = Bytes::from_slice(&env, b"super-secret-key");
        client.confirm_swap(&swap_id, &key);

        assert_eq!(client.get_decryption_key(&swap_id), Some(key));
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

        // 250 bps = 2.5%
        let mut allowed = soroban_sdk::Vec::new(&env);
        allowed.push_back(usdc_id.clone());
        client.initialize(&Address::generate(&env), &250u32, &fee_recipient, &60u64, &registry_id, &allowed);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &10_000,
            &zk_verifier,
        );
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));

        // fee = 10000 * 250 / 10000 = 250; seller gets 9750
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

        let mut allowed = soroban_sdk::Vec::new(&env);
        allowed.push_back(usdc_id.clone());
        client.initialize(&Address::generate(&env), &0u32, &fee_recipient, &60u64, &registry_id, &allowed);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &1000,
            &zk_verifier,
        );
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));

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

        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64, &registry_id, &{
            let mut v = soroban_sdk::Vec::new(&env);
            v.push_back(usdc_id.clone());
            v
        });
        client.pause();

        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
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

        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64, &registry_id, &{
            let mut v = soroban_sdk::Vec::new(&env);
            v.push_back(usdc_id.clone());
            v
        });
        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
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

        client.initialize(&admin, &0u32, &Address::generate(&env), &60u64, &registry_id, &{
            let mut v = soroban_sdk::Vec::new(&env);
            v.push_back(usdc_id.clone());
            v
        });
        client.pause();
        client.unpause();

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_duplicate_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let usdc_admin = Address::generate(&env);
        let usdc_id = env
            .register_stellar_asset_contract_v2(usdc_admin.clone())
            .address();

        let buyer1 = Address::generate(&env);
        let buyer2 = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer1, &1000);
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer2, &1000);

        // Register listing with seller as owner
        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &registry_id,
            &{
                let mut v = soroban_sdk::Vec::new(&env);
                v.push_back(usdc_id.clone());
                v
            },
        );

        // buyer1 initiates
        client.initiate_swap(
            &listing_id,
            &buyer1,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        // buyer2 initiates, should panic
        client.initiate_swap(
            &listing_id,
            &buyer2,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
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
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &registry_id,
            &{
                let mut v = soroban_sdk::Vec::new(&env);
                v.push_back(usdc_id.clone());
                v
            },
        );

        client.initiate_swap(
            &listing_id,
            &buyer,
            &impersonator,
            &usdc_id,
            &500,
            &zk_verifier,
        );
    }

    // ── helper ────────────────────────────────────────────────────────────────
    /// Sets up a full swap environment: USDC token, mints `usdc_amount` to `buyer`,
    /// registers one IP listing owned by `seller`, and registers the AtomicSwap contract.
    /// Returns `(usdc_id, listing_id, registry_id, contract_id, client)`.
    fn setup_swap_env<'a>(
        env: &'a Env,
        buyer: &Address,
        seller: &Address,
        usdc_amount: i128,
    ) -> (Address, u64, Address, Address, AtomicSwapClient<'a>) {
        let usdc_admin = Address::generate(env);
        let usdc_id = env
            .register_stellar_asset_contract_v2(usdc_admin.clone())
            .address();
        token::StellarAssetClient::new(env, &usdc_id).mint(buyer, &usdc_amount);

        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(env, &registry_id);
        let listing_id = registry.register_ip(
            seller,
            &Bytes::from_slice(env, b"QmHash"),
            &Bytes::from_slice(env, b"root"),
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(env, &contract_id);
        let mut allowed = soroban_sdk::Vec::new(env);
        allowed.push_back(usdc_id.clone());
        client.initialize(
            &Address::generate(env),
            &0u32,
            &Address::generate(env),
            &60u64,
            &registry_id,
            &allowed,
        );
        (usdc_id, listing_id, registry_id, contract_id, client)
    }

    // ── 5.1 ───────────────────────────────────────────────────────────────────
    #[test]
    fn test_get_swaps_by_buyer_empty() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);

        let stranger = Address::generate(&env);
        let result = client.get_swaps_by_buyer(&stranger);
        assert_eq!(result.len(), 0);
    }

    // ── 5.2 ───────────────────────────────────────────────────────────────────
    #[test]
    fn test_get_swaps_by_buyer_single() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let (usdc_id, listing_id, _registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 500);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids.get(0).unwrap(), swap_id);
    }

    // ── 5.3 ───────────────────────────────────────────────────────────────────
    #[test]
    fn test_get_swaps_by_buyer_multiple() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        // Mint enough for 3 swaps of 500 each
        let (usdc_id, listing_id, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 1500);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_2 = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash-2"),
            &Bytes::from_slice(&env, b"root-2"),
        );
        let listing_id_3 = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash-3"),
            &Bytes::from_slice(&env, b"root-3"),
        );

        let id1 = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        let id2 = client.initiate_swap(
            &listing_id_2,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        let id3 = client.initiate_swap(
            &listing_id_3,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
        assert_eq!(ids.get(2).unwrap(), id3);
    }

    // ── 5.4 ───────────────────────────────────────────────────────────────────
    #[test]
    fn test_buyer_index_isolation() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer_a = Address::generate(&env);
        let buyer_b = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        // Set up for buyer_a
        let (usdc_id, listing_id_a, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer_a, &seller, 500);

        // Mint USDC for buyer_b using the same token
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer_b, &500);

        // Register a second listing for buyer_b's swap
        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_b = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash2"),
            &Bytes::from_slice(&env, b"root2"),
        );

        let id_a = client.initiate_swap(
            &listing_id_a,
            &buyer_a,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        let id_b = client.initiate_swap(
            &listing_id_b,
            &buyer_b,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids_a = client.get_swaps_by_buyer(&buyer_a);
        assert_eq!(ids_a.len(), 1);
        assert_eq!(ids_a.get(0).unwrap(), id_a);

        let ids_b = client.get_swaps_by_buyer(&buyer_b);
        assert_eq!(ids_b.len(), 1);
        assert_eq!(ids_b.get(0).unwrap(), id_b);
    }

    // ── 5.5 ───────────────────────────────────────────────────────────────────
    #[test]
    fn test_buyer_index_consistency_roundtrip() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let (usdc_id, listing_id, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 1000);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_2 = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash-4"),
            &Bytes::from_slice(&env, b"root-4"),
        );

        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        client.initiate_swap(
            &listing_id_2,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids = client.get_swaps_by_buyer(&buyer);
        assert_eq!(ids.len(), 2);
        for i in 0..ids.len() {
            let id = ids.get(i).unwrap();
            assert!(
                client.get_swap_status(&id).is_some(),
                "swap_id {} has no corresponding swap record",
                id
            );
        }
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")]
    fn test_initiate_swap_rejects_zero_amount() {
        let env = Env::default();
        env.mock_all_auths();

        let usdc_admin = Address::generate(&env);
        let usdc_id = env
            .register_stellar_asset_contract_v2(usdc_admin.clone())
            .address();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);
        token::StellarAssetClient::new(&env, &usdc_id).mint(&buyer, &1000);

        let registry_id = env.register(IpRegistry, ());
        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );

        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &60u64,
            &registry_id,
            &{
                let mut v = soroban_sdk::Vec::new(&env);
                v.push_back(usdc_id.clone());
                v
            },
        );

        // zero amount should be rejected before any transfer or storage
        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &0,
            &zk_verifier,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_cancel_swap_rejects_before_expiry() {
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
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &registry_id,
            &{
                let mut v = soroban_sdk::Vec::new(&env);
                v.push_back(usdc_id.clone());
                v
            },
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        assert_eq!(usdc_client.balance(&buyer), 500);

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
        client.initialize(
            &Address::generate(&env),
            &0u32,
            &Address::generate(&env),
            &120u64,
            &registry_id,
            &{
                let mut v = soroban_sdk::Vec::new(&env);
                v.push_back(usdc_id.clone());
                v
            },
        );

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
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

    // ── seller index ──────────────────────────────────────────────────────────

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

        let (usdc_id, listing_id, _registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 500);

        let swap_id = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
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
        let zk_verifier = Address::generate(&env);

        let (usdc_id, listing_id, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 1500);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_2 = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash-s2"),
            &Bytes::from_slice(&env, b"root-s2"),
        );
        let listing_id_3 = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash-s3"),
            &Bytes::from_slice(&env, b"root-s3"),
        );

        let id1 = client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        let id2 = client.initiate_swap(
            &listing_id_2,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        let id3 = client.initiate_swap(
            &listing_id_3,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
        assert_eq!(ids.get(2).unwrap(), id3);
    }

    #[test]
    fn test_seller_index_isolation() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller_a = Address::generate(&env);
        let seller_b = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        // seller_a setup
        let (usdc_id, listing_id_a, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller_a, 1000);

        // register a listing owned by seller_b
        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_b = registry.register_ip(
            &seller_b,
            &Bytes::from_slice(&env, b"QmHash-b"),
            &Bytes::from_slice(&env, b"root-b"),
        );

        let id_a = client.initiate_swap(
            &listing_id_a,
            &buyer,
            &seller_a,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        let id_b = client.initiate_swap(
            &listing_id_b,
            &buyer,
            &seller_b,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids_a = client.get_swaps_by_seller(&seller_a);
        assert_eq!(ids_a.len(), 1);
        assert_eq!(ids_a.get(0).unwrap(), id_a);

        let ids_b = client.get_swaps_by_seller(&seller_b);
        assert_eq!(ids_b.len(), 1);
        assert_eq!(ids_b.get(0).unwrap(), id_b);
    }

    #[test]
    fn test_seller_index_consistency_roundtrip() {
        let env = Env::default();
        env.mock_all_auths();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let zk_verifier = Address::generate(&env);

        let (usdc_id, listing_id, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 1000);

        let registry = IpRegistryClient::new(&env, &registry_id);
        let listing_id_2 = registry.register_ip(
            &seller,
            &Bytes::from_slice(&env, b"QmHash-r2"),
            &Bytes::from_slice(&env, b"root-r2"),
        );

        client.initiate_swap(
            &listing_id,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );
        client.initiate_swap(
            &listing_id_2,
            &buyer,
            &seller,
            &usdc_id,
            &500,
            &zk_verifier,
        );

        let ids = client.get_swaps_by_seller(&seller);
        assert_eq!(ids.len(), 2);
        for i in 0..ids.len() {
            let id = ids.get(i).unwrap();
            assert!(
                client.get_swap_status(&id).is_some(),
                "swap_id {} has no corresponding swap record",
                id
            );
        }
    }
}
