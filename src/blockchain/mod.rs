use crate::types::block::Block;
use crate::types::hash::H256;
use crate::types::hash::Hashable;
use crate::types::address::Address;
use std::collections::HashMap;
use hex_literal::hex;

#[derive(Debug)]
pub enum BlockchainError {
    BlockNotInserted,
    InvalidNonce,
    InsufficientBalance,
}

pub struct Blockchain {
    pub blocks: HashMap<H256, Block>,
    chain_lengths: HashMap<H256, usize>, // Track chain length for each block's hash
    tip: H256, // Track the tip of the longest chain
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        // Create a genesis block with fixed values
        let parent = H256::from([0u8; 32]); // Parent is all zeroes
        let nonce = 0;
        /*
        let difficulty = H256::from(hex!(
            "000000000000000000000000000000000000000000000000000000000000000f"
        ));
        */
        
        let difficulty = H256::from(hex!(
            "00007fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        ));
        
        let content = crate::types::block::Content {
            data: vec![], // Empty transactions
        };
        let timestamp = 0;
        let merkle_root = crate::types::block::compute_merkle_root(&content.data);

        let header = crate::types::block::Header {
            parent,
            nonce,
            difficulty,
            timestamp,
            merkle_root,
        };

        let genesis_block = Block {
            header,
            content,
        };

        let genesis_hash = genesis_block.hash();

        let mut blocks = HashMap::new();
        blocks.insert(genesis_hash, genesis_block);

        let mut chain_lengths = HashMap::new();
        chain_lengths.insert(genesis_hash, 0); // Genesis block has height 0

        Self {
            blocks,
            chain_lengths,
            tip: genesis_hash, // The genesis block is the initial tip
        }
    }

    /// Insert a block into the blockchain
    pub fn insert(&mut self, block: &Block) -> Result<(), BlockchainError> {
        let block_hash = block.hash();
        let parent_hash = block.get_parent();

        // Ensure the parent exists in the blockchain
        if let Some(&parent_length) = self.chain_lengths.get(&parent_hash) {

            let new_length = parent_length + 1;

            // Insert the block
            self.blocks.insert(block_hash, block.clone());
            self.chain_lengths.insert(block_hash, new_length);

            // Update the tip if the new block creates a longer chain
            if new_length > self.chain_lengths[&self.tip] {
                self.tip = block_hash;
            }
            Ok(())
        } else {
            Err(BlockchainError::BlockNotInserted)
        }
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.tip
    }

    pub fn get_block(&self, hash: &H256) -> Option<&Block> {
        self.blocks.get(hash)
    }

    /// Get all blocks' hashes of the longest chain, ordered from genesis to the tip
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut chain = Vec::new();
        let mut current_hash = self.tip;

        // Traverse backwards from the tip to the genesis
        while let Some(block) = self.blocks.get(&current_hash) {
            chain.push(current_hash);
            current_hash = block.get_parent(); // Move to the parent block
        }

        chain.reverse(); // Reverse the chain to be from genesis to tip
        chain
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());

    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST