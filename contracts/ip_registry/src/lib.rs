#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, Env, Vec};

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
}

#[contract]
pub struct IpRegistry;

#[contractimpl]
impl IpRegistry {
    /// Register a new IP listing. Returns the listing ID.
    pub fn register_ip(env: Env, owner: Address, ipfs_hash: Bytes, merkle_root: Bytes) -> u64 {
        owner.require_auth();
        let id: u64 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0) + 1;
        env.storage().instance().set(&DataKey::Counter, &id);
        env.storage().instance().set(
            &DataKey::Listing(id),
            &Listing { owner, ipfs_hash, merkle_root },
        );
        id
    }

    pub fn get_listing(env: Env, listing_id: u64) -> Listing {
        env.storage()
            .instance()
            .get(&DataKey::Listing(listing_id))
            .expect("listing not found")
    }

    pub fn list_by_owner(env: Env, owner: Address) -> Vec<u64> {
        let count: u64 = env.storage().instance().get(&DataKey::Counter).unwrap_or(0);
        let mut result = Vec::new(&env);
        for id in 1..=count {
            let listing: Listing = env
                .storage()
                .instance()
                .get(&DataKey::Listing(id))
                .unwrap();
            if listing.owner == owner {
                result.push_back(id);
            }
        }
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

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
}
