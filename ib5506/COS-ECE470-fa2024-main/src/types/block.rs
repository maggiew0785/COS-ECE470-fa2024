use serde::{Serialize, Deserialize};
use crate::types::hash::{H256, Hashable};
use std::time::{SystemTime, UNIX_EPOCH};
use ring::digest;
use rand::Rng;
use crate::types::merkle::MerkleTree;
use crate::types::transaction::SignedTransaction;


// Define the Header struct
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub parent: H256,
    pub nonce: u32,
    pub difficulty: H256,
    pub timestamp: u128,
    pub merkle_root: H256,
}

impl Hashable for Header {
    fn hash(&self) -> H256 {
        ring::digest::digest(&ring::digest::SHA256, &bincode::serialize(self).expect("Failed to serialize Header")).into()
    }
}

// Define the Content struct
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Content {
    pub data: Vec<SignedTransaction>,
}

// Define the Block struct that contains Header and Content
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: Header,
    pub content: Content,
}

// Implement Hashable for Block (only hashes the header)
impl Hashable for Block {
    fn hash(&self) -> H256 {
        self.header.hash()
    }
}

// Implement Block methods
impl Block {
    pub fn get_parent(&self) -> H256 {
        self.header.parent
    }

    pub fn get_difficulty(&self) -> H256 {
        self.header.difficulty
    }
}

// Function to generate the Merkle root using MerkleTree
pub fn compute_merkle_root(transactions: &[SignedTransaction]) -> H256 {
    let tx_hashes: Vec<H256> = transactions.iter().map(|tx| tx.hash()).collect();
    let merkle_tree = MerkleTree::new(&tx_hashes); // assuming MerkleTree can be created this way
    merkle_tree.root()
}

// Function to generate a random block
#[cfg(any(test, test_utilities))]
pub fn generate_random_block(parent: &H256) -> Block {
    let mut rng = rand::thread_rng();
    let nonce: u32 = rng.gen();
    let difficulty = H256::from([0xff; 32]); // Placeholder difficulty (can be changed)
    
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis();
    
    let content = Content {
        data: vec![], // Empty transactions for this example
    };
    
    let merkle_root = compute_merkle_root(&content.data);
    
    let header = Header {
        parent: *parent,
        nonce,
        difficulty,
        timestamp,
        merkle_root,
    };

    Block {
        header,
        content,
    }
}
