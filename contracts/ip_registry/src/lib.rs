#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, Env, Vec};

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

#[contract]
pub struct IpRegistry;

#[contractimpl]
impl IpRegistry {
    /// Register a new IP listing. Returns the listing ID.
    /// 
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `owner` - The address of the owner registering the IP.
    /// * `ipfs_hash` - The IPFS hash of the off-chain IP data.
    /// * `merkle_root` - The Merkle root of the listing data (used for ZK verifications).
    /// 
    /// # Returns
    /// Returns the unique `u64` identifier for the newly registered listing.
    /// 
    /// # Panics
    /// * Panics if the caller is not the `owner`.
    pub fn register_ip(env: Env, owner: Address, ipfs_hash: Bytes, merkle_root: Bytes) -> u64 {
        owner.require_auth();
        let id: u64 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0) + 1;
        env.storage().instance().set(&DataKey::Counter, &id);

        let key = DataKey::Listing(id);
        env.storage().persistent().set(&key, &Listing { owner: owner.clone(), ipfs_hash, merkle_root });
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        let idx_key = DataKey::OwnerIndex(owner.clone());
        let mut ids: Vec<u64> = env.storage().persistent().get(&idx_key).unwrap_or_else(|| Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(&idx_key, &ids);
        env.storage().persistent().extend_ttl(&idx_key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        env.storage().instance().extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        id
    }

    /// Retrieves a specific IP listing by its ID.
    /// 
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `listing_id` - The ID of the listing to retrieve.
    /// 
    /// # Returns
    /// Returns the `Listing` struct containing owner, IPFS hash, and Merkle root.
    /// 
    /// # Panics
    /// * Panics if the listing is not found in persistent storage.
    pub fn get_listing(env: Env, listing_id: u64) -> Listing {
        env.storage()
            .persistent()
            .get(&DataKey::Listing(listing_id))
            .expect("listing not found")
    }

    /// Retrieves all listing IDs owned by a specific address.
    /// 
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `owner` - The address of the owner.
    /// 
    /// # Returns
    /// Returns a `Vec<u64>` containing all listing IDs associated with the specified owner.
    /// 
    /// # Panics
    /// This view function does not panic under normal conditions, but will panic if internal persistent loading fails for an existing ID.
    pub fn list_by_owner(env: Env, owner: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerIndex(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, Env};

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

        let listing = client.get_listing(&id);
        assert_eq!(listing.owner, owner);
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

        let listing = client.get_listing(&id);
        assert_eq!(listing.owner, owner);
    }
}
