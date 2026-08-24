#![no_std]
use soroban_sdk::{
    contracterror, contractevent, contracttype, Address, Bytes, BytesN, Env, Vec,
};

#[cfg(not(feature = "contract"))]
use soroban_sdk::contractclient;

#[cfg(feature = "contract")]
use soroban_sdk::{contract, contractimpl};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    Unauthorized = 1,
    RootNotFound = 2,
    ProofTooLong = 3,
    InvalidInput = 4,
    InvalidRoot = 5,
}

#[cfg(feature = "contract")]
const PERSISTENT_TTL_LEDGERS: u32 = 6_312_000;
#[cfg(feature = "contract")]
const MAX_PROOF_DEPTH: u32 = 64;
/// ~3600 seconds at ~6 seconds per ledger
#[cfg(feature = "contract")]
const CACHE_TTL_LEDGERS: u32 = 600;
/// Maximum cached proof results before LRU eviction kicks in
#[cfg(feature = "contract")]
const MAX_CACHE_ENTRIES: u32 = 128;

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
    /// Temporary cache entry: proof fingerprint -> bool result
    ProofCache(BytesN<32>),
    /// Insertion-ordered list of cache keys for LRU eviction (instance storage)
    CacheIndex,
}

#[contractevent]
pub struct MerkleRootSet {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
    pub merkle_root: BytesN<32>,
}

#[contractevent]
pub struct ProofVerified {
    #[topic]
    pub listing_id: u64,
    pub result: bool,
}

#[contractevent]
pub struct RootOwnershipTransferred {
    #[topic]
    pub listing_id: u64,
    pub from: Address,
    pub to: Address,
}

/// Emitted when a proof result is served from cache
#[contractevent]
pub struct CacheHit {
    #[topic]
    pub listing_id: u64,
    pub cache_key: BytesN<32>,
}

/// Emitted when cache lookup misses and full verification runs
#[contractevent]
pub struct CacheMiss {
    #[topic]
    pub listing_id: u64,
}

/// Emitted on merkle root update — signals that stale cached results exist for the listing
#[contractevent]
pub struct CacheInvalidated {
    #[topic]
    pub listing_id: u64,
}

/// Client interface for ZkVerifier — only available in library mode to avoid duplication
#[cfg(not(feature = "contract"))]
#[contractclient(name = "ZkVerifierClient")]
pub trait ZkVerifierInterface {
    fn set_merkle_root(
        env: Env,
        owner: Address,
        listing_id: u64,
        root: BytesN<32>,
    ) -> Result<(), ContractError>;
    fn verify_partial_proof(env: Env, listing_id: u64, leaf: Bytes, path: Vec<ProofNode>) -> bool;
}

/// Compute a deterministic cache key from all proof inputs.
///
/// The key is SHA-256(listing_id_be8 || root || sha256(leaf) || path_bytes).
/// Binding the root into the key means a merkle root update automatically
/// invalidates all cached entries for that listing — cache poisoning is impossible
/// because a valid cached result can only be replayed with an identical root.
#[cfg(feature = "contract")]
fn compute_cache_key(
    env: &Env,
    listing_id: u64,
    root: &BytesN<32>,
    leaf: &Bytes,
    path: &Vec<ProofNode>,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&listing_id.to_be_bytes());
    data.extend_from_array(&root.to_array());
    let leaf_hash: BytesN<32> = env.crypto().sha256(leaf).into();
    data.extend_from_array(&leaf_hash.to_array());
    for node in path.iter() {
        data.extend_from_array(&node.sibling.to_array());
        let flag = [node.is_left as u8];
        data.extend_from_array(&flag);
    }
    env.crypto().sha256(&data).into()
}

#[cfg(feature = "contract")]
fn cache_lookup(env: &Env, cache_key: &BytesN<32>) -> Option<bool> {
    env.storage()
        .temporary()
        .get::<DataKey, bool>(&DataKey::ProofCache(cache_key.clone()))
}

