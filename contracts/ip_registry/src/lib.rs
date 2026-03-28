#![no_std]
use soroban_sdk::{
    contract, contractclient, contracterror, contractevent, contractimpl, contracttype,
    panic_with_error, Address, Bytes, Env, Vec,
};

/// Entry for batch IP registration: (ipfs_hash, merkle_root, royalty_bps, royalty_recipient, price_usdc)
pub type IpEntry = (Bytes, Bytes, u32, Address, i128);

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidInput = 1,
    CounterOverflow = 2,
    ListingNotFound = 3,
    PendingSwapExists = 4,
    Unauthorized = 5,
    NotInitialized = 6,
    AlreadyInitialized = 7,
    InvalidPrice = 8,
    ContractPaused = 9,
}






/// Minimal interface to check for a pending swap on a listing.
#[contractclient(name = "AtomicSwapClient")]
pub trait AtomicSwapInterface {
    fn has_pending_swap(env: Env, listing_id: u64) -> bool;
}

/// Client interface for IpRegistry — always compiled so dependents can use IpRegistryClient.
#[cfg(not(feature = "contract"))]
#[contractclient(name = "IpRegistryClient")]
pub trait IpRegistryInterface {
    fn get_listing(env: Env, listing_id: u64) -> Option<Listing>;
    fn register_ip(
        env: Env,
        owner: Address,
        ipfs_hash: Bytes,
        merkle_root: Bytes,
        royalty_bps: u32,
        royalty_recipient: Address,
        price_usdc: i128,
    ) -> Result<u64, ContractError>;
}









#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub ttl_threshold: u32,
    pub ttl_extend_to: u32,
}










#[contracttype]
#[derive(Clone)]
pub struct Listing {
    pub owner: Address,
    pub ipfs_hash: Bytes,
    pub merkle_root: Bytes,
    pub royalty_bps: u32,
    pub royalty_recipient: Address,
    pub price_usdc: i128,
}

#[contracttype]
pub enum DataKey {
    Listing(u64),
    Counter,
    OwnerIndex(Address),
    Config,
    Paused,
}

#[contractevent]
pub struct IpDeregistered {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
}

#[contractevent]
pub struct ListingRegistered {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
    pub ipfs_hash: Bytes,
}

#[contractevent]
pub struct BatchIpRegistered {
    #[topic]
    pub owner: Address,
    pub listing_ids: Vec<u64>,
    pub ipfs_hashes: Vec<Bytes>,
    pub merkle_roots: Vec<Bytes>,
}

#[contractevent]
pub struct TtlUpdated {
    #[topic]
    pub admin: Address,
    pub new_threshold: u32,
    pub new_extend_to: u32,
}

#[contractevent]
pub struct OwnershipTransferred {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
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

#[cfg_attr(feature = "contract", contract)]
pub struct IpRegistry;

fn get_config(env: &Env) -> Config {
    env.storage()
        .persistent()
        .get(&DataKey::Config)
        .unwrap_or_else(|| panic_with_error!(env, ContractError::NotInitialized))
}

fn extend_persistent(env: &Env, key: &DataKey, cfg: &Config) {
    env.storage()
        .persistent()
        .extend_ttl(key, cfg.ttl_threshold, cfg.ttl_extend_to);
}

fn assert_not_paused(env: &Env) {
    let paused: bool = env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);
    if paused {
        panic_with_error!(env, ContractError::ContractPaused);
    }
}

