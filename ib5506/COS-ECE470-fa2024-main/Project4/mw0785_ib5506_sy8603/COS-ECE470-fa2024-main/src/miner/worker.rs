use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use log::{debug, info};
use crate::types::block::Block;
use crate::network::server::Handle as ServerHandle;
use std::thread;
use std::sync::{Arc, Mutex};
use crate::Blockchain;
use crate::types::hash::Hashable;


#[derive(Clone)]
pub struct Worker {
    server: ServerHandle,
    finished_block_chan: Receiver<Block>,
    pub blockchain: Arc<Mutex<Blockchain>>, // Add blockchain
}

impl Worker {
    pub fn new(
        server: &ServerHandle,
        finished_block_chan: Receiver<Block>,
        blockchain: Arc<Mutex<Blockchain>>, // Add blockchain argument
    ) -> Self {
        Self {
            server: server.clone(),
            finished_block_chan,
            blockchain: Arc::clone(&blockchain), // Clone and store blockchain
        }
    }

    pub fn start(self) {
        thread::Builder::new()
            .name("miner-worker".to_string())
            .spawn(move || {
                self.worker_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn worker_loop(&self) {
        loop {
            let block = self.finished_block_chan.recv().expect("Receive finished block error");
            // TODO for student: insert this finished block to blockchain, and broadcast this block hash
             // Lock the blockchain and insert the block
            {
                //println!("Inserting block with hash: {:?}", block.hash());
                let mut blockchain = self.blockchain.lock().expect("Failed to lock the blockchain");
                blockchain.insert(&block).expect("Failed to insert block into blockchain");
                //println!("Blockchain tip updated to block hash: {:?}", block.hash());
            } // The lock is automatically released here
            
            // Logging the insertion of the block
            //info!("Block inserted into the blockchain: {:?}", block.hash());
        }
    }
}
