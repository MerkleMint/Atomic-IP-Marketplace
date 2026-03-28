#![no_std]
use soroban_poseidon::poseidon_hash;
use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, crypto::BnScalar, Address, Bytes, BytesN,
    Env, U256, Vec,
};

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

/// Emitted when a Merkle root is stored for a listing.
#[contractevent]
pub struct MerkleRootSet {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
    pub root: BytesN<32>,
}

/// Emitted when a partial proof is verified.
#[contractevent]
pub struct ProofVerified {
    #[topic]
    pub listing_id: u64,
    pub result: bool,
}

#[contract]
pub struct ZkVerifier;

/// Convert a `BytesN<32>` to a `U256` (big-endian).
fn bytesn_to_u256(env: &Env, b: &BytesN<32>) -> U256 {
    U256::from_be_bytes(env, &b.into())
}

/// Convert a `U256` to a `BytesN<32>` (big-endian, zero-padded).
fn u256_to_bytesn(env: &Env, u: &U256) -> BytesN<32> {
    let be: Bytes = u.to_be_bytes();
    let len = be.len();
    if len == 32 {
        be.try_into().unwrap()
    } else {
        let mut padded = Bytes::new(env);
        for _ in 0..(32 - len) {
            padded.push_back(0u8);
        }
        padded.append(&be);
        padded.try_into().unwrap()
    }
}

/// Hash a single field element using Poseidon (t=2, 1 input) over BN254.
fn poseidon1(env: &Env, a: U256) -> U256 {
    let inputs: Vec<U256> = soroban_sdk::vec![env, a];
    poseidon_hash::<2, BnScalar>(env, &inputs)
}

/// Hash two field elements using Poseidon (t=3, 2 inputs) over BN254.
fn poseidon2(env: &Env, a: U256, b: U256) -> U256 {
    let inputs: Vec<U256> = soroban_sdk::vec![env, a, b];
    poseidon_hash::<3, BnScalar>(env, &inputs)
}

