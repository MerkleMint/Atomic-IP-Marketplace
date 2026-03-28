#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, Address,
    Bytes, Env, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidInput = 1,
    CounterOverflow = 2,
    ListingNotFound = 3,
    Unauthorized = 4,
}

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;

#[contracttype]
#[derive(Clone)]
pub struct Listing {
    pub owner: Address,
    pub ipfs_hash: Bytes,
    pub merkle_root: Bytes,
}

#[contracttype]
pub enum DataKey {
    Listing(u64),
    Counter,
    OwnerIndex(Address),
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

#[contract]
pub struct IpRegistry;

#[contractimpl]
impl IpRegistry {
    /// Register a new IP listing. Returns the listing ID.
    pub fn register_ip(env: Env, owner: Address, ipfs_hash: Bytes, merkle_root: Bytes) -> u64 {
        if ipfs_hash.is_empty() || merkle_root.is_empty() {
            panic_with_error!(&env, ContractError::InvalidInput);
        }
        owner.require_auth();
        let prev: u64 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0);
        let id: u64 = prev
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::CounterOverflow));
        env.storage().instance().set(&DataKey::Counter, &id);

        let key = DataKey::Listing(id);
        env.storage().persistent().set(
            &key,
            &Listing {
                owner: owner.clone(),
                ipfs_hash: ipfs_hash.clone(),
                merkle_root: merkle_root.clone(),
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

        id
    }

    /// Retrieves a specific IP listing by its ID.
    pub fn get_listing(env: Env, listing_id: u64) -> Option<Listing> {
        env.storage()
            .persistent()
            .get(&DataKey::Listing(listing_id))
    }

    /// Retrieves all listing IDs owned by a specific address.
    pub fn list_by_owner(env: Env, owner: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerIndex(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Transfer ownership of a listing to another address.
    pub fn transfer_listing(env: Env, listing_id: u64, new_owner: Address) {
        let key = DataKey::Listing(listing_id);
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::ListingNotFound));

        listing.owner.require_auth();
        let old_owner = listing.owner.clone();

        if old_owner == new_owner {
            return;
        }

        // Update listing owner
        listing.owner = new_owner.clone();
        env.storage().persistent().set(&key, &listing);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        // Update old owner index
        let old_idx_key = DataKey::OwnerIndex(old_owner.clone());
        let mut old_ids: Vec<u64> = env.storage().persistent().get(&old_idx_key).unwrap();
        if let Some(pos) = old_ids.first_index_of(listing_id) {
            old_ids.remove(pos);
        }
        env.storage().persistent().set(&old_idx_key, &old_ids);
        env.storage().persistent().extend_ttl(
            &old_idx_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Update new owner index
        let new_idx_key = DataKey::OwnerIndex(new_owner.clone());
        let mut new_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&new_idx_key)
            .unwrap_or_else(|| Vec::new(&env));
        new_ids.push_back(listing_id);
        env.storage().persistent().set(&new_idx_key, &new_ids);
        env.storage().persistent().extend_ttl(
            &new_idx_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );

        // Emit transfer event
        env.events().publish(
            (soroban_sdk::symbol_short!("transfer"), listing_id),
            (old_owner, new_owner),
        );

        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    extern crate std;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        Env,
    };

    #[test]
    fn test_register_and_get() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let hash = Bytes::from_slice(&env, b"QmTestHash");
        let root = Bytes::from_slice(&env, b"merkle_root_bytes");

        let id = client.register_ip(&owner, &hash, &root);
        assert_eq!(id, 1);

        let listing = client.get_listing(&id).expect("listing should exist");
        assert_eq!(listing.owner, owner);
    }

    #[test]
    fn test_register_ip_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let hash = Bytes::from_slice(&env, b"QmTestHash");
        let root = Bytes::from_slice(&env, b"merkle_root_bytes");

        let id = client.register_ip(&owner, &hash, &root);

        let expected = IpRegistered {
            listing_id: id,
            owner: owner.clone(),
            ipfs_hash: hash,
            merkle_root: root,
        };
        assert_eq!(
            env.events().all(),
            std::vec![expected.to_xdr(&env, &contract_id)]
        );
    }

    #[test]
    fn test_owner_index() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let hash = Bytes::from_slice(&env, b"QmHash");
        let root = Bytes::from_slice(&env, b"root");

        let id1 = client.register_ip(&owner_a, &hash, &root);
        let id2 = client.register_ip(&owner_b, &hash, &root);
        let id3 = client.register_ip(&owner_a, &hash, &root);

        let a_ids = client.list_by_owner(&owner_a);
        assert_eq!(a_ids.len(), 2);
        assert_eq!(a_ids.get(0).unwrap(), id1);
        assert_eq!(a_ids.get(1).unwrap(), id3);

        let b_ids = client.list_by_owner(&owner_b);
        assert_eq!(b_ids.len(), 1);
        assert_eq!(b_ids.get(0).unwrap(), id2);

        let empty = client.list_by_owner(&Address::generate(&env));
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_listing_survives_ttl_boundary() {
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

        env.ledger().with_mut(|li| li.sequence_number += 5_000);

        let listing = client.get_listing(&id).expect("listing should exist");
        assert_eq!(listing.owner, owner);
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
            &Bytes::from_slice(&env, b"merkle_root_bytes"),
        );
        assert!(result.is_err());
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
            &Bytes::from_slice(&env, b"QmTestHash"),
            &Bytes::new(&env),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_counter_overflow_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IpRegistry, ());
        let client = IpRegistryClient::new(&env, &contract_id);

        env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .set(&DataKey::Counter, &u64::MAX);
        });

        let owner = Address::generate(&env);
        let result = client.try_register_ip(
            &owner,
            &Bytes::from_slice(&env, b"QmHash"),
            &Bytes::from_slice(&env, b"root"),
        );
        assert!(result.is_err());
    }
}
