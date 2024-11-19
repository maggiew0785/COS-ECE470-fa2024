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
use crate::info;

#[derive(Debug)]
#[derive(Clone)]  // Add this line
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
}

use lazy_static::lazy_static;

// Define the three ICO keypairs as static variables

lazy_static! {
    // Store the seeds instead of the keypairs
    static ref ICO_SEEDS: [[u8; 32]; 3] = [
        [1u8; 32],  // Fixed seed for first keypair
        [2u8; 32],  // Fixed seed for second keypair
        [3u8; 32],  // Fixed seed for third keypair
    ];
}

// Function to retrieve the appropriate keypair based on P2P address
pub fn retrieve_keypair(p2p_addr: std::net::SocketAddr) -> Ed25519KeyPair {
    let index = (p2p_addr.port() % 3) as usize;
    Ed25519KeyPair::from_seed_unchecked(&ICO_SEEDS[index]).unwrap()
}

impl Blockchain {

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


        // Initialize genesis state with all three ICO addresses
        let mut genesis_state = State::new();
        
        // Create accounts for each ICO seed
        for seed in ICO_SEEDS.iter() {
            let keypair = Ed25519KeyPair::from_seed_unchecked(seed).unwrap();
            let address = Address::from_public_key_bytes(keypair.public_key().as_ref());
            genesis_state.create_account(address, 10_000_000); // 10M coins each
}
        let mut states = HashMap::new();
        states.insert(genesis_hash, genesis_state);

        let mut chain_lengths = HashMap::new();
        chain_lengths.insert(genesis_hash, 0); // Genesis block has height 0

        Self {
            blocks,
            states,
            chain_lengths,
            tip: genesis_hash, // The genesis block is the initial tip
        }
    }

    /// Insert a block into the blockchain
    pub fn insert(&mut self, block: &Block) -> Result<(), BlockchainError> {
        let block_hash = block.hash();
        let parent_hash = block.get_parent();
    
        if let Some(parent_state) = self.states.get(&parent_hash) {
            // Process transactions to get new state
            let new_state = self.process_block_transactions(block, parent_state.clone())?;  // Add ? here
            
            // Store block and its state
            self.blocks.insert(block_hash, block.clone());
            self.states.insert(block_hash, new_state);  // Remove clone() since new_state is already owned
            
            // Update chain length
            let new_length = self.chain_lengths[&parent_hash] + 1;
            self.chain_lengths.insert(block_hash, new_length);
            
            // Update tip if new chain is longer
            if new_length > self.chain_lengths[&self.tip] {
                self.tip = block_hash;
            }
            
            Ok(())
        } else {
            info!("Block not inserted because parent state not found");
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

    fn process_block_transactions(&mut self, block: &Block, parent_state: State) -> Result<State, BlockchainError> {
        let mut new_state = parent_state;
        
        // Process each transaction, returning error if any fail
        for tx in &block.content.data {
            if let Err(_) = new_state.process_transaction(tx) {
                info!("");
            }
        }
        
        Ok(new_state)
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