#![no_std]
use soroban_sdk::{
    contract, contracterror, contractclient, contractevent, contractimpl, contracttype,
    panic_with_error, Address, Bytes, Env, Vec,
};

/// Entry for batch IP registration: (ipfs_hash, merkle_root)
pub type IpEntry = (Bytes, Bytes);

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidInput = 1,
    CounterOverflow = 2,
    ListingNotFound = 3,
    PendingSwapExists = 4,
    Unauthorized = 5,
}

/// Minimal interface to check for a pending swap on a listing.
#[contractclient(name = "AtomicSwapClient")]
pub trait AtomicSwapInterface {
    fn has_pending_swap(env: Env, listing_id: u64) -> bool;
}

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;

#[contracttype]
#[derive(Clone)]
pub struct Listing {
    pub owner: Address,
    pub ipfs_hash: Bytes,
    pub merkle_root: Bytes,
    pub royalty_bps: u32,
    pub royalty_recipient: Address,
    /// Seller-set price in USDC (smallest unit). 0 = no minimum enforced.
    pub price_usdc: i128,
}

#[contracttype]
pub enum DataKey {
    Listing(u64),
    /// Counter is in persistent storage to survive instance TTL resets
    /// and prevent listing ID collisions.
    Counter,
    OwnerIndex(Address),
}

/// Emitted when an IP listing is deregistered.
#[contractevent]
pub struct IpDeregistered {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
}

/// Emitted when a new IP listing is registered.
#[contractevent]
pub struct IpRegistered {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
    pub ipfs_hash: Bytes,
    pub merkle_root: Bytes,
}

#[contractevent]
pub struct BatchIpRegistered {
    #[topic]
    pub owner: Address,
    pub listing_ids: Vec<u64>,
    pub ipfs_hashes: Vec<Bytes>,
    pub merkle_roots: Vec<Bytes>,
}

#[contract]
pub struct IpRegistry;

