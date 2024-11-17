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
use ring::signature::Ed25519KeyPair;
use crate::types::address::Address;
use crate::network::message::Message;
use crate::error;
use crate::types::hash::Hashable;
use crate::network::server::Handle as ServerHandle;


pub struct TransactionGenerator {
    network: NetworkServerHandle,
    mempool: Arc<Mutex<Mempool>>,
    blockchain: Arc<Mutex<Blockchain>>,
    keypair: ring::signature::Ed25519KeyPair,
}

impl TransactionGenerator {
    pub fn new(
        network: ServerHandle, 
        mempool: Arc<Mutex<Mempool>>, 
        blockchain: Arc<Mutex<Blockchain>>,
    ) -> Self {
        let keypair = {
            let blockchain = blockchain.lock().unwrap();
            // Clone the keypair bytes and create a new one
            let seed = [42u8; 32];  // Use same seed as blockchain
            Ed25519KeyPair::from_seed_unchecked(&seed).unwrap()
        };
        Self {
            network,
            mempool,
            blockchain,
            keypair,
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
            let transaction = {
                let blockchain = self.blockchain.lock().unwrap();
                let current_state = blockchain.states.get(&blockchain.tip())
                    .expect("Tip state must exist");
                
                // Create random receiver address
                let receiver = Address::from_public_key_bytes(key_pair::random().public_key().as_ref());
                
                // Get sender's current nonce
                let sender_address = Address::from_public_key_bytes(self.keypair.public_key().as_ref());
                let nonce = if let Some(account) = current_state.get_account_state(&sender_address) {
                    account.nonce + 1
                } else {
                    1  // Start with nonce 1 if account doesn't exist
                };
    
                Transaction {
                    receiver,
                    value: rand::random::<u64>() % 100,  // Random value between 0-99
                    nonce,
                }
            };
    
            let signed_tx = SignedTransaction::new(transaction, &self.keypair);
            info!("Created transaction with hash: {:?}", signed_tx.hash());
    
            // First verify with blockchain state
            let is_valid = {
                let blockchain = self.blockchain.lock().unwrap();
                let current_state = blockchain.states.get(&blockchain.tip())
                    .expect("Tip state must exist");                
                
                info!("Verifying transaction against current state");
                signed_tx.verify(current_state)
            }; // blockchain lock is dropped here
    
            // Then handle mempool operations separately
            if is_valid {
                info!("Transaction verified successfully");
                let mut mempool = self.mempool.lock().unwrap();
                if mempool.insert(signed_tx.clone()) {
                    info!("Transaction added to mempool, current size: {}", mempool.transactions.len());
                    drop(mempool); // Drop mempool lock before broadcast
                    
                    info!("Broadcasting transaction to network");
                    self.network.broadcast(Message::NewTransactionHashes(vec![signed_tx.hash()]));
                } else {
                    error!("Failed to add transaction to mempool");
                    drop(mempool);
                }
            } else {
                error!("Transaction verification failed");
            }
    
            if theta != 0 {
                let interval = Duration::from_millis(1 * theta);
                thread::sleep(interval);
            }
        }
    }
}

impl Clone for TransactionGenerator {
    fn clone(&self) -> Self {
        // Create new keypair with same seed when cloning
        let seed = [42u8; 32];
        let keypair = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        
        Self {
            network: self.network.clone(),
            mempool: Arc::clone(&self.mempool),
            blockchain: Arc::clone(&self.blockchain),
            keypair,
        }
    }
}