/// Interpret raw bytes as a field element by zero-padding to 32 bytes (big-endian U256).
fn bytes_to_field(env: &Env, b: &Bytes) -> U256 {
    let len = b.len();
    if len == 32 {
        U256::from_be_bytes(env, b)
    } else {
        let mut padded = Bytes::new(env);
        for _ in 0..(32 - len) {
            padded.push_back(0u8);
        }
        padded.append(b);
        U256::from_be_bytes(env, &padded)
    }
}

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
            assert!(
                existing_owner == owner,
                "unauthorized: caller is not the listing owner"
            );
        } else {
            env.storage().persistent().set(&owner_key, &owner);
            env.storage().persistent().extend_ttl(
                &owner_key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        let key = DataKey::MerkleRoot(listing_id);
        env.storage().persistent().set(&key, &root);
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_TTL_LEDGERS, PERSISTENT_TTL_LEDGERS);

        MerkleRootSet {
            listing_id,
            owner,
            root,
        }
        .publish(&env);
    }

    /// Returns true if a Merkle root has been set for the given listing, false otherwise.
    ///
    /// # Arguments
    /// * `env` - The contract environment.
    /// * `listing_id` - The ID of the listing.
    ///
    /// # Returns
    /// `true` if a root exists, `false` otherwise. Never panics.
    pub fn proof_exists(env: Env, listing_id: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::MerkleRoot(listing_id))
    }

    /// Retrieves the stored Merkle root for a given listing.
    pub fn get_merkle_root(env: Env, listing_id: u64) -> Option<BytesN<32>> {
        env.storage()
            .persistent()
            .get(&DataKey::MerkleRoot(listing_id))
    }

    /// Verify a Merkle inclusion proof for a leaf against the stored root using Poseidon hashing.
    ///
    /// Compatible with off-chain Poseidon (circom/iden3) proof generators over BN254.
    pub fn verify_partial_proof(
        env: Env,
        listing_id: u64,
        leaf: Bytes,
        path: Vec<ProofNode>,
    ) -> bool {
        let root: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::MerkleRoot(listing_id))
            .expect("root not found");

        let leaf_fe = bytes_to_field(&env, &leaf);
        let mut current: U256 = poseidon1(&env, leaf_fe);

        for node in path.iter() {
            let sibling = bytesn_to_u256(&env, &node.sibling);
            current = if node.is_left {
                poseidon2(&env, sibling, current)
            } else {
                poseidon2(&env, current, sibling)
            };
        }

        let result = u256_to_bytesn(&env, &current) == root;

        ProofVerified { listing_id, result }.publish(&env);

        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    extern crate std;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        Bytes, Env, Vec,
    };

    fn poseidon_leaf(env: &Env, leaf: &Bytes) -> BytesN<32> {
        let fe = bytes_to_field(env, leaf);
        let h = poseidon1(env, fe);
        u256_to_bytesn(env, &h)
    }

    fn poseidon_pair(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
        let l = bytesn_to_u256(env, left);
        let r = bytesn_to_u256(env, right);
        let h = poseidon2(env, l, r);
        u256_to_bytesn(env, &h)
    }

    #[test]
    fn test_proof_exists_returns_false_when_no_root_set() {
        let env = Env::default();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        assert!(!client.proof_exists(&99u64));
    }

    #[test]
    fn test_proof_exists_returns_true_after_set_merkle_root() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"proof_data");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();

        assert!(!client.proof_exists(&1u64));
        client.set_merkle_root(&owner, &1u64, &root);
        assert!(client.proof_exists(&1u64));
    }

    #[test]
    fn test_proof_exists_is_isolated_per_listing() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"data");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();

        client.set_merkle_root(&owner, &1u64, &root);

        assert!(client.proof_exists(&1u64));
        assert!(!client.proof_exists(&2u64));
    }

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
        let root = poseidon_leaf(&env, &leaf);

        client.set_merkle_root(&owner, &1u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&1u64, &leaf, &path));
    }

    #[test]
    fn test_set_merkle_root_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"gear_ratio:3:1");
        let root = poseidon_leaf(&env, &leaf);

        client.set_merkle_root(&owner, &1u64, &root);

        let expected = MerkleRootSet {
            listing_id: 1u64,
            owner: owner.clone(),
            root: root.clone(),
        };
        assert_eq!(
            env.events().all(),
            std::vec![expected.to_xdr(&env, &contract_id)]
        );
    }

    #[test]
    fn test_verify_partial_proof_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"gear_ratio:3:1");
        let root = poseidon_leaf(&env, &leaf);
        client.set_merkle_root(&owner, &1u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        let result = client.verify_partial_proof(&1u64, &leaf, &path);
        assert!(result);

        // The last event should be the ProofVerified event.
        let all = env.events().all().filter_by_contract(&contract_id);
        let last = all.events().last().unwrap().clone();
        let expected = ProofVerified {
            listing_id: 1u64,
            result: true,
        };
        assert_eq!(std::vec![last], std::vec![expected.to_xdr(&env, &contract_id)]);
    }

    #[test]
    fn test_merkle_root_survives_ttl_boundary() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"circuit_spec:v2");
        let root = poseidon_leaf(&env, &leaf);
        client.set_merkle_root(&owner, &42u64, &root);

        env.ledger().with_mut(|li| li.sequence_number += 5_000);

        assert_eq!(client.get_merkle_root(&42u64), Some(root));
    }

    #[test]
    #[should_panic(expected = "unauthorized: caller is not the listing owner")]
    fn test_unauthorized_overwrite_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"secret");
        let root = poseidon_leaf(&env, &leaf);

        client.set_merkle_root(&owner, &1u64, &root);

        let fake_leaf = Bytes::from_slice(&env, b"fake");
        let fake_root = poseidon_leaf(&env, &fake_leaf);
        client.set_merkle_root(&attacker, &1u64, &fake_root);
    }

    #[test]
    fn test_two_leaf_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);

        let leaf0 = Bytes::from_slice(&env, b"leaf_zero");
        let leaf1 = Bytes::from_slice(&env, b"leaf_one");
        let h0 = poseidon_leaf(&env, &leaf0);
        let h1 = poseidon_leaf(&env, &leaf1);
        let root = poseidon_pair(&env, &h0, &h1);

        client.set_merkle_root(&owner, &2u64, &root);

        let path0: Vec<ProofNode> = soroban_sdk::vec![
            &env,
            ProofNode { sibling: h1.clone(), is_left: false }
        ];
        assert!(client.verify_partial_proof(&2u64, &leaf0, &path0));

        let path1: Vec<ProofNode> = soroban_sdk::vec![
            &env,
            ProofNode { sibling: h0.clone(), is_left: true }
        ];
        assert!(client.verify_partial_proof(&2u64, &leaf1, &path1));
    }
}