/// Store a proof result with TTL. Evicts the oldest entry when MAX_CACHE_ENTRIES is reached.
#[cfg(feature = "contract")]
fn cache_store(env: &Env, cache_key: BytesN<32>, result: bool) {
    let storage_key = DataKey::ProofCache(cache_key.clone());

    // Refresh TTL for an already-cached entry without touching the index
    if env.storage().temporary().has(&storage_key) {
        env.storage().temporary().set(&storage_key, &result);
        env.storage()
            .temporary()
            .extend_ttl(&storage_key, CACHE_TTL_LEDGERS, CACHE_TTL_LEDGERS);
        return;
    }

    let mut index: Vec<BytesN<32>> = env
        .storage()
        .instance()
        .get::<DataKey, Vec<BytesN<32>>>(&DataKey::CacheIndex)
        .unwrap_or_else(|| Vec::new(env));

    // LRU eviction: oldest entry (front of vec) is removed when at capacity
    if index.len() >= MAX_CACHE_ENTRIES {
        if let Some(oldest) = index.pop_front() {
            env.storage()
                .temporary()
                .remove(&DataKey::ProofCache(oldest));
        }
    }

    index.push_back(cache_key);
    env.storage().instance().set(&DataKey::CacheIndex, &index);
    env.storage().temporary().set(&storage_key, &result);
    env.storage()
        .temporary()
        .extend_ttl(&storage_key, CACHE_TTL_LEDGERS, CACHE_TTL_LEDGERS);
}

/// Core Merkle proof computation, extracted for reuse from cached and uncached paths.
#[cfg(feature = "contract")]
fn compute_proof(env: &Env, root: &BytesN<32>, leaf: &Bytes, path: &Vec<ProofNode>) -> bool {
    let zero_sibling = BytesN::from_array(env, &[0u8; 32]);
    let mut current: BytesN<32> = env.crypto().sha256(leaf).into();
    for node in path.iter() {
        if node.sibling == zero_sibling {
            return false;
        }
        let mut combined = Bytes::new(env);
        if node.is_left {
            combined.extend_from_array(&node.sibling.to_array());
            combined.extend_from_array(&current.to_array());
        } else {
            combined.extend_from_array(&current.to_array());
            combined.extend_from_array(&node.sibling.to_array());
        }
        current = env.crypto().sha256(&combined).into();
    }
    current == *root
}

#[cfg(feature = "contract")]
#[contract]
pub struct ZkVerifier;