#[contractimpl]
impl IpRegistry {
    pub fn register_ip(
        env: Env,
        owner: Address,
        ipfs_hash: Bytes,
        merkle_root: Bytes,
        royalty_bps: u32,
        royalty_recipient: Address,
        price_usdc: i128,
    ) -> Result<u64, ContractError> {
        if ipfs_hash.is_empty() || merkle_root.is_empty() || price_usdc < 0 || royalty_bps > 10_000 {
            return Err(ContractError::InvalidInput);
        }
        owner.require_auth();

        let prev: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0);
        let id: u64 = prev
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CounterOverflow));
        env.storage().persistent().set(&DataKey::Counter, &id);
        env.storage().persistent().extend_ttl(
            &DataKey::Counter,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        let key = DataKey::Listing(id);
        env.storage().persistent().set(
            &key,
            &Listing {
                owner: owner.clone(),
                ipfs_hash: ipfs_hash.clone(),
                merkle_root: merkle_root.clone(),
                royalty_bps,
                royalty_recipient: royalty_recipient.clone(),
                price_usdc,
            },
        );
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        let idx_key = DataKey::OwnerIndex(owner.clone());
        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or_else(|| Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(&idx_key, &ids);
        env.storage().persistent().extend_ttl(
            &idx_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        IpRegistered {
            listing_id: id,
            owner,
            ipfs_hash,
            merkle_root,
        }
        .publish(&env);

        Ok(id)
    }

    pub fn batch_register_ip(env: Env, owner: Address, entries: Vec<IpEntry>) -> Vec<u64> {
        let mut i: u32 = 0;
        while i < entries.len() {
            let (ipfs_hash, merkle_root) = entries.get(i).unwrap();
            if ipfs_hash.is_empty() || merkle_root.is_empty() {
                panic_with_error!(&env, ContractError::InvalidInput);
            }
            i += 1;
        }

        owner.require_auth();

        let mut listing_ids: Vec<u64> = Vec::new(&env);
        let mut ipfs_hashes: Vec<Bytes> = Vec::new(&env);
        let mut merkle_roots: Vec<Bytes> = Vec::new(&env);

        let mut j: u32 = 0;
        while j < entries.len() {
            let (ipfs_hash, merkle_root) = entries.get(j).unwrap();

            let prev: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::Counter)
                .unwrap_or(0);
            let id: u64 = prev
                .checked_add(1)
                .unwrap_or_else(|| panic_with_error!(&env, ContractError::CounterOverflow));
            env.storage().persistent().set(&DataKey::Counter, &id);
            env.storage().persistent().extend_ttl(
                &DataKey::Counter,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );

            let key = DataKey::Listing(id);
            env.storage().persistent().set(
                &key,
                &Listing {
                    owner: owner.clone(),
                    ipfs_hash: ipfs_hash.clone(),
                    merkle_root: merkle_root.clone(),
                    royalty_bps: 0,
                    royalty_recipient: owner.clone(),
                    price_usdc: 0,
                },
            );
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );

            let idx_key = DataKey::OwnerIndex(owner.clone());
            let mut ids: Vec<u64> = env
                .storage()
                .persistent()
                .get(&idx_key)
                .unwrap_or_else(|| Vec::new(&env));
            ids.push_back(id);
            env.storage().persistent().set(&idx_key, &ids);
            env.storage().persistent().extend_ttl(
                &idx_key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );

            listing_ids.push_back(id);
            ipfs_hashes.push_back(ipfs_hash.clone());
            merkle_roots.push_back(merkle_root.clone());

            IpRegistered {
                listing_id: id,
                owner: owner.clone(),
                ipfs_hash,
                merkle_root,
            }
            .publish(&env);

            j += 1;
        }

        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

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
            env.storage()
                .persistent()
                .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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

    /// Update ipfs_hash and/or merkle_root of an existing listing.
    /// Requires owner auth. Rejects if a pending swap exists for the listing.
    pub fn update_listing(
        env: Env,
        listing_id: u64,
        new_ipfs_hash: Bytes,
        new_merkle_root: Bytes,
        atomic_swap: Option<Address>,
    ) {
        if new_ipfs_hash.is_empty() || new_merkle_root.is_empty() {
            panic_with_error!(&env, ContractError::InvalidInput);
        }
        let key = DataKey::Listing(listing_id);
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::ListingNotFound));

        listing.owner.require_auth();

        if let Some(swap_addr) = atomic_swap {
            if AtomicSwapClient::new(&env, &swap_addr).has_pending_swap(&listing_id) {
                panic_with_error!(&env, ContractError::PendingSwapExists);
            }
        }

        listing.ipfs_hash = new_ipfs_hash;
        listing.merkle_root = new_merkle_root;
        env.storage().persistent().set(&key, &listing);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
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
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        token, Env, Event,
    };

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

    #[test]
    fn test_register_and_get() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmTestHash", b"merkle_root", 1000);
        assert_eq!(id, 1);
        let listing = client.get_listing(&id).expect("listing should exist");
        assert_eq!(listing.owner, owner);
        assert_eq!(listing.price_usdc, 1000);
    }

    #[test]
    fn test_get_listing_missing_returns_none() {
        let env = Env::default();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        assert!(client.get_listing(&999).is_none());
    }

    #[test]
    fn test_register_rejects_empty_ipfs_hash() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
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
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
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
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
            &0u32,
            &owner,
            &-1i128,
        );
        assert_eq!(result, Err(Ok(ContractError::InvalidInput)));
    }

    #[test]
    fn test_listing_count() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        assert_eq!(client.listing_count(), 0);
        let owner = Address::generate(&env);
        register(&client, &owner, b"QmHash1", b"root1", 0);
        assert_eq!(client.listing_count(), 1);
        register(&client, &owner, b"QmHash2", b"root2", 0);
        assert_eq!(client.listing_count(), 2);
    }

    #[test]
    fn test_owner_index() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let id1 = register(&client, &owner_a, b"QmHash1", b"root1", 0);
        let id2 = register(&client, &owner_b, b"QmHash2", b"root2", 0);
        let id3 = register(&client, &owner_a, b"QmHash3", b"root3", 0);
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
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 0);
        env.ledger().with_mut(|li| li.sequence_number += 5_000);
        assert!(client.get_listing(&id).is_some());
    }

    #[test]
    fn test_counter_persists_across_ttl_boundary() {
        // Counter is in persistent storage — must not reset after instance TTL expires.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);

        let id1 = register(&client, &owner, b"QmHash1", b"root1", 0);
        let id2 = register(&client, &owner, b"QmHash2", b"root2", 0);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Advance past instance TTL
        env.ledger().with_mut(|li| li.sequence_number += 6_400_000);

        // Counter must continue from 2, not reset to 0
        let id3 = register(&client, &owner, b"QmHash3", b"root3", 0);
        assert_eq!(id3, 3, "Counter reset after TTL — ID collision risk");

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
        assert_eq!(client.listing_count(), 3);
    }

    #[test]
    fn test_listing_ids_unique_after_many_registrations() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let mut seen: Vec<u64> = Vec::new(&env);

        let mut i: u32 = 0;
        while i < 20 {
            let id = register(&client, &owner, b"QmHash", b"root", 0);
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
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash1"),
            Bytes::from_slice(&env, b"root1"),
        ));
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash2"),
            Bytes::from_slice(&env, b"root2"),
        ));
        let ids = client.batch_register_ip(&owner, &entries);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), 1);
        assert_eq!(ids.get(1).unwrap(), 2);
        assert_eq!(client.list_by_owner(&owner).len(), 2);
    }

    #[test]
    fn test_batch_register_ip_empty_list() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let entries: Vec<IpEntry> = Vec::new(&env);
        let ids = client.batch_register_ip(&owner, &entries);
        assert_eq!(ids.len(), 0);
        assert_eq!(client.listing_count(), 0);
    }

    #[test]
    fn test_batch_register_ip_rejects_empty_ipfs_hash() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((Bytes::new(&env), Bytes::from_slice(&env, b"root")));
        assert!(client.try_batch_register_ip(&owner, &entries).is_err());
    }

    #[test]
    fn test_batch_register_ip_atomic_failure() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let mut entries: Vec<IpEntry> = Vec::new(&env);
        entries.push_back((
            Bytes::from_slice(&env, b"QmHash1"),
            Bytes::from_slice(&env, b"root1"),
        ));
        entries.push_back((Bytes::new(&env), Bytes::from_slice(&env, b"root2")));
        assert!(client.try_batch_register_ip(&owner, &entries).is_err());
        assert_eq!(client.listing_count(), 0);
    }

    #[test]
    fn test_deregister_listing_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 0);

        client.deregister_listing(&owner, &id);

        assert!(client.get_listing(&id).is_none());
        assert_eq!(client.list_by_owner(&owner).len(), 0);
    }

    #[test]
    fn test_deregister_listing_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let id = register(&client, &owner, b"QmHash", b"root", 0);

        let result = client.try_deregister_listing(&attacker, &id);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
        assert!(client.get_listing(&id).is_some());
    }

    #[test]
    fn test_update_listing_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmOld"),
            &Bytes::from_slice(&env, b"old_root"),
        );
        client.update_listing(
            &owner,
            &id,
            &Bytes::from_slice(&env, b"QmNew"),
            &Bytes::from_slice(&env, b"new_root"),
        );
        let listing = client.get_listing(&id).unwrap();
        assert_eq!(listing.ipfs_hash, Bytes::from_slice(&env, b"QmNew"));
        assert_eq!(listing.merkle_root, Bytes::from_slice(&env, b"new_root"));
    }

    #[test]
    fn test_update_listing_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );
        let result = client.try_update_listing(
            &attacker,
            &id,
            &Bytes::from_slice(&env, b"QmNew"),
            &Bytes::from_slice(&env, b"new_root"),
        );
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    }

    #[test]
    fn test_update_listing_not_found() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let result = client.try_update_listing(
            &owner,
            &999u64,
            &Bytes::from_slice(&env, b"QmNew"),
            &Bytes::from_slice(&env, b"new_root"),
        );
        assert_eq!(result, Err(Ok(ContractError::ListingNotFound)));
    }

    #[test]
    fn test_deregister_listing_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );
        client.deregister_listing(&owner, &id);
        assert!(client.get_listing(&id).is_none());
        assert_eq!(client.list_by_owner(&owner).len(), 0);
    }

    #[test]
    fn test_deregister_listing_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );
        let result = client.try_deregister_listing(&attacker, &id);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    }

    #[test]
    fn test_transfer_ownership_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );
        client.transfer_ownership(&owner, &id, &new_owner);
        let listing = client.get_listing(&id).unwrap();
        assert_eq!(listing.owner, new_owner);
        assert_eq!(client.list_by_owner(&owner).len(), 0);
        assert_eq!(client.list_by_owner(&new_owner).len(), 1);
    }

    #[test]
    fn test_transfer_ownership_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let id = client.register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );
        let result = client.try_transfer_ownership(&attacker, &id, &new_owner);
        assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
    }

    #[test]
    fn test_list_by_owner_page() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let h = Bytes::from_slice(&env, b"h");
        let r = Bytes::from_slice(&env, b"r");
        let id1 = client.register_ip(&owner, &h, &r);
        let id2 = client.register_ip(&owner, &h, &r);
        let id3 = client.register_ip(&owner, &h, &r);
        let page = client.list_by_owner_page(&owner, &0u32, &2u32);
        assert_eq!(page.len(), 2);
        assert_eq!(page.get(0).unwrap(), id1);
        assert_eq!(page.get(1).unwrap(), id2);
        let page2 = client.list_by_owner_page(&owner, &2u32, &2u32);
        assert_eq!(page2.len(), 1);
        assert_eq!(page2.get(0).unwrap(), id3);
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
}
