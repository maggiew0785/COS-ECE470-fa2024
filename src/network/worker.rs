use super::message::Message;
use super::peer;
use super::server::Handle as ServerHandle;
use crate::types::hash::{H256, Hashable};
use crate::types::block::Block;
use crate::types::mempool::Mempool;
use crate::blockchain::Blockchain;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crossbeam::channel::Sender;


use log::{debug, info, warn, error};

use std::thread;

#[cfg(any(test,test_utilities))]
use super::peer::TestReceiver as PeerTestReceiver;
#[cfg(any(test,test_utilities))]
use super::server::TestReceiver as ServerTestReceiver;
#[derive(Clone)]
pub struct Worker {
    // The blockchain is now thread-safe using Arc<Mutex<Blockchain>>
    pub blockchain: Arc<Mutex<Blockchain>>,
    pub mempool: Arc<Mutex<Mempool>>,
    // Change buffer to map from parent_hash -> blocks waiting for that parent
    orphan_buffer: HashMap<H256, Vec<Block>>, // parent_hash -> blocks
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
}


impl Worker {
    pub fn new(
        blockchain: Arc<Mutex<Blockchain>>,
        mempool: Arc<Mutex<Mempool>>,
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
    ) -> Self {
        Self {
            blockchain,
            mempool,
            orphan_buffer: HashMap::new(),
            msg_chan: msg_src,
            num_worker,
            server: server.clone(),
        }
    }

    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let mut cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    fn worker_loop(&mut self) {
        loop {
            let result = smol::block_on(self.msg_chan.recv());
            if let Err(e) = result {
                error!("network worker terminated {}", e);
                break;
            }
            let msg = result.unwrap();
            let (msg, mut peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            info!("Received message: {:?}", msg);
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                Message::NewBlockHashes(hashes) => {
                    let mut blocks_to_request = Vec::new();
                    {
                        let blockchain = self.blockchain.lock().unwrap();
                        for hash in hashes {
                            if !blockchain.blocks.contains_key(&hash) {
                                blocks_to_request.push(hash);
                            }
                        }
                    }
                    if !blocks_to_request.is_empty() {
                        peer.write(Message::GetBlocks(blocks_to_request));
                    }
                }
                // Handle GetBlocks
                Message::GetBlocks(hashes) => {
                    let mut blocks = Vec::new();
                    let blockchain = self.blockchain.lock().unwrap();
                    for hash in hashes {
                        if let Some(block) = blockchain.blocks.get(&hash) {
                            blocks.push(block.clone());
                        }
                    }
                    if !blocks.is_empty() {
                        peer.write(Message::Blocks(blocks));
                    }
                }
                // Handle Blocks
                Message::Blocks(blocks) => {
                    for block in blocks {
                        let parent_hash = block.get_parent();
                        let block_hash = block.hash();
                
                        // First check if we already have this block
                        {
                            let blockchain = self.blockchain.lock().unwrap();
                            if blockchain.blocks.contains_key(&block_hash) {
                                debug!("Block already known: {:?}", block_hash);
                                continue;
                            }
                            drop(blockchain);
                        }
                
                        // Check parent existence BEFORE doing expensive validations
                        {
                            let blockchain = self.blockchain.lock().unwrap();
                            if !blockchain.blocks.contains_key(&parent_hash) {
                                info!("Parent block not found: {:?}, buffering block: {:?}", parent_hash, block_hash);
                                drop(blockchain);
                                
                                // Add to orphan buffer and request parent
                                self.orphan_buffer
                                    .entry(parent_hash)
                                    .or_insert_with(Vec::new)
                                    .push(block);
                                peer.write(Message::GetBlocks(vec![parent_hash]));
                                continue;
                            }
                            drop(blockchain);
                        }
                
                        // Now validate PoW
                        if block.hash() > block.header.difficulty {
                            warn!("Block failed PoW check: {:?}", block_hash);
                            continue;
                        }
                
                         // Validate transactions
                        let mut all_transactions_valid = true;
                        {
                            let blockchain = self.blockchain.lock().unwrap();
                            if let Some(parent_state) = blockchain.states.get(&parent_hash) {
                                for tx in &block.content.data {
                                    if !tx.verify(parent_state) {
                                        warn!("Invalid transaction in block: {:?}", block_hash);
                                        all_transactions_valid = false;
                                        break;
                                    }
                                }
                            } else {
                                error!("Parent state missing for block: {:?}", block_hash);
                                all_transactions_valid = false;
                            }
                            drop(blockchain);
                        }

                        if !all_transactions_valid {
                            continue;
                        }
                
                        // Try to insert the block
                        let insert_success = {
                            let mut blockchain = self.blockchain.lock().unwrap();
                            let success = blockchain.insert(&block).is_ok();
                            drop(blockchain);
                            success
                        };

                        if insert_success {
                            info!("Block inserted: {:?}", block_hash);
                            
                            // Update mempool in separate lock scope
                            {
                                let mut mempool = self.mempool.lock().unwrap();
                                mempool.remove_transactions(&block.content.data);
                                drop(mempool);
                            }
                            
                            // Broadcast after all locks are released
                            self.server.broadcast(Message::NewBlockHashes(vec![block_hash]));
                            
                            // Process orphans after all other operations
                            self.process_orphans(block_hash);
                        }
                    }
                }

                Message::NewTransactionHashes(hashes) => {
                    let mut txs_to_request = Vec::new();
                    {
                        let mempool = self.mempool.lock().unwrap();
                        for hash in hashes {
                            if !mempool.contains(&hash) {
                                txs_to_request.push(hash);
                            }
                        }
                        drop(mempool);
                    }
                    if !txs_to_request.is_empty() {
                        peer.write(Message::GetTransactions(txs_to_request));
                    }
                }

                Message::GetTransactions(hashes) => {
                    let mut transactions = Vec::new();
                    let mempool = self.mempool.lock().unwrap();
                    for hash in hashes {
                        if let Some(tx) = mempool.get_transaction(&hash) {
                            transactions.push(tx.clone());
                        }
                    }
                    drop(mempool);
                    if !transactions.is_empty() {
                        peer.write(Message::Transactions(transactions));
                    }
                }

                Message::Transactions(transactions) => {
                    // Validate transactions with consistent state
                    let mut valid_transactions = Vec::new();
                    {
                        let blockchain = self.blockchain.lock().unwrap();
                        let current_state = blockchain.states.get(&blockchain.tip())
                            .expect("Tip state must exist");
                            
                        // Validate while holding the lock
                        for tx in transactions {
                            if tx.verify(current_state) {
                                valid_transactions.push(tx);
                            }
                        }
                        drop(blockchain);
                    }
                    
                    // Process valid transactions
                    if !valid_transactions.is_empty() {
                        let mut mempool = self.mempool.lock().unwrap();
                        for tx in valid_transactions {
                            if mempool.insert(tx.clone()) {
                                // Only broadcast after successful insertion
                                drop(mempool);  // Release lock before network operation
                                self.server.broadcast(Message::NewTransactionHashes(vec![tx.hash()]));
                                mempool = self.mempool.lock().unwrap();  // Re-acquire for next iteration
                            }
                        }
                        drop(mempool);
                    }
                }

                _ => unimplemented!(),
            }
        }
    }
}

