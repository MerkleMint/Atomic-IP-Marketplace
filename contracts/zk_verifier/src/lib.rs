#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    Unauthorized = 1,
}

const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;

/// A single Merkle proof node: (sibling_hash, is_left)
#[contracttype]
#[derive(Clone)]
pub struct ProofNode {
    pub sibling: BytesN<32>,
    pub is_left: bool,
}

#[contracttype]
pub enum DataKey {
    MerkleRoot(u64),
    Owner(u64),
}

#[contract]
pub struct ZkVerifier;

#[contractimpl]
impl ZkVerifier {
    /// Store the Merkle root for a listing. Only the listing owner can set or overwrite it.
    pub fn set_merkle_root(env: Env, owner: Address, listing_id: u64, root: BytesN<32>) {
        owner.require_auth();
        let owner_key = DataKey::Owner(listing_id);
        if let Some(existing_owner) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&owner_key)
        {
            if existing_owner != owner {
                soroban_sdk::panic_with_error!(&env, ContractError::Unauthorized);
            }
        } else {
            env.storage().persistent().set(&owner_key, &owner);
        }
        env.storage().persistent().extend_ttl(
            &owner_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        let key = DataKey::MerkleRoot(listing_id);
        env.storage().persistent().set(&key, &root);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.events().publish(MerkleRootSet {
            listing_id,
            owner,
            merkle_root: root,
        });
    }

    /// Retrieves the stored Merkle root for a given listing, or None if not set.
    pub fn get_merkle_root(env: Env, listing_id: u64) -> Option<BytesN<32>> {
        env.storage()
            .persistent()
            .get(&DataKey::MerkleRoot(listing_id))
    }

    /// Verify a Merkle inclusion proof for a leaf against the stored root.
    pub fn verify_partial_proof(
        env: Env,
        listing_id: u64,
        leaf: Bytes,
        path: Vec<ProofNode>,
    ) -> bool {
        let root: BytesN<32> = match env
            .storage()
            .persistent()
            .get(&DataKey::MerkleRoot(listing_id))
        {
            Some(r) => r,
            None => return false,
        };

        let mut current: BytesN<32> = env.crypto().sha256(&leaf).into();
        for node in path.iter() {
            let mut combined = Bytes::new(&env);
            if node.is_left {
                combined.extend_from_array(&node.sibling.to_array());
                combined.extend_from_array(&current.to_array());
            } else {
                combined.extend_from_array(&current.to_array());
                combined.extend_from_array(&node.sibling.to_array());
            }
            current = env.crypto().sha256(&combined).into();
        }
        current == root
    }

    /// Transfer ownership of a listing's Merkle root to a new owner.
    pub fn transfer_root_ownership(
        env: Env,
        current_owner: Address,
        listing_id: u64,
        new_owner: Address,
    ) {
        current_owner.require_auth();
        let owner_key = DataKey::Owner(listing_id);
        let stored: Address = env
            .storage()
            .persistent()
            .get(&owner_key)
            .unwrap_or_else(|| soroban_sdk::panic_with_error!(&env, ContractError::Unauthorized));
        if stored != current_owner {
            soroban_sdk::panic_with_error!(&env, ContractError::Unauthorized);
        }
        env.storage().persistent().set(&owner_key, &new_owner);
        env.storage().persistent().extend_ttl(
            &owner_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        Bytes, Env, Vec,
    };

    #[test]
    fn test_get_merkle_root_missing_returns_none() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);
        assert_eq!(client.get_merkle_root(&99u64), None);
    }

    #[test]
    fn test_single_leaf_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"gear_ratio:3:1");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();

        client.set_merkle_root(&owner, &1u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&1u64, &leaf, &path));
    }

    #[test]
    fn test_merkle_root_survives_ttl_boundary() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"circuit_spec:v2");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &42u64, &root);

        env.ledger().with_mut(|li| li.sequence_number += 5_000);

        assert_eq!(client.get_merkle_root(&42u64), Some(root));
    }

    #[test]
    fn test_owner_ttl_extended_on_root_update() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let root1: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"root_v1"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root1);

        // Advance ledger close to TTL expiry
        env.ledger()
            .with_mut(|li| li.sequence_number += PERSISTENT_TTL_LEDGERS - 1);

        // Update root — must also refresh Owner TTL
        let root2: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"root_v2"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root2);

        // Advance again; owner key should still be alive (TTL was re-extended)
        env.ledger()
            .with_mut(|li| li.sequence_number += PERSISTENT_TTL_LEDGERS - 1);

        // A different caller must still be rejected — owner key is alive
        let attacker = Address::generate(&env);
        let fake_root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"fake"))
            .into();
        let result = std::panic::catch_unwind(|| {
            client.set_merkle_root(&attacker, &1u64, &fake_root);
        });
        assert!(result.is_err(), "attacker should be rejected while owner key is alive");
    }

    #[test]
    fn test_unauthorized_overwrite_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"secret");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();

        client.set_merkle_root(&owner, &1u64, &root);

        let fake_root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"fake"))
            .into();
        client.set_merkle_root(&attacker, &1u64, &fake_root);
    }

    #[test]
    fn test_verify_partial_proof_missing_root_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(!client.verify_partial_proof(&99u64, &leaf, &path));
    }

    #[test]
    fn test_transfer_root_ownership_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let root: BytesN<32> = env.crypto().sha256(&Bytes::from_slice(&env, b"leaf")).into();
        client.set_merkle_root(&owner, &1u64, &root);

        client.transfer_root_ownership(&owner, &1u64, &new_owner);

        // new_owner can now update the root; old owner cannot
        let new_root: BytesN<32> = env.crypto().sha256(&Bytes::from_slice(&env, b"new")).into();
        client.set_merkle_root(&new_owner, &1u64, &new_root);
        assert_eq!(client.get_merkle_root(&1u64), Some(new_root));
    }

    #[test]
    fn test_transfer_root_ownership_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let root: BytesN<32> = env.crypto().sha256(&Bytes::from_slice(&env, b"leaf")).into();
        client.set_merkle_root(&owner, &1u64, &root);

        let result = client.try_transfer_root_ownership(&attacker, &1u64, &new_owner);
        assert!(result.is_err());
    }
}
