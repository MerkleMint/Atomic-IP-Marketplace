#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, BytesN, Bytes, Env, Vec};

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
}

#[contract]
pub struct ZkVerifier;

#[contractimpl]
impl ZkVerifier {
    /// Store the Merkle root for a listing.
    pub fn set_merkle_root(env: Env, listing_id: u64, root: BytesN<32>) {
        env.storage()
            .instance()
            .set(&DataKey::MerkleRoot(listing_id), &root);
    }

    pub fn get_merkle_root(env: Env, listing_id: u64) -> BytesN<32> {
        env.storage()
            .instance()
            .get(&DataKey::MerkleRoot(listing_id))
            .expect("root not found")
    }

    /// Verify a Merkle inclusion proof for a leaf against the stored root.
    pub fn verify_partial_proof(env: Env, listing_id: u64, leaf: Bytes, path: Vec<ProofNode>) -> bool {
        let root: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::MerkleRoot(listing_id))
            .expect("root not found");

        let mut current: BytesN<32> = env.crypto().sha256(&leaf).into();
        for node in path.iter() {
            // Concatenate: if is_left, sibling || current, else current || sibling
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
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{Bytes, Env, Vec};

    #[test]
    fn test_single_leaf_proof() {
        let env = Env::default();
        let contract_id = env.register(ZkVerifier, ());
        let client = ZkVerifierClient::new(&env, &contract_id);

        let leaf = Bytes::from_slice(&env, b"gear_ratio:3:1");
        // For a single-element tree, root = sha256(leaf)
        let root: BytesN<32> = env.crypto().sha256(&leaf).into();

        client.set_merkle_root(&1u64, &root);

        let path: Vec<ProofNode> = Vec::new(&env);
        assert!(client.verify_partial_proof(&1u64, &leaf, &path));
    }
}
