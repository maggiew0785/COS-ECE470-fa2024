use std::collections::HashMap;
use super::{
    hash::{Hashable, H256},
    transaction::SignedTransaction,
};
use crate::Blockchain;

#[derive(Debug, Default, Clone)]
pub struct Mempool {
    transactions: HashMap<H256, SignedTransaction>,
    max_block_size: usize,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            max_block_size: 100, // You can adjust this value
        }
    }

    pub fn insert(&mut self, transaction: SignedTransaction) -> bool {
        let hash = transaction.hash();
        if !self.transactions.contains_key(&hash) && transaction.verify() {
            self.transactions.insert(hash, transaction);
            true
        } else {
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
}