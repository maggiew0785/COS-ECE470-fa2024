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
use crate::blockchain::retrieve_keypair;  // Import the standalone function


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
        p2p_addr: std::net::SocketAddr,
    ) -> Self {
        let keypair = retrieve_keypair(p2p_addr);
        let address = Address::from_public_key_bytes(keypair.public_key().as_ref());
        info!("Transaction generator initialized with address: {:?}", address);
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
        let mut nonce = 1;  // Initialize nonce counter
        loop {
            let sender_address = Address::from_public_key_bytes(self.keypair.public_key().as_ref());
            let transaction = {
                
                // Create random receiver address
                let receiver = Address::from_public_key_bytes(key_pair::random().public_key().as_ref());
                
                Transaction {
                    receiver,
                    value: rand::random::<u64>() % 100,  // Random value between 0-99
                    nonce,
                }
            };
            info!("Created transaction: sender={:?}, receiver={:?}, value={}, nonce={}", 
                  sender_address, transaction.receiver, transaction.value, transaction.nonce);
            let signed_tx = SignedTransaction::new(transaction, &self.keypair);
            info!("Created transaction with hash: {:?}", signed_tx.hash());
            
            // Broadcast the transaction hash to network
            info!("Broadcasting transaction to network");
                        // Insert into mempool before broadcasting
            {
                let mut mempool = self.mempool.lock().unwrap();
                if mempool.insert(signed_tx.clone()) {
                    info!("Transaction added to mempool, broadcasting to network");
                    // Only broadcast if successfully added to mempool
                    self.network.broadcast(Message::NewTransactionHashes(vec![signed_tx.hash()]));
                } else {
                    info!("Transaction already in mempool, skipping broadcast");
                }
                info!("GENERATORMempool contains {} transactions", mempool.transactions.len());
                
            }

            // Increment nonce for next transaction
            nonce += 1;

            if theta != 0 {
                let interval = Duration::from_millis(1 * theta);
                thread::sleep(interval);
            }
        }
    }
}

impl Clone for TransactionGenerator {
    fn clone(&self) -> Self {
        // Use same keypair derivation as original
        let keypair = retrieve_keypair(self.network.p2p_addr);
        
        Self {
            network: self.network.clone(),
            mempool: Arc::clone(&self.mempool),
            blockchain: Arc::clone(&self.blockchain),
            keypair,
        }
    }
}
