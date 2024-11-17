use crate::types::block::Block;
use crate::types::hash::H256;
use crate::types::hash::Hashable;
use crate::types::address::Address;
use crate::types::state::State;  // Add this import
use std::collections::HashMap;
use hex_literal::hex;
use ring::signature::Ed25519KeyPair;
use ring::rand::SystemRandom;
use ring::signature::KeyPair;

#[derive(Debug)]
pub enum BlockchainError {
    BlockNotInserted,
    InvalidNonce,
    InsufficientBalance,
    InvalidTransaction,  
    StateError,
}
#[derive(Debug)]
pub struct Blockchain {
    pub blocks: HashMap<H256, Block>,
    pub states: HashMap<H256, State>,  // Maps block hash to state after that block
    chain_lengths: HashMap<H256, usize>, // Track chain length for each block's hash
    tip: H256, // Track the tip of the longest chain
    ico_keypair: ring::signature::Ed25519KeyPair,  
}




impl Blockchain {
    pub fn generate_ico_keypair() -> Ed25519KeyPair {
        // Use fixed seed for consistent ICO address across all nodes
        let seed = [42u8; 32];  // Fixed seed value
        Ed25519KeyPair::from_seed_unchecked(&seed).unwrap()
    }
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        // Create a genesis block with fixed values
        let parent = H256::from([0u8; 32]); // Parent is all zeroes
        let nonce = 0;
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

         // Create ICO keypair
        let ico_keypair = Self::generate_ico_keypair();
        
        // Create ICO address from public key
        let ico_address = Address::from_public_key_bytes(
            ico_keypair.public_key().as_ref()
        );

        let mut states = HashMap::new();
        // Create initial state for genesis block
        let mut genesis_state = State::new();
        genesis_state.create_account(ico_address, 1_000_000); // Initial balance of 1,000,000
        states.insert(genesis_hash, genesis_state);

        let mut chain_lengths = HashMap::new();
        chain_lengths.insert(genesis_hash, 0); // Genesis block has height 0

        Self {
            blocks,
            states,
            chain_lengths,
            tip: genesis_hash, // The genesis block is the initial tip
            ico_keypair,
        }
    }

    /// Insert a block into the blockchain
    pub fn insert(&mut self, block: &Block) -> Result<(), BlockchainError> {
        let block_hash = block.hash();
        let parent_hash = block.get_parent();
    
        if let Some(parent_state) = self.states.get(&parent_hash) {
            // Process transactions to get new state
            let new_state = self.process_block_transactions(block, parent_state.clone());
            
            // Store block and its state
            self.blocks.insert(block_hash, block.clone());
            self.states.insert(block_hash, new_state);
            
            // Update chain length
            let new_length = self.chain_lengths[&parent_hash] + 1;
            self.chain_lengths.insert(block_hash, new_length);
            
            // Update tip if new chain is longer
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

    pub fn get_current_state(&self) -> State {
        self.states.get(&self.tip)
            .expect("Tip state must exist")
            .clone()
    }

    pub fn get_block(&self, hash: &H256) -> Option<&Block> {
        self.blocks.get(hash)
    }

     // Add this method
     pub fn get_ico_keypair(&self) -> &Ed25519KeyPair {
        &self.ico_keypair
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

    fn process_block_transactions(&mut self, block: &Block, parent_state: State) -> State {
        let mut new_state = parent_state.clone();
        
        // First validate all transactions together to ensure they're valid as a group
        let mut temp_state = new_state.clone();
        for tx in &block.content.data {
            // Check signature and nonce
            if !tx.verify(&temp_state) {
                return parent_state; // Invalid block, revert to parent state
            }
            
            // Try applying transaction
            match temp_state.process_transaction(tx) {
                Ok(_) => continue,
                Err(_) => return parent_state, // Invalid block, revert to parent state
            }
        }
        
        // If all transactions are valid, apply them to actual state
        for tx in &block.content.data {
            new_state.process_transaction(tx)
                .expect("Transaction validation already passed");
        }
        
        new_state
    }
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()  // Remove p2p_addr parameter
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