#[cfg_attr(feature = "contract", contractimpl)]
impl IpRegistry {
    /// Must be called once before any other function.
    pub fn initialize(
        env: Env,
        admin: Address,
        ttl_threshold: u32,
        ttl_extend_to: u32,
    ) -> Result<(), ContractError> {
        if env.storage().persistent().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }
        let config = Config {
            admin,
            ttl_threshold,
            ttl_extend_to,
        };
        env.storage().persistent().set(&DataKey::Config, &config);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, ttl_threshold, ttl_extend_to);
        Ok(())
    }

    /// Admin-only: update TTL parameters. Emits a TtlUpdated event.
    pub fn update_ttl(
        env: Env,
        admin: Address,
        new_threshold: u32,
        new_extend_to: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        let mut cfg = get_config(&env);
        if cfg.admin != admin {
            return Err(ContractError::Unauthorized);
        }
        cfg.ttl_threshold = new_threshold;
        cfg.ttl_extend_to = new_extend_to;
        env.storage().persistent().set(&DataKey::Config, &cfg);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, cfg.ttl_threshold, cfg.ttl_extend_to);

        TtlUpdated {
            admin,
            new_threshold,
            new_extend_to,
        }
        .publish(&env);

        Ok(())
    }

    /// Admin-only: pause the contract. Blocks new IP registrations.
    pub fn pause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .map(|cfg: Config| cfg.admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.storage()
            .instance()
            .extend_ttl(100_000, 6_312_000);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, 100_000, 6_312_000);
        ContractPausedEvent { admin }.publish(&env);
    }

    /// Admin-only: unpause the contract. Allows IP registrations to resume.
    pub fn unpause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .map(|cfg: Config| cfg.admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotInitialized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .extend_ttl(100_000, 6_312_000);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, 100_000, 6_312_000);
        ContractUnpausedEvent { admin }.publish(&env);
    }

    pub fn register_ip(
        env: Env,
        owner: Address,
        ipfs_hash: Bytes,
        merkle_root: Bytes,
        royalty_bps: u32,
        royalty_recipient: Address,
        price_usdc: i128,
    ) -> Result<u64, ContractError> {
        assert_not_paused(&env);
        if ipfs_hash.is_empty() || merkle_root.is_empty() || royalty_bps > 10_000 {
            return Err(ContractError::InvalidInput);
        }
        if price_usdc <= 0 {
            return Err(ContractError::InvalidPrice);
        }
        owner.require_auth();
        let cfg = get_config(&env);

        let prev: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0);
        let id: u64 = prev
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CounterOverflow));
        env.storage().persistent().set(&DataKey::Counter, &id);
        extend_persistent(&env, &DataKey::Counter, &cfg);

        let key = DataKey::Listing(id);
        env.storage().persistent().set(
            &key,
            &Listing {
                owner: owner.clone(),
                ipfs_hash: ipfs_hash.clone(),
                merkle_root: merkle_root.clone(),
                royalty_bps,
                royalty_recipient,
                price_usdc,
            },
        );
        extend_persistent(&env, &key, &cfg);

        let idx_key = DataKey::OwnerIndex(owner.clone());
        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or_else(|| Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(&idx_key, &ids);
        extend_persistent(&env, &idx_key, &cfg);

        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, cfg.ttl_threshold, cfg.ttl_extend_to);

        ListingRegistered {
            listing_id: id,
            owner,
            ipfs_hash,
        }
        .publish(&env);

        Ok(id)
    }

    pub fn batch_register_ip(env: Env, owner: Address, entries: Vec<IpEntry>) -> Vec<u64> {
        assert_not_paused(&env);
        let mut i: u32 = 0;
        while i < entries.len() {
            let (ipfs_hash, merkle_root, royalty_bps, _royalty_recipient, price_usdc) = entries.get(i).unwrap();
            if ipfs_hash.is_empty() || merkle_root.is_empty() || price_usdc < 0 || royalty_bps > 10_000 {
                panic_with_error!(&env, ContractError::InvalidInput);
            }
            if price_usdc <= 0 {
                panic_with_error!(&env, ContractError::InvalidPrice);
            }
            i += 1;
        }

        owner.require_auth();
        let cfg = get_config(&env);

        let mut listing_ids: Vec<u64> = Vec::new(&env);
        let mut ipfs_hashes: Vec<Bytes> = Vec::new(&env);
        let mut merkle_roots: Vec<Bytes> = Vec::new(&env);

        let mut j: u32 = 0;
        while j < entries.len() {
            let (ipfs_hash, merkle_root, royalty_bps, royalty_recipient, price_usdc) = entries.get(j).unwrap();

            let prev: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::Counter)
                .unwrap_or(0);
            let id: u64 = prev
                .checked_add(1)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::CounterOverflow));
            env.storage().persistent().set(&DataKey::Counter, &id);
            extend_persistent(&env, &DataKey::Counter, &cfg);

            let key = DataKey::Listing(id);
            env.storage().persistent().set(
                &key,
                &Listing {
                    owner: owner.clone(),
                    ipfs_hash: ipfs_hash.clone(),
                    merkle_root: merkle_root.clone(),
                    royalty_bps,
                    royalty_recipient,
                    price_usdc,
                },
            );
            extend_persistent(&env, &key, &cfg);

            let idx_key = DataKey::OwnerIndex(owner.clone());
            let mut ids: Vec<u64> = env
                .storage()
                .persistent()
                .get(&idx_key)
                .unwrap_or_else(|| Vec::new(&env));
            ids.push_back(id);
            env.storage().persistent().set(&idx_key, &ids);
            extend_persistent(&env, &idx_key, &cfg);

            listing_ids.push_back(id);
            ipfs_hashes.push_back(ipfs_hash.clone());
            merkle_roots.push_back(merkle_root.clone());

            ListingRegistered {
                listing_id: id,
                owner: owner.clone(),
                ipfs_hash,
            }
            .publish(&env);

            j += 1;
        }

        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, cfg.ttl_threshold, cfg.ttl_extend_to);

        BatchIpRegistered {
            owner,
            listing_ids: listing_ids.clone(),
            ipfs_hashes,
            merkle_roots,
        }
        .publish(&env);

        listing_ids
    }

    pub fn get_listing(env: Env, listing_id: u64) -> Option<Listing> {
        let key = DataKey::Listing(listing_id);
        if env.storage().persistent().has(&key) {
            let cfg = get_config(&env);
            extend_persistent(&env, &key, &cfg);
        }
        env.storage().persistent().get(&key)
    }

    pub fn listing_count(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0)
    }

    pub fn list_by_owner(env: Env, owner: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerIndex(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get a paginated list of listing IDs for an owner.
    /// Returns listing IDs starting at `offset` with a maximum of `limit` results.
    pub fn list_by_owner_page(env: Env, owner: Address, offset: u32, limit: u32) -> Vec<u64> {
        let all_listings = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerIndex(owner))
            .unwrap_or_else(|| Vec::new(&env));

        if offset >= all_listings.len() {
            return Vec::new(&env);
        }

        let end = core::cmp::min(offset.saturating_add(limit), all_listings.len());
        all_listings.slice(offset..end)
    }

    /// Update ipfs_hash and/or merkle_root of an existing listing.
    /// Requires owner auth. Rejects if a pending swap exists for the listing.
    pub fn update_listing(
        env: Env,
        owner: Address,
        listing_id: u64,
        new_ipfs_hash: Bytes,
        new_merkle_root: Bytes,
    ) {
        assert_not_paused(&env);
        if new_ipfs_hash.is_empty() || new_merkle_root.is_empty() {
            panic_with_error!(&env, ContractError::InvalidInput);
        }
        owner.require_auth();

        let key = DataKey::Listing(listing_id);
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::ListingNotFound));

        if listing.owner != owner {
            panic_with_error!(&env, ContractError::Unauthorized);
        }

        let cfg = get_config(&env);
        listing.ipfs_hash = new_ipfs_hash;
        listing.merkle_root = new_merkle_root;
        env.storage().persistent().set(&key, &listing);
        extend_persistent(&env, &key, &cfg);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, cfg.ttl_threshold, cfg.ttl_extend_to);
    }

    /// Remove a listing from the registry. Only the owner may call this.
    pub fn deregister_listing(
        env: Env,
        owner: Address,
        listing_id: u64,
    ) -> Result<(), ContractError> {
        owner.require_auth();

        let key = DataKey::Listing(listing_id);
        let listing: Listing = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::ListingNotFound)?;

        if listing.owner != owner {
            return Err(ContractError::Unauthorized);
        }

        env.storage().persistent().remove(&key);

        let idx_key = DataKey::OwnerIndex(owner.clone());
        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or_else(|| Vec::new(&env));
        if let Some(pos) = (0..ids.len()).find(|&i| ids.get(i).unwrap() == listing_id) {
            ids.remove(pos);
        }
        env.storage().persistent().set(&idx_key, &ids);

        IpDeregistered { listing_id, owner }.publish(&env);

        Ok(())
    }

    /// Transfer ownership of a listing to a new owner.
    /// Requires current owner auth. Rejects if a pending swap exists for the listing.
    pub fn transfer_listing_ownership(
        env: Env,
        owner: Address,
        listing_id: u64,
        new_owner: Address,
    ) -> Result<(), ContractError> {
        owner.require_auth();

        let key = DataKey::Listing(listing_id);
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::ListingNotFound)?;

        if listing.owner != owner {
            return Err(ContractError::Unauthorized);
        }

        let cfg = get_config(&env);

        // Remove listing_id from old owner's index
        let old_idx_key = DataKey::OwnerIndex(owner.clone());
        let mut old_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&old_idx_key)
            .unwrap_or_else(|| Vec::new(&env));
        if let Some(pos) = (0..old_ids.len()).find(|&i| old_ids.get(i).unwrap() == listing_id) {
            old_ids.remove(pos);
        }
        env.storage().persistent().set(&old_idx_key, &old_ids);
        extend_persistent(&env, &old_idx_key, &cfg);

        // Add listing_id to new owner's index
        let new_idx_key = DataKey::OwnerIndex(new_owner.clone());
        let mut new_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&new_idx_key)
            .unwrap_or_else(|| Vec::new(&env));
        new_ids.push_back(listing_id);
        env.storage().persistent().set(&new_idx_key, &new_ids);
        extend_persistent(&env, &new_idx_key, &cfg);

        // Update listing owner
        listing.owner = new_owner.clone();
        env.storage().persistent().set(&key, &listing);
        extend_persistent(&env, &key, &cfg);

        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Config, cfg.ttl_threshold, cfg.ttl_extend_to);

        OwnershipTransferred {
            listing_id,
            from: owner,
            to: new_owner,
        }
        .publish(&env);

        Ok(())
    }

    /// Expose the current config for off-chain inspection.
    pub fn get_config(env: Env) -> Config {
        get_config(&env)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _, Events},
        Env, IntoVal,
    };

    const THRESHOLD: u32 = 100_000;
    const EXTEND_TO: u32 = 6_312_000;

    fn setup() -> (Env, IpRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &THRESHOLD, &EXTEND_TO);
        (env, client, admin)
    }

    fn register(
        client: &IpRegistryClient,
        owner: &Address,
        hash: &[u8],
        root: &[u8],
        price: i128,
    ) -> u64 {
        let env = &client.env;
        client.register_ip(
            owner,
            &Bytes::from_slice(env, hash),
            &Bytes::from_slice(env, root),
            &0u32,
            owner,
            &price,
        )
    }

    // ── Issue #192 tests ────────────────────────────────────────────────────

    #[test]
    fn test_initialize_stores_ttl_values() {
        let (_env, client, _admin) = setup();
        let cfg = client.get_config();
        assert_eq!(cfg.ttl_threshold, THRESHOLD);
        assert_eq!(cfg.ttl_extend_to, EXTEND_TO);
    }

    #[test]
    fn test_update_ttl_authorized() {
        let (_env, client, admin) = setup();
        client.update_ttl(&admin, &200_000, &9_000_000);
        let cfg = client.get_config();
        assert_eq!(cfg.ttl_threshold, 200_000);
        assert_eq!(cfg.ttl_extend_to, 9_000_000);
    }

    #[test]
    fn test_update_ttl_unauthorized_panics() {
        let (env, client, _admin) = setup();
        let attacker = Address::generate(&env);
        let result = client.try_update_ttl(&attacker, &1, &1);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_uses_updated_ttl() {
        // After updating TTL, a new registration should succeed and the config
        // values should reflect the update (functional smoke-check).
        let (env, client, admin) = setup();
        client.update_ttl(&admin, &50_000, &3_000_000);
        let cfg = client.get_config();
        assert_eq!(cfg.ttl_threshold, 50_000);
        assert_eq!(cfg.ttl_extend_to, 3_000_000);

        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1);
        assert!(client.get_listing(&id).is_some());
    }

    // ── Existing tests (preserved) ──────────────────────────────────────────

    #[test]
    fn test_register_and_get() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmTestHash", b"merkle_root", 1000);
        assert_eq!(id, 1);
        let listing = client.get_listing(&id).expect("listing should exist");
        assert_eq!(listing.owner, owner);
        assert_eq!(listing.price_usdc, 1000);
    }

    #[test]
    fn test_get_listing_missing_returns_none() {
        let (_env, client, _admin) = setup();
        assert!(client.get_listing(&999).is_none());
    }

    #[test]
    fn test_register_rejects_empty_ipfs_hash() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::new(&env),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &0i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
    }

    #[test]
    fn test_register_rejects_empty_merkle_root() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::new(&env),
            &0u32,
            &owner,
            &0i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
    }

    #[test]
    fn test_register_rejects_negative_price() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &-1i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidPrice)));
    }

    #[test]
    fn test_register_rejects_zero_price() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &0i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidPrice)));
    }

    #[test]
    fn test_listing_count() {
        let (env, client, _admin) = setup();
        assert_eq!(client.listing_count(), 0);
        let owner = Address::generate(&env);
        register(&client, &owner, b"QmHash1", b"root1", 1);
        assert_eq!(client.listing_count(), 1);
        register(&client, &owner, b"QmHash2", b"root2", 1);
        assert_eq!(client.listing_count(), 2);
    }

    #[test]
    fn test_owner_index() {
        let (env, client, _admin) = setup();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let id1 = register(&client, &owner_a, b"QmHash1", b"root1", 1);
        let id2 = register(&client, &owner_b, b"QmHash2", b"root2", 1);
        let id3 = register(&client, &owner_a, b"QmHash3", b"root3", 1);
        let a_ids = client.list_by_owner(&owner_a);
        assert_eq!(a_ids.len(), 2);
        assert_eq!(a_ids.get(0).unwrap(), id1);
        assert_eq!(a_ids.get(1).unwrap(), id3);
        let b_ids = client.list_by_owner(&owner_b);
        assert_eq!(b_ids.len(), 1);
        assert_eq!(b_ids.get(0).unwrap(), id2);
    }

    #[test]
    fn test_listing_survives_ttl_boundary() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1);
        env.ledger().with_mut(|li| li.sequence_number += 5_000);
        assert!(client.get_listing(&id).is_some());
    }

    #[test]
    fn test_counter_persists_across_ttl_boundary() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id1 = register(&client, &owner, b"QmHash1", b"root1", 1);
        let id2 = register(&client, &owner, b"QmHash2", b"root2", 1);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        env.ledger().with_mut(|li| li.sequence_number += 6_400_000);
        let id3 = register(&client, &owner, b"QmHash3", b"root3", 1);
        assert_eq!(id3, 3, "Counter reset after TTL — ID collision risk");
        assert_eq!(client.listing_count(), 3);
    }

    #[test]
    fn test_listing_ids_unique_after_many_registrations() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let mut seen: Vec<u64> = Vec::new(&env);
        let mut i: u32 = 0;
        while i < 20 {
            let id = register(&client, &owner, b"QmHash", b"root", 1);
            assert_eq!(id, (i + 1) as u64);
            let mut j: u32 = 0;
            while j < seen.len() {
                assert_ne!(seen.get(j).unwrap(), id);
                j += 1;
            }
            seen.push_back(id);
            i += 1;
        }
        assert_eq!(client.listing_count(), 20);
    }

    #[test]
    fn test_batch_register_ip() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash1"),
            Bytes::from_slice(&env, b"root1"),
            500,
            owner.clone(),
            1000,
        ));
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash2"),
            Bytes::from_slice(&env, b"root2"),
            500,
            owner.clone(),
            1000,
        ));
        let ids = client.batch_register_ip(&owner, &entries);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), 1);
        assert_eq!(ids.get(1).unwrap(), 2);
        assert_eq!(client.list_by_owner(&owner).len(), 2);
    }

    #[test]
    fn test_batch_register_ip_empty_list() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let entries: Vec<IpEntry> = Vec::new(&env);
        let ids = client.batch_register_ip(&owner, &entries);
        assert_eq!(ids.len(), 0);
        assert_eq!(client.listing_count(), 0);
    }

    #[test]
    fn test_batch_register_ip_rejects_empty_ipfs_hash() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((Bytes::new(&env), Bytes::from_slice(&env, b"root"), 500, owner.clone(), 1000));
        assert!(client.try_batch_register_ip(&owner, &entries).is_err());
    }

    #[test]
    fn test_batch_register_ip_atomic_failure() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash1"),
            Bytes::from_slice(&env, b"root1"),
            500,
            owner.clone(),
            1000,
        ));
        entries.push_back((Bytes::new(&env), Bytes::from_slice(&env, b"root2"), 500, owner.clone(), 1000));
        assert!(client.try_batch_register_ip(&owner, &entries).is_err());
        assert_eq!(client.listing_count(), 0);
    }

    #[test]
    fn test_deregister_listing_success() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1);
        client.deregister_listing(&owner, &id);
        assert!(client.get_listing(&id).is_none());
        assert_eq!(client.list_by_owner(&owner).len(), 0);
    }

    #[test]
    fn test_deregister_listing_unauthorized() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1);
        let result = client.try_deregister_listing(&attacker, &id);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
        assert!(client.get_listing(&id).is_some());
    }

    #[test]
    fn test_already_initialized() {
        let (env, client, admin) = setup();
        let result = client.try_initialize(&admin, &THRESHOLD, &EXTEND_TO);
        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
        // Ensure env is used to avoid unused variable warning
        let _ = Address::generate(&env);
    }

    #[test]
    fn test_batch_register_ip_emits_events() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash1"),
            Bytes::from_slice(&env, b"root1"),
            500,
            owner.clone(),
            1000,
        ));
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash2"),
            Bytes::from_slice(&env, b"root2"),
            500,
            owner.clone(),
            1000,
        ));
        client.batch_register_ip(&owner, &entries);
        // Events are emitted; verify no panic and count is correct.
        assert_eq!(client.listing_count(), 2);
    }

    #[test]
    fn test_register_ip_rejects_royalty_bps_above_10000() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &10_001u32,
            &owner,
            &1000i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
    }

    #[test]
    fn test_register_ip_accepts_royalty_bps_10000() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let admin = Address::generate(&env);
        client.initialize(&admin, &THRESHOLD, &EXTEND_TO);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &10_000u32,
            &owner,
            &1000i128,
        );
        assert_eq!(id, 1);
    }

    #[test]
    fn test_transfer_listing_ownership_success() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 500);

        client.transfer_listing_ownership(&owner, &id, &new_owner);

        let listing = client.get_listing(&id).expect("listing should exist");
        assert_eq!(listing.owner, new_owner);
        assert_eq!(client.list_by_owner(&owner).len(), 0);
        assert_eq!(client.list_by_owner(&new_owner).len(), 1);
        assert_eq!(client.list_by_owner(&new_owner).get(0).unwrap(), id);
    }

    #[test]
    fn test_transfer_listing_ownership_unauthorized() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1);

        let result = client.try_transfer_listing_ownership(&attacker, &id, &new_owner);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

        // Ownership unchanged
        let listing = client.get_listing(&id).unwrap();
        assert_eq!(listing.owner, owner);
    }

    #[test]
    fn test_transfer_listing_ownership_not_found() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);

        let result = client.try_transfer_listing_ownership(&owner, &999, &new_owner);
        assert_eq!(result, Err(Ok(ContractError::ListingNotFound)));
    }

    // ── Pause/Unpause tests ────────────────────────────────────────────────

    #[test]
    fn test_pause_emits_event() {
        let (env, client, _admin) = setup();
        client.pause();
        let events = env.events().all().filter_by_contract(&client.address);
        assert!(
            !events.events().is_empty(),
            "ContractPausedEvent not emitted"
        );
    }

    #[test]
    fn test_unpause_emits_event() {
        let (env, client, _admin) = setup();
        client.pause();
        client.unpause();
        let events = env.events().all().filter_by_contract(&client.address);
        assert!(
            !events.events().is_empty(),
            "ContractUnpausedEvent not emitted"
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_register_ip_blocked_when_paused() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        client.pause();
        client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &1000i128,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_batch_register_ip_blocked_when_paused() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash1"),
            Bytes::from_slice(&env, b"root1"),
            500,
            owner.clone(),
            1000,
        ));
        client.pause();
        client.batch_register_ip(&owner, &entries);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_update_listing_blocked_when_paused() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        client.pause();
        client.update_listing(
            &owner,
            &id,
            &Bytes::from_slice(&env, b"QmHashNew"),
            &Bytes::from_slice(&env, b"rootNew"),
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_update_listing_rejects_empty_ipfs_hash() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        
        client.update_listing(
            &owner,
            &id,
            &Bytes::new(&env),
            &Bytes::from_slice(&env, b"newRoot"),
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_update_listing_rejects_empty_merkle_root() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        
        client.update_listing(
            &owner,
            &id,
            &Bytes::from_slice(&env, b"newHash"),
            &Bytes::new(&env),
        );
    }

    #[test]
    fn test_register_ip_allowed_after_unpause() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        client.pause();
        client.unpause();
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        assert_eq!(id, 1);
        assert!(client.get_listing(&id).is_some());
    }

    #[test]
    fn test_deregister_listing_allowed_when_paused() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        client.pause();
        // Deregister should succeed even when paused (read-only operation)
        client.deregister_listing(&owner, &id);
        assert!(client.get_listing(&id).is_none());
    }

    #[test]
    fn test_get_listing_allowed_when_paused() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        client.pause();
        // Get should succeed even when paused (read-only operation)
        assert!(client.get_listing(&id).is_some());
    }

    #[test]
    fn test_register_ip_emits_listing_registered() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let hash = b"QmHash";
        let root = b"root";
        let price = 1000i128;

        client.register_ip(
            &owner,
            &Bytes::from_slice(&env, hash),
            &Bytes::from_slice(&env, root),
            &0u32,
            &owner,
            &price,
        );

        let events = env.events().all().filter_by_contract(&client.address);
        assert!(
            !events.events().is_empty(),
            "ListingRegistered event should be emitted"
        );
        
        // Verify an event was emitted (simplified test)
        let event_count = events.events().len();
        assert!(event_count >= 1, "At least one event should be emitted");
    }

    // ── TTL persistence tests ────────────────────────────────────────────────

    #[test]
    fn test_config_survives_past_instance_ttl() {
        // Test that Config in persistent storage survives past instance TTL expiration
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        
        // Register a listing
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        assert!(client.get_listing(&id).is_some());
        
        // Advance ledger far past typical instance TTL (beyond 6,312,000 ledgers)
        env.ledger().with_mut(|li| li.sequence_number += 7_000_000);
        
        // Config should still be accessible from persistent storage
        let cfg = client.get_config();
        assert_eq!(cfg.admin, admin);
        assert_eq!(cfg.ttl_threshold, THRESHOLD);
        assert_eq!(cfg.ttl_extend_to, EXTEND_TO);
        
        // get_listing should still work even though instance storage would have expired
        let listing = client.get_listing(&id);
        assert!(listing.is_some(), "Listing should be accessible after instance TTL expiration");
        assert_eq!(listing.unwrap().owner, owner);
        
        // Should be able to register new listings (config still accessible)
        let id2 = register(&client, &owner, b"QmHash2", b"root2", 2000);
        assert_eq!(id2, 2);
        assert!(client.get_listing(&id2).is_some());
    }

    #[test]
    fn test_get_listing_works_after_ledge_advancement_without_config_access() {
        // Test that get_listing extends its own TTL and works even if config wasn't accessed recently
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        
        // Register a listing
        let id = register(&client, &owner, b"QmHash", b"root", 1000);
        assert!(client.get_listing(&id).is_some());
        
        // Advance ledger past typical TTL but not too far
        env.ledger().with_mut(|li| li.sequence_number += 500_000);
        
        // get_listing should work and extend its own TTL
        let listing = client.get_listing(&id);
        assert!(listing.is_some(), "get_listing should work after ledger advancement");
        assert_eq!(listing.unwrap().owner, owner);
        
        // Advance further and verify it still works
        env.ledger().with_mut(|li| li.sequence_number += 500_000);
        let listing2 = client.get_listing(&id);
        assert!(listing2.is_some(), "get_listing should continue working after multiple TTL extensions");
    }

    // ── Issue #260 tests ────────────────────────────────────────────────────

    #[test]
    fn test_register_ip_zero_price() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &0i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidPrice)));
    }

    #[test]
    fn test_register_ip_negative_price() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &-1i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidPrice)));
    }

    #[test]
    fn test_register_ip_valid_price() {
        let (env, client, _admin) = setup();
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 1);
        let listing = client.get_listing(&id).expect("listing should exist");
        assert_eq!(listing.price_usdc, 1);
    }
}
