#![no_std]
use ip_registry::IpRegistryClient;
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Bytes, Env};

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ContractError {
    EmptyDecryptionKey,
}

#[contracttype]
#[derive(Clone, PartialEq)]
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
    pub status: SwapStatus,
    pub decryption_key: Option<Bytes>,
}

#[contracttype]
pub enum DataKey {
    Swap(u64),
    Counter,
    BuyerIndex(Address),
    Config,
    Admin,
    Paused,
}

#[contract]
pub struct AtomicSwap;

#[contractimpl]
impl AtomicSwap {
    /// One-time initialisation: store protocol fee config and admin.
    pub fn initialize(env: Env, admin: Address, fee_bps: u32, fee_recipient: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Config),
            "already initialized"
        );
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Config, &Config { fee_bps, fee_recipient });
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Pause the contract — blocks initiate_swap and confirm_swap. Admin only.
    pub fn pause(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage().instance().extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    /// Unpause the contract. Admin only.
    pub fn unpause(env: Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }

    fn assert_not_paused(env: &Env) {
        let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
        assert!(!paused, "contract is paused");
    }

    /// Buyer initiates swap by locking USDC into the contract.
    /// Cross-calls ip_registry to verify seller owns the listing.
    /// 
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `listing_id` - The ID of the listing in the IP registry.
    /// * `buyer` - The address of the buyer initiating the swap.
    /// * `seller` - The address of the seller owning the listing.
    /// * `usdc_token` - The address of the USDC token contract.
    /// * `usdc_amount` - The amount of USDC to lock up.
    /// * `zk_verifier` - The address of the zero-knowledge verifier contract.
    /// * `ip_registry` - The address of the IP registry contract.
    /// 
    /// # Returns
    /// Returns the unique `u64` identifier for the newly created swap.
    /// 
    /// # Panics
    /// * Panics if the caller is not the `buyer`.
    /// * Panics if the `seller` does not own the `listing_id` in the `ip_registry`.
    /// * Panics if the token transfer fails.
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

        let active_listing_key = DataKey::ActiveListingSwap(listing_id);
        if let Some(existing_swap_id) = env.storage().persistent().get::<DataKey, u64>(&active_listing_key) {
            let existing_swap: Swap = env.storage().persistent().get(&DataKey::Swap(existing_swap_id)).unwrap();
            assert!(existing_swap.status != SwapStatus::Pending, "swap already pending for this listing");
        }

        // Verify seller owns the listing in ip_registry
        let listing = IpRegistryClient::new(&env, &ip_registry).get_listing(&listing_id);
        assert!(listing.owner == seller, "seller does not own this listing");

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
            &Swap { listing_id, buyer: buyer.clone(), seller, usdc_amount, usdc_token, zk_verifier, status: SwapStatus::Pending, decryption_key: None },
        );
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage().persistent().set(&active_listing_key, &id);
        env.storage().persistent().extend_ttl(&active_listing_key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        // Maintain buyer index
        let buyer_key = DataKey::BuyerIndex(buyer.clone());
        let mut ids: soroban_sdk::Vec<u64> = env
            .storage()
            .persistent()
            .get(&buyer_key)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(&buyer_key, &ids);
        env.storage().persistent().extend_ttl(&buyer_key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

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
        assert!(!decryption_key.is_empty(), "{:?}", ContractError::EmptyDecryptionKey);
        let key = DataKey::Swap(swap_id);
        let mut swap: Swap = env.storage().persistent().get(&key).expect("swap not found");
        assert!(swap.status == SwapStatus::Pending, "swap not pending");
        swap.seller.require_auth();

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

        swap.status = SwapStatus::Completed;
        swap.decryption_key = Some(decryption_key);
        env.storage().persistent().set(&key, &swap);
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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
        let mut swap: Swap = env.storage().persistent().get(&key).expect("swap not found");
        assert!(swap.status == SwapStatus::Pending, "swap not pending");
        swap.buyer.require_auth();
        token::Client::new(&env, &swap.usdc_token).transfer(
            &env.current_contract_address(),
            &swap.buyer,
            &swap.usdc_amount,
        );
        swap.status = SwapStatus::Cancelled;
        env.storage().persistent().set(&key, &swap);
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage().instance().extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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
}

#[cfg(test)]
mod test {
    use super::*;
    use ip_registry::{IpRegistry, IpRegistryClient};
    use soroban_sdk::{testutils::Address as _, token, Bytes, Env};

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

    #[test]
    fn test_get_swap_status_returns_none_for_missing_swap() {
        let env = Env::default();
        let contract_id = env.register(AtomicSwap, ());
        let client = AtomicSwapClient::new(&env, &contract_id);
        assert_eq!(client.get_swap_status(&999), None);
    }

    #[test]
    #[should_panic(expected = "EmptyDecryptionKey")]
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
        client.initialize(&Address::generate(&env), &100u32, &fee_recipient);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);

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
        client.initialize(&Address::generate(&env), &250u32, &fee_recipient);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &10_000, &zk_verifier, &registry_id);
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

        client.initialize(&Address::generate(&env), &0u32, &fee_recipient);

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &1000, &zk_verifier, &registry_id);
        client.confirm_swap(&swap_id, &Bytes::from_slice(&env, b"key"));

        assert_eq!(usdc_client.balance(&seller), 1000);
        assert_eq!(usdc_client.balance(&fee_recipient), 0);
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
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

        client.initialize(&admin, &0u32, &Address::generate(&env));
        client.pause();

        client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
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

        client.initialize(&admin, &0u32, &Address::generate(&env));
        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);

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

        client.initialize(&admin, &0u32, &Address::generate(&env));
        client.pause();
        client.unpause();

        let swap_id = client.initiate_swap(&listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
        assert_eq!(client.get_swap_status(&swap_id), Some(SwapStatus::Pending));
    }

    #[test]
    #[should_panic(expected = "swap already pending for this listing")]
    fn test_duplicate_swap_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let usdc_admin = Address::generate(&env);
        let usdc_id = env.register_stellar_asset_contract_v2(usdc_admin.clone()).address();

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

        // buyer1 initiates
        client.initiate_swap(&listing_id, &buyer1, &seller, &usdc_id, &500, &zk_verifier, &registry_id);

        // buyer2 initiates, should panic
        client.initiate_swap(&listing_id, &buyer2, &seller, &usdc_id, &500, &zk_verifier, &registry_id);
    }

    #[test]
    #[should_panic(expected = "seller does not own this listing")]
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

        client.initiate_swap(&listing_id, &buyer, &impersonator, &usdc_id, &500, &zk_verifier, &registry_id);
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

        let (usdc_id, listing_id, registry_id, _contract_id, client) =
            setup_swap_env(&env, &buyer, &seller, 500);

        let swap_id = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
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

        let id1 = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        let id2 = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        let id3 = client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
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
            &listing_id_a, &buyer_a, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        let id_b = client.initiate_swap(
            &listing_id_b, &buyer_b, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
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

        client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
        );
        client.initiate_swap(
            &listing_id, &buyer, &seller, &usdc_id, &500, &zk_verifier, &registry_id,
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
}
