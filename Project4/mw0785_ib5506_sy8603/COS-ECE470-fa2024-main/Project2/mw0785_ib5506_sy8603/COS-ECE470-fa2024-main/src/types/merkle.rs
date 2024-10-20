use super::hash::{Hashable, H256};
use ring::digest;

/// A Merkle tree.
#[derive(Debug, Default)]
pub struct MerkleTree {
    levels: Vec<Vec<H256>>, // Each inner vector represents a level
}

impl MerkleTree {
    pub fn new<T>(data: &[T]) -> Self where T: Hashable, {
        let mut levels = Vec::new();

        // Step 1: Hash the data to create leaves
        let mut current_level: Vec<H256> = data.iter().map(|item| item.hash()).collect();

        // Handle empty data
        if current_level.is_empty() {
            current_level.push(H256::default());
        }

        // Add leaves to levels
        levels.push(current_level.clone());

        // Step 2: Build the tree
        while current_level.len() > 1 {
            if current_level.len() % 2 != 0 {
                // Duplicate last node if necessary
                current_level.push(*current_level.last().unwrap());
            }

            let mut next_level = Vec::new();
            for i in (0..current_level.len()).step_by(2) {
                let left = current_level[i];
                let right = current_level[i + 1];

                let combined = [left.as_ref(), right.as_ref()].concat();
                let parent_hash = digest::digest(&digest::SHA256, &combined).into();

                next_level.push(parent_hash);
            }

            levels.push(next_level.clone());
            current_level = next_level;
        }

        MerkleTree { levels }
    }

    pub fn root(&self) -> H256 {
        // If the tree is empty, return default hash
        self.levels.last().unwrap()[0]
    }

    /// Returns the Merkle Proof of data at index i
    pub fn proof(&self, index: usize) -> Vec<H256> {
        let mut proof = Vec::new();
        let mut idx = index;

        if self.levels.is_empty() || index >= self.levels[0].len() {
            // Invalid index, return empty proof
            return proof;
        }

        for level in &self.levels[..self.levels.len() - 1] {
            if idx % 2 == 0 {
                // Sibling is on the right
                if idx + 1 < level.len() {
                    proof.push(level[idx + 1]);
                } else {
                    // Edge case where sibling is duplicated
                    proof.push(level[idx]);
                }
            } else {
                // Sibling is on the left
                proof.push(level[idx - 1]);
            }

            idx /= 2;
        }

        proof
    }
}

// Move the `verify` function outside the `impl MerkleTree` block
/// Verify that the datum hash with a vector of proofs will produce the Merkle root.
/// Also need the index of datum and `leaf_size`, the total number of leaves.
pub fn verify(root: &H256, datum: &H256, proof: &[H256], index: usize, leaf_size: usize) -> bool {
    if index >= leaf_size {
        return false;
    }

    let mut computed_hash = *datum;
    let mut idx = index;

    for sibling_hash in proof {
        let (left, right) = if idx % 2 == 0 {
            (computed_hash.as_ref(), sibling_hash.as_ref())
        } else {
            (sibling_hash.as_ref(), computed_hash.as_ref())
        };

        let combined = [left, right].concat();
        computed_hash = digest::digest(&digest::SHA256, &combined).into();

        idx /= 2;
    }

    computed_hash == *root
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST


#[cfg(test)]
mod tests {
    use crate::types::hash::H256;
    use super::*;
    macro_rules! gen_merkle_tree_data {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
            ]
        }};
    }
    #[test]
    fn sp2022autograder011() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920")).into()
        );
        // "b69566be6e1720872f73651d1851a0eae0060a132cf0f64a0ffaea248de6cba0" is the hash of
        // "0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d"
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
        // "6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920" is the hash of
        // the concatenation of these two hashes "b69..." and "965..."
        // notice that the order of these two matters
    }
    #[test]
    fn sp2022autograder012() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert_eq!(proof,
                   vec![hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into()]
        );
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
    }
    #[test]
    fn sp2022autograder013() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert!(verify(&merkle_tree.root(), &input_data[0].hash(), &proof, 0, input_data.len()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST