use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use log::info;  // Keep this for logging
use rand;

use crate::network::server::Handle as NetworkServerHandle;
use crate::types::mempool::Mempool;
use crate::blockchain::Blockchain;
use crate::types::transaction::{Transaction, SignedTransaction};
use crate::types::key_pair;
use ring::signature::KeyPair;
use crate::types::address::Address;
use crate::network::message::Message;
use crate::types::hash::Hashable;

#[derive(Clone)]
pub struct TransactionGenerator {
    network: NetworkServerHandle,
    mempool: Arc<Mutex<Mempool>>,
    blockchain: Arc<Mutex<Blockchain>>,
}

impl TransactionGenerator {
    pub fn new(network: NetworkServerHandle, mempool: Arc<Mutex<Mempool>>, blockchain: Arc<Mutex<Blockchain>>) -> Self {
        Self {
            network,
            mempool,
            blockchain,
        }
    }

    pub fn start(self, theta: u64) {
        info!("Transaction generator starting with theta {}", theta);
        thread::Builder::new()
            .name("transaction-generator".to_string())
            .spawn(move || {
                self.generate_transactions(theta);
            })
            .unwrap();
    }

    fn generate_transactions(&self, theta: u64) {
        loop {
            let key_pair = key_pair::random();
            let transaction = Transaction {
                receiver: Address::from_public_key_bytes(key_pair.public_key().as_ref()),
                value: rand::random::<u64>() % 100,
                nonce: 1,
            };
            
            let signed_tx = SignedTransaction::new(transaction, &key_pair);
            
            // Add to mempool with blockchain reference
            {
                let mut mempool = self.mempool.lock().unwrap();
                mempool.insert(signed_tx.clone());
                drop(mempool);
            }
            
            // Broadcast transaction hash
            self.network.broadcast(Message::NewTransactionHashes(
                vec![signed_tx.hash()]
            ));

            if theta != 0 {
                let interval = Duration::from_millis(1 * theta);
                thread::sleep(interval);
            }
        }
    }
}