impl Worker {
    fn process_orphans(&mut self, parent_hash: H256) {
        let mut blocks_to_process = Vec::new();
        let mut current_hash = parent_hash;
        
        // Collect all orphans that can now be processed
        while let Some(orphans) = self.orphan_buffer.remove(&current_hash) {
            for orphan in orphans {
                let orphan_hash = orphan.hash();
                blocks_to_process.push(orphan);
                current_hash = orphan_hash;
            }
        }
    
        // Process collected orphans
        for block in blocks_to_process {
            let block_hash = block.hash();
            
            // Validate PoW before taking any locks
            if block.hash() > block.header.difficulty {
                warn!("Orphaned block failed PoW check: {:?}", block_hash);
                continue;
            }
    
            // Try to insert block
            let insert_success = {
                let mut blockchain = self.blockchain.lock().unwrap();
                let success = blockchain.insert(&block).is_ok();
                drop(blockchain);
                success
            };
    
            if insert_success {
                info!("Orphaned block inserted: {:?}", block_hash);
                
                // Update mempool in separate lock scope
                {
                    let mut mempool = self.mempool.lock().unwrap();
                    mempool.remove_transactions(&block.content.data);
                    drop(mempool);
                }
                
                // Broadcast after all locks are released
                self.server.broadcast(Message::NewBlockHashes(vec![block_hash]));
            } else {
                // If insertion failed, put block back in orphan buffer
                self.orphan_buffer.entry(current_hash)
                    .or_insert_with(Vec::new)
                    .push(block);
            }
        }
    }
}

#[cfg(any(test,test_utilities))]
struct TestMsgSender {
    s: smol::channel::Sender<(Vec<u8>, peer::Handle)>
}
#[cfg(any(test,test_utilities))]
impl TestMsgSender {
    fn new() -> (TestMsgSender, smol::channel::Receiver<(Vec<u8>, peer::Handle)>) {
        let (s,r) = smol::channel::unbounded();
        (TestMsgSender {s}, r)
    }

    fn send(&self, msg: Message) -> PeerTestReceiver {
        let bytes = bincode::serialize(&msg).unwrap();
        let (handle, r) = peer::Handle::test_handle();
        smol::block_on(self.s.send((bytes, handle))).unwrap();
        r
    }
}
#[cfg(any(test,test_utilities))]
/// returns two structs used by tests, and an ordered vector of hashes of all blocks in the blockchain
fn generate_test_worker_and_start() -> (TestMsgSender, ServerTestReceiver, Vec<H256>) {
    let (server, server_receiver) = ServerHandle::new_for_test();
    let (test_msg_sender, msg_chan) = TestMsgSender::new();

    // Initialize blockchain and pass it to worker
    let blockchain = Blockchain::new();
    let blockchain = Arc::new(Mutex::new(blockchain));

    let worker = Worker::new(Arc::clone(&blockchain), 1, msg_chan, &server);
    worker.start(); 


    // Get the hashes of the longest chain in the blockchain (should contain the genesis block initially)
    let longest_chain_hashes = {
        let blockchain = blockchain.lock().unwrap();
        blockchain.all_blocks_in_longest_chain()
    };

    // Return the test message sender, server test receiver, and the longest chain hashes
    (test_msg_sender, server_receiver, longest_chain_hashes)
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod test {
    use ntest::timeout;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    use super::super::message::Message;
    use super::generate_test_worker_and_start;

    #[test]
    #[timeout(60000)]
    fn reply_new_block_hashes() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut peer_receiver = test_msg_sender.send(Message::NewBlockHashes(vec![random_block.hash()]));
        let reply = peer_receiver.recv();
        if let Message::GetBlocks(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_get_blocks() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let h = v.last().unwrap().clone();
        let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(vec![h.clone()]));
        let reply = peer_receiver.recv();
        if let Message::Blocks(v) = reply {
            assert_eq!(1, v.len());
            assert_eq!(h, v[0].hash())
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_blocks() {
        let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(vec![random_block.clone()]));
        let reply = server_receiver.recv().unwrap();
        if let Message::NewBlockHashes(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST