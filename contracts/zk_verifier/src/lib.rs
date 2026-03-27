#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, Bytes, BytesN,
    Env, Vec,
};

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

#[contractevent]
pub struct MerkleRootSet {
    #[topic]
    pub listing_id: u64,
    #[topic]
    pub owner: Address,
    pub merkle_root: BytesN<32>,
}

#[contract]
pub struct ZkVerifier;

#[contractimpl]
impl ZkVerifier {
    /// Store the Merkle root for a listing. Only the listing owner can set or overwrite it.
    pub fn set_merkle_root(
        env: Env,
        owner: Address,
        listing_id: u64,
        root: BytesN<32>,
    ) -> Result<(), ContractError> {
        owner.require_auth();
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
        env.storage()
            .persistent()
            .get(&DataKey::MerkleRoot(listing_id))
    }

    /// Verify a Merkle inclusion proof for a leaf against the stored root.
    ///
    /// # Proof format
    ///
    /// Each `ProofNode` in `path` contains:
    ///   - `sibling: BytesN<32>` — the SHA-256 hash of the sibling node at this level.
    ///   - `is_left: bool`       — true if the sibling is the LEFT child (current node is right).
    ///
    /// The leaf is hashed with SHA-256 to produce the starting node hash.
    /// At each level the current hash and sibling are concatenated (sibling first if
    /// `is_left == true`, current first otherwise) and SHA-256'd to produce the
    /// parent hash. The final hash must equal the stored Merkle root.
    ///
    /// Single-leaf trees have an empty path; the root equals `sha256(leaf)`.
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
        let result = current == root;
        ProofVerified { listing_id, result }.publish(&env);
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
        testutils::{Address as _, Ledger as _},
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
        assert_eq!(
            result,
            Err(Ok(ContractError::Unauthorized)),
            "attacker should not be able to overwrite owner's root"
        );
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
        let root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"leaf"))
            .into();
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
        let root: BytesN<32> = env
            .crypto()
            .sha256(&Bytes::from_slice(&env, b"leaf"))
            .into();
        client.set_merkle_root(&owner, &1u64, &root);

        let result = client.try_transfer_root_ownership(&attacker, &1u64, &new_owner);
        assert!(result.is_err());
    }

    // ── SHA-256 proof tests ───────────────────────────────────────────────────

    #[test]
    fn test_two_leaf_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        // Two-leaf tree: leaf_a = b"leaf_a", leaf_b = b"leaf_b"
        // root = sha256(sha256(leaf_a) || sha256(leaf_b))
        let leaf_a = Bytes::from_slice(&env, b"leaf_a");
        let leaf_b = Bytes::from_slice(&env, b"leaf_b");
        let hash_a: BytesN<32> = env.crypto().sha256(&leaf_a).into();
        let hash_b: BytesN<32> = env.crypto().sha256(&leaf_b).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_a.to_array());
        combined.extend_from_array(&hash_b.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();

        client.set_merkle_root(&owner, &2u64, &root);

        // Prove leaf_a: sibling is hash_b, is_left = false (sibling is right)
        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode { sibling: hash_b, is_left: false });
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
        path.push_back(ProofNode { sibling: hash_sib, is_left: false });
        assert!(!client.verify_partial_proof(&4u64, &tampered, &path));
    }

    #[test]
    fn test_is_left_ordering_correctness() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        // leaf is the RIGHT child; sibling is LEFT
        // root = sha256(hash_sib || hash_leaf)
        let leaf = Bytes::from_slice(&env, b"right_leaf");
        let sibling_bytes = Bytes::from_slice(&env, b"left_sibling");
        let hash_leaf: BytesN<32> = env.crypto().sha256(&leaf).into();
        let hash_sib: BytesN<32> = env.crypto().sha256(&sibling_bytes).into();
        let mut combined = Bytes::new(&env);
        combined.extend_from_array(&hash_sib.to_array());
        combined.extend_from_array(&hash_leaf.to_array());
        let root: BytesN<32> = env.crypto().sha256(&combined).into();

        client.set_merkle_root(&owner, &5u64, &root);

        // is_left = true means sibling is on the left
        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode { sibling: hash_sib.clone(), is_left: true });
        assert!(client.verify_partial_proof(&5u64, &leaf, &path));

        // Wrong ordering should fail
        let mut wrong_path: Vec<ProofNode> = Vec::new(&env);
        wrong_path.push_back(ProofNode { sibling: hash_sib, is_left: false });
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

        // Submit a wrong sibling
        let wrong_sibling = Bytes::from_slice(&env, b"wrong_sibling");
        let hash_wrong_sib: BytesN<32> = env.crypto().sha256(&wrong_sibling).into();
        let mut path: Vec<ProofNode> = Vec::new(&env);
        path.push_back(ProofNode { sibling: hash_wrong_sib, is_left: false });
        assert!(!client.verify_partial_proof(&3u64, &leaf, &path));
    }

    #[test]
    fn test_two_level_merkle_proof() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        // 4-leaf tree: [a, b, c, d]
        // level1: ab = sha256(h_a||h_b), cd = sha256(h_c||h_d)
        // root:   sha256(ab||cd)
        // Prove leaf_a with path: [sibling=h_b (right), sibling=cd (right)]
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
        path.push_back(ProofNode { sibling: h_b, is_left: false });
        path.push_back(ProofNode { sibling: cd, is_left: false });
        assert!(client.verify_partial_proof(&10u64, &leaf_a, &path));
    }

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
        let result = client.verify_partial_proof(&1u64, &leaf, &path);
        assert!(result);

        // At least one event should have been emitted (proof_verified)
        assert!(!env.events().all().events().is_empty(), "proof_verified event not emitted");
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

        // At least one event should have been emitted (merkle_root_set)
        assert!(!env.events().all().events().is_empty(), "merkle_root_set event not emitted");
    }

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

        // listing 2 has no root — proof should return false
        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(!client.verify_partial_proof(&2u64, &leaf, &path));
    }
}
