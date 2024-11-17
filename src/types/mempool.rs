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
        
        if self.transactions.contains_key(&hash) {
            info!("Transaction already in mempool: {:?}", hash);
            return false;
        }
    
        let is_valid = {
            let blockchain = self.blockchain.lock().unwrap();
            match blockchain.states.get(&blockchain.tip()) {
                Some(current_state) => {
                    info!("Validating transaction in mempool");
                    let valid = transaction.verify(current_state);
                    if valid {
                        info!("Transaction validation successful in mempool");
                    } else {
                        error!("Transaction validation failed in mempool");
                    }
                    valid
                },
                None => {
                    error!("Could not get current state from blockchain");
                    false
                }
            }
        };
    
        if is_valid {
            info!("Adding transaction {:?} to mempool", hash);
            self.transactions.insert(hash, transaction);
            true
        } else {
            error!("Transaction {:?} not added to mempool due to validation failure", hash);
            false
        }
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
            self.transactions.remove(&tx.hash());
        }
    }

    // Get a specific transaction by its hash
    pub fn get_transaction(&self, hash: &H256) -> Option<&SignedTransaction> {
        self.transactions.get(hash)
    }

    pub fn validate_transactions(&mut self) {
        let current_state = {
            let blockchain = self.blockchain.lock().unwrap();
            blockchain.states.get(&blockchain.tip())
                .expect("Tip state must exist")
                .clone()
        };
    
        let mut temp_state = current_state.clone();
        let mut valid_txs = Vec::new();
        
        // First pass: collect valid transactions in order
        for (_, tx) in &self.transactions {
            if tx.verify(&temp_state) && temp_state.process_transaction(tx).is_ok() {
                valid_txs.push(tx.hash());
            }
        }
        
        // Second pass: retain only valid transactions
        self.transactions.retain(|hash, _| valid_txs.contains(hash));
    }
}