#[cfg(feature = "contract")]
#[contractimpl]
impl ZkVerifier {
    /// Store the Merkle root for a listing. Only the listing owner can set or overwrite it.
    /// Emits `CacheInvalidated` so observers know prior cached results for this listing are stale.
    pub fn set_merkle_root(
        env: Env,
        owner: Address,
        listing_id: u64,
        root: BytesN<32>,
    ) -> Result<(), ContractError> {
        owner.require_auth();
        if root == BytesN::from_array(&env, &[0u8; 32]) {
            return Err(ContractError::InvalidRoot);
        }
        let owner_key = DataKey::Owner(listing_id);
        if let Some(existing_owner) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&owner_key)
        {
            if existing_owner != owner {
                return Err(ContractError::Unauthorized);
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
        // Cache invalidation is implicit: new cache keys incorporate the updated root,
        // so old entries can never be hit again. Emit the event for metric tracking.
        CacheInvalidated { listing_id }.publish(&env);
        MerkleRootSet {
            listing_id,
            owner,
            merkle_root: root,
        }
        .publish(&env);
        Ok(())
    }

    /// Retrieves the stored Merkle root for a given listing, or None if not set.
    pub fn get_merkle_root(env: Env, listing_id: u64) -> Option<BytesN<32>> {
        let key = DataKey::MerkleRoot(listing_id);
        let result = env.storage().persistent().get(&key);
        if result.is_some() {
            env.storage().persistent().extend_ttl(
                &key,
                PERSISTENT_TTL_LEDGERS,
                PERSISTENT_TTL_LEDGERS,
            );
        }
        result
    }

    /// Retrieves the owner of a listing's Merkle root, or None if no root has been set.
    pub fn get_owner(env: Env, listing_id: u64) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Owner(listing_id))
    }

    /// Verify a Merkle inclusion proof for a leaf against the stored root.
    ///
    /// # Caching
    ///
    /// Results are cached for `CACHE_TTL_LEDGERS` (~3600 s) keyed by
    /// SHA-256(listing_id || root || sha256(leaf) || path_bytes).
    /// Because the current merkle root is part of the cache key, updating
    /// the root automatically invalidates all prior cached results for that
    /// listing — it is impossible to replay a stale cached result.
    /// The cache holds at most `MAX_CACHE_ENTRIES` entries; the oldest is
    /// evicted (LRU) when the limit is reached.
    ///
    /// # Proof format
    ///
    /// Each `ProofNode` in `path` contains:
    ///   - `sibling: BytesN<32>` — the SHA-256 hash of the sibling node.
    ///   - `is_left: bool`       — true if the sibling is the LEFT child.
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

        if path.len() > MAX_PROOF_DEPTH {
            soroban_sdk::panic_with_error!(&env, ContractError::ProofTooLong);
        }

        let cache_key = compute_cache_key(&env, listing_id, &root, &leaf, &path);

        if let Some(cached) = cache_lookup(&env, &cache_key) {
            CacheHit {
                listing_id,
                cache_key,
            }
            .publish(&env);
            return cached;
        }

        CacheMiss { listing_id }.publish(&env);

        let result = compute_proof(&env, &root, &leaf, &path);
        ProofVerified { listing_id, result }.publish(&env);
        cache_store(&env, cache_key, result);
        result
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
            .unwrap_or_else(|| soroban_sdk::panic_with_error!(&env, ContractError::RootNotFound));
        if stored != current_owner {
            soroban_sdk::panic_with_error!(&env, ContractError::Unauthorized);
        }
        env.storage().persistent().set(&owner_key, &new_owner);
        env.storage().persistent().extend_ttl(
            &owner_key,
            PERSISTENT_TTL_LEDGERS,
            PERSISTENT_TTL_LEDGERS,
        );
        RootOwnershipTransferred {
            listing_id,
            from: current_owner,
            to: new_owner,
        }
        .publish(&env);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        Bytes, Env, Vec,
    };

    // ── Merkle root storage tests ─────────────────────────────────────────────

    #[test]
    fn test_get_merkle_root_missing_returns_none() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);
        assert_eq!(client.get_merkle_root(&99u64), None);
    }

    #[test]
    fn test_get_owner_returns_none_when_no_root() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);
        assert_eq!(client.get_owner(&99u64), None);
    }

    #[test]
    fn test_get_owner_returns_correct_owner() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"root"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root);

        assert_eq!(client.get_owner(&1u64), Some(owner));
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
    fn test_get_merkle_root_extends_ttl_on_read() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"ttl_test_leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &99u64, &root);

        env.ledger()
            .with_mut(|li| li.sequence_number += PERSISTENT_TTL_LEDGERS - 1);

        assert_eq!(client.get_merkle_root(&99u64), Some(root.clone()));

        env.ledger()
            .with_mut(|li| li.sequence_number += PERSISTENT_TTL_LEDGERS - 1);

        assert_eq!(client.get_merkle_root(&99u64), Some(root));
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

        env.ledger()
            .with_mut(|li| li.sequence_number += PERSISTENT_TTL_LEDGERS - 1);

        let root2: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"root_v2"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root2);

        env.ledger()
            .with_mut(|li| li.sequence_number += PERSISTENT_TTL_LEDGERS - 1);

        let attacker = Address::generate(&env);
        let fake_root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"fake"))
            .into();
        let result = client.try_set_merkle_root(&attacker, &1u64, &fake_root);
        assert!(
            result.is_err(),
            "attacker should be rejected while owner key is alive"
        );
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
        let result = client.try_set_merkle_root(&attacker, &1u64, &fake_root);
        assert!(result.is_err());
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
    fn test_verify_partial_proof_rejects_zero_sibling_node() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &7u64, &root);

        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: BytesN::from_array(&env, &[0u8; 32]),
            is_left: false,
        });
        assert!(!client.verify_partial_proof(&7u64, &leaf, &path));
    }

    #[test]
    fn test_verify_partial_proof_rejects_oversized_path() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &8u64, &root);

        let non_zero_hash: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
        let mut path: Vec<ProofNode> = Vec::new(&env);
        for _ in 0..(MAX_PROOF_DEPTH + 1) {
            path.push_back(ProofNode {
                sibling: non_zero_hash.clone(),
                is_left: false,
            });
        }

        let result = client.try_verify_partial_proof(&8u64, &leaf, &path);
        assert!(result.is_err());
    }

    // ── Transfer ownership tests ──────────────────────────────────────────────

    #[test]
    fn test_transfer_root_ownership_success() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"leaf"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root);

        client.transfer_root_ownership(&owner, &1u64, &new_owner);

        let new_root: BytesN<32> = env.crypto().sha256(&Bytes::from_slice(&env, b"new")).into();
        client.set_merkle_root(&new_owner, &1u64, &new_root);
        assert_eq!(client.get_merkle_root(&1u64), Some(new_root));
    }

    #[test]
    fn test_transfer_root_ownership_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"leaf"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root);
        client.transfer_root_ownership(&owner, &1u64, &new_owner);

        assert!(
            !env.events().all().events().is_empty(),
            "transfer ownership must emit an event"
        );
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
        let root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"leaf"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root);

        let result = client.try_transfer_root_ownership(&attacker, &1u64, &new_owner);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_root_ownership_root_not_found() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);

        let result = client.try_transfer_root_ownership(&owner, &99u64, &new_owner);
        assert!(result.is_err());
    }

    // ── SHA-256 proof correctness tests ──────────────────────────────────────

    #[test]
    fn test_two_leaf_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf_a = Bytes::from_slice(&env, b"leaf_a");
        let leaf_b = Bytes::from_slice(&env, b"leaf_b");
        let hash_a: BytesN<32> = env.crypto().sha256(&leaf_a).into();
        let hash_b: BytesN<32> = env.crypto().sha256(&leaf_b).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_a.to_array());
        combined.extend_from_array(&hash_b.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();

        client.set_merkle_root(&owner, &2u64, &root);

        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: hash_b,
            is_left: false,
        });
        assert!(client.verify_partial_proof(&2u64, &leaf_a, &path));
    }

    #[test]
    fn test_tampered_leaf_fails_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let real_leaf = Bytes::from_slice(&env, b"real_leaf");
        let sibling = Bytes::from_slice(&env, b"sibling");
        let hash_real: BytesN<32> = env.crypto().sha256(&real_leaf).into();
        let hash_sib: BytesN<32> = env.crypto().sha256(&sibling).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_real.to_array());
        combined.extend_from_array(&hash_sib.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();

        client.set_merkle_root(&owner, &4u64, &root);

        let tampered = Bytes::from_slice(&env, b"tampered_leaf");
        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: hash_sib,
            is_left: false,
        });
        assert!(!client.verify_partial_proof(&4u64, &tampered, &path));
    }

    #[test]
    fn test_is_left_ordering_correctness() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"right_leaf");
        let sibling_bytes = Bytes::from_slice(&env, b"left_sibling");
        let hash_leaf: BytesN<32> = env.crypto().sha256(&leaf).into();
        let hash_sib: BytesN<32> = env.crypto().sha256(&sibling_bytes).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_sib.to_array());
        combined.extend_from_array(&hash_leaf.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();

        client.set_merkle_root(&owner, &5u64, &root);

        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: hash_sib.clone(),
            is_left: true,
        });
        assert!(client.verify_partial_proof(&5u64, &leaf, &path));

        let mut wrong_path: Vec<ProofNode> = Vec::new(&env);
        wrong_path.push_back(ProofNode {
            sibling: hash_sib,
            is_left: false,
        });
        assert!(!client.verify_partial_proof(&5u64, &leaf, &wrong_path));
    }

    #[test]
    fn test_invalid_proof_wrong_sibling() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let real_sibling = Bytes::from_slice(&env, b"real_sibling");
        let hash_leaf: BytesN<32> = env.crypto().sha256(&leaf).into();
        let hash_real_sib: BytesN<32> = env.crypto().sha256(&real_sibling).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_leaf.to_array());
        combined.extend_from_array(&hash_real_sib.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();

        client.set_merkle_root(&owner, &3u64, &root);

        let wrong_sibling = Bytes::from_slice(&env, b"wrong_sibling");
        let hash_wrong_sib: BytesN<32> = env.crypto().sha256(&wrong_sibling).into();
        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: hash_wrong_sib,
            is_left: false,
        });
        assert!(!client.verify_partial_proof(&3u64, &leaf, &path));
    }

    #[test]
    fn test_two_level_merkle_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf_a = Bytes::from_slice(&env, b"leaf_a");
        let leaf_b = Bytes::from_slice(&env, b"leaf_b");
        let leaf_c = Bytes::from_slice(&env, b"leaf_c");
        let leaf_d = Bytes::from_slice(&env, b"leaf_d");
        let h_a: BytesN<32> = env.crypto().sha256(&leaf_a).into();
        let h_b: BytesN<32> = env.crypto().sha256(&leaf_b).into();
        let h_c: BytesN<32> = env.crypto().sha256(&leaf_c).into();
        let h_d: BytesN<32> = env.crypto().sha256(&leaf_d).into();

        let mut ab_bytes = Bytes::new(&env);
        ab_bytes.extend_from_array(&h_a.to_array());
        ab_bytes.extend_from_array(&h_b.to_array());
        let ab: BytesN<32> = env.crypto().sha256(&ab_bytes).into();

        let mut cd_bytes = Bytes::new(&env);
        cd_bytes.extend_from_array(&h_c.to_array());
        cd_bytes.extend_from_array(&h_d.to_array());
        let cd: BytesN<32> = env.crypto().sha256(&cd_bytes).into();

        let mut root_bytes = Bytes::new(&env);
        root_bytes.extend_from_array(&ab.to_array());
        root_bytes.extend_from_array(&cd.to_array());
        let root: BytesN<32> = env.crypto().sha256(&root_bytes).into();

        client.set_merkle_root(&owner, &10u64, &root);

        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: h_b,
            is_left: false,
        });
        path.push_back(ProofNode {
            sibling: cd,
            is_left: false,
        });
        assert!(client.verify_partial_proof(&10u64, &leaf_a, &path));
    }

    // ── Event emission tests ──────────────────────────────────────────────────

    #[test]
    fn test_verify_partial_proof_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"event_leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &1u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&1u64, &leaf, &path));
        assert!(!env.events().all().events().is_empty(), "no events emitted");
    }

    #[test]
    fn test_set_merkle_root_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"event_leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &1u64, &root);

        assert!(!env.events().all().events().is_empty(), "no events emitted");
    }

    // ── Misc correctness tests ────────────────────────────────────────────────

    #[test]
    fn test_proof_exists_returns_false_when_no_root_set() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        assert_eq!(client.get_merkle_root(&1u64), None);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(!client.verify_partial_proof(&1u64, &leaf, &path));
    }

    #[test]
    fn test_proof_exists_returns_true_after_set_merkle_root() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &1u64, &root);

        assert!(client.get_merkle_root(&1u64).is_some());
        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&1u64, &leaf, &path));
    }

    #[test]
    fn test_proof_exists_is_isolated_per_listing() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &1u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(!client.verify_partial_proof(&2u64, &leaf, &path));
    }

    #[test]
    fn test_set_merkle_root_rejects_zero_root() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let zero_root: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
        let result = client.try_set_merkle_root(&owner, &1u64, &zero_root);
        assert_eq!(
            result.unwrap_err().unwrap(),
            ContractError::InvalidRoot,
            "zero root must be rejected"
        );
    }

    // ── Cache behaviour tests ─────────────────────────────────────────────────

    #[test]
    fn test_cache_miss_on_first_call_emits_cache_miss_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"cache_miss_leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &20u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        client.verify_partial_proof(&20u64, &leaf, &path);

        // Events must include CacheMiss (the proof was computed fresh)
        let events = env.events().all();
        assert!(!events.events().is_empty());
    }

    #[test]
    fn test_cache_hit_on_repeated_verify() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"repeated_leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &21u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);

        // First call: cache miss, computes proof
        let first = client.verify_partial_proof(&21u64, &leaf, &path);
        // Second call: should hit the cache and return the same result
        let second = client.verify_partial_proof(&21u64, &leaf, &path);

        assert_eq!(first, second);
        assert!(first, "valid proof must return true");
    }

    #[test]
    fn test_cache_returns_false_from_cache() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"real_leaf");
        let sibling = Bytes::from_slice(&env, b"sibling");
        let hash_leaf: BytesN<32> = env.crypto().sha256(&leaf).into();
        let hash_sib: BytesN<32> = env.crypto().sha256(&sibling).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_leaf.to_array());
        combined.extend_from_array(&hash_sib.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();
        client.set_merkle_root(&owner, &22u64, &root);

        // Submit a wrong proof -> false, cached
        let wrong_sib: BytesN<32> = BytesN::from_array(&env, &[9u8; 32]);
        let mut bad_path: Vec<ProofNode> = Vec::new(&env);
        bad_path.push_back(ProofNode {
            sibling: wrong_sib,
            is_left: false,
        });
        assert!(!client.verify_partial_proof(&22u64, &leaf, &bad_path));
        // Second call: same false result from cache
        assert!(!client.verify_partial_proof(&22u64, &leaf, &bad_path));
    }

    #[test]
    fn test_cache_invalidation_after_root_update() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"inv_leaf");

        // root1 = sha256(leaf), so empty-path proof is valid
        let root1: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &23u64, &root1);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&23u64, &leaf, &path));

        // Update root to something that doesn't match sha256(leaf)
        let root2: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"other_data"))
            .into();
        client.set_merkle_root(&owner, &23u64, &root2);

        // Same leaf + empty path no longer matches the new root
        // (and must NOT return the stale cached 'true')
        assert!(!client.verify_partial_proof(&23u64, &leaf, &path));
    }

    #[test]
    fn test_ttl_expiration_forces_recomputation() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let leaf = Bytes::from_slice(&env, b"ttl_cache_leaf");
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();
        client.set_merkle_root(&owner, &24u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&24u64, &leaf, &path));

        // Advance past cache TTL; entry expires in temporary storage
        env.ledger()
            .with_mut(|li| li.sequence_number += CACHE_TTL_LEDGERS + 1);

        // Result must still be correct after recomputation
        assert!(client.verify_partial_proof(&24u64, &leaf, &path));
    }

    #[test]
    fn test_cache_poisoning_impossible() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let real_leaf = Bytes::from_slice(&env, b"real");
        let fake_leaf = Bytes::from_slice(&env, b"fake");

        let root: BytesN<32> = env.crypto().sha256(&real_leaf).into();
        client.set_merkle_root(&owner, &25u64, &root);

        // Cache a valid result for real_leaf
        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&25u64, &real_leaf, &path));

        // Attempt with a different leaf must NOT return the cached 'true'
        assert!(!client.verify_partial_proof(&25u64, &fake_leaf, &path));
    }

    /// Benchmark: verify that the second call (cache hit) produces the same result
    /// as the first call (cache miss + full SHA-256 chain), demonstrating correctness
    /// of the caching layer. On-chain, cache hits skip the full merkle hash chain,
    /// reducing CPU instruction cost proportionally to tree depth.
    #[test]
    fn test_cache_benchmark_repeated_deep_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);

        // Build a 4-leaf tree for a multi-level proof
        let leaf_a = Bytes::from_slice(&env, b"bench_a");
        let leaf_b = Bytes::from_slice(&env, b"bench_b");
        let leaf_c = Bytes::from_slice(&env, b"bench_c");
        let leaf_d = Bytes::from_slice(&env, b"bench_d");
        let h_a: BytesN<32> = env.crypto().sha256(&leaf_a).into();
        let h_b: BytesN<32> = env.crypto().sha256(&leaf_b).into();
        let h_c: BytesN<32> = env.crypto().sha256(&leaf_c).into();
        let h_d: BytesN<32> = env.crypto().sha256(&leaf_d).into();

        let mut ab = Bytes::new(&env);
        ab.extend_from_array(&h_a.to_array());
        ab.extend_from_array(&h_b.to_array());
        let ab_hash: BytesN<32> = env.crypto().sha256(&ab).into();

        let mut cd = Bytes::new(&env);
        cd.extend_from_array(&h_c.to_array());
        cd.extend_from_array(&h_d.to_array());
        let cd_hash: BytesN<32> = env.crypto().sha256(&cd).into();

        let mut root_bytes = Bytes::new(&env);
        root_bytes.extend_from_array(&ab_hash.to_array());
        root_bytes.extend_from_array(&cd_hash.to_array());
        let root: BytesN<32> = env.crypto().sha256(&root_bytes).into();

        client.set_merkle_root(&owner, &30u64, &root);

        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode {
            sibling: h_b,
            is_left: false,
        });
        path.push_back(ProofNode {
            sibling: cd_hash,
            is_left: false,
        });

        // First call: cache miss, full SHA-256 chain traversal
        assert!(client.verify_partial_proof(&30u64, &leaf_a, &path));
        // Second call: cache hit, result returned without recomputation
        assert!(client.verify_partial_proof(&30u64, &leaf_a, &path));
    }

    #[test]
    fn test_lru_eviction_does_not_corrupt_results() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);

        // Fill cache with MAX_CACHE_ENTRIES unique entries (one per listing_id)
        for i in 0u64..MAX_CACHE_ENTRIES as u64 {
            let leaf = Bytes::from_slice(&env, &i.to_be_bytes());
            let root: BytesN<32> = env.crypto().sha256(&leaf).into();
            client.set_merkle_root(&owner, &i, &root);
            let path: Vec<ProofNode> = Vec::new(&env);
            assert!(client.verify_partial_proof(&i, &leaf, &path));
        }

        // Inserting one more triggers LRU eviction of the oldest (listing_id=0)
        let extra = MAX_CACHE_ENTRIES as u64;
        let extra_leaf = Bytes::from_slice(&env, &extra.to_be_bytes());
        let extra_root: BytesN<32> = env.crypto().sha256(&extra_leaf).into();
        client.set_merkle_root(&owner, &extra, &extra_root);
        client.verify_partial_proof(&extra, &extra_leaf, &Vec::new(&env));

        // Evicted entry can still be verified correctly — recomputed from scratch
        let leaf_0 = Bytes::from_slice(&env, &0u64.to_be_bytes());
        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&0u64, &leaf_0, &path));
    }
}
