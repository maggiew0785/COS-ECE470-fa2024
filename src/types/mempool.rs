use std::collections::HashMap;
use super::{
    hash::{Hashable, H256},
    transaction::SignedTransaction,
};
use crate::Blockchain;
use crate::error;
use std::sync::{Arc, Mutex};
use crate::info;


#[derive(Debug, Default, Clone)]
pub struct Mempool {
    pub transactions: HashMap<H256, SignedTransaction>,
    max_block_size: usize,
    blockchain: Arc<Mutex<Blockchain>>,
}

impl Mempool {
    pub fn new(blockchain: Arc<Mutex<Blockchain>>) -> Self {
        Self {
            transactions: HashMap::new(),
            max_block_size: 100,
            blockchain,
        }
    }

    // In insert function:
    pub fn insert(&mut self, transaction: SignedTransaction) -> bool {
        let hash = transaction.hash();
        
        info!("Attempting to insert transaction {} into mempool", hash);

        if self.transactions.contains_key(&hash) {
            info!("Transaction already in mempool: {:?}", hash);
            return false;
        }
    
        info!("Adding transaction {:?} to mempool", hash);
        self.transactions.insert(hash, transaction);
        true
    }

    // Get transactions for block creation (up to max_block_size)
    pub fn get_transactions(&self) -> Vec<SignedTransaction> {
        self.transactions.values()
            .take(self.max_block_size)
            .cloned()
            .collect()
    }

    pub fn contains(&self, hash: &H256) -> bool {
        self.transactions.contains_key(hash)
    }

    // Remove transactions that were included in a block
    pub fn remove_transactions(&mut self, transactions: &[SignedTransaction]) {
        for tx in transactions {
            info!("Removing transaction {:?} from mempool", tx.hash());
            self.transactions.remove(&tx.hash());
        }
    }

    // Get a specific transaction by its hash
    pub fn get_transaction(&self, hash: &H256) -> Option<&SignedTransaction> {
        self.transactions.get(hash)
    }

    pub fn validate_transactions(&self) -> Vec<SignedTransaction> {
        let current_state = {
            let blockchain = self.blockchain.lock().unwrap();
            blockchain.states.get(&blockchain.tip())
                .expect("Tip state must exist")
                .clone()
        };
    
        // Filter valid transactions without modifying mempool
        self.transactions.values()
            .filter(|tx| {
                let is_valid = tx.verify(&current_state);
                if is_valid {
                    info!("Transaction {:?} passed validation", tx.hash());
                } else {
                    info!("Transaction {:?} failed validation", tx.hash());
                }
                is_valid
            })
            .cloned()
            .take(self.max_block_size)
            .collect()
    }
}