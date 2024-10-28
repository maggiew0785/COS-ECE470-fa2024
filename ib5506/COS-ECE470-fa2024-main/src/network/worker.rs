use super::message::Message;
use super::peer;
use super::server::Handle as ServerHandle;
use crate::types::hash::{H256, Hashable};
use crate::types::block::Block;
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
    // Change buffer to map from parent_hash -> blocks waiting for that parent
    orphan_buffer: HashMap<H256, Vec<Block>>, // parent_hash -> blocks
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
}


impl Worker {
    pub fn new(
        blockchain: Arc<Mutex<Blockchain>>,
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
    ) -> Self {
        Self {
            blockchain,
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

                        // 1. PoW Validity Check
                        if block.hash() > block.header.difficulty {
                            warn!("Block failed PoW check: {:?}", block_hash);
                            continue;
                        }

                        let mut blockchain = self.blockchain.lock().unwrap();

                        // 2. Parent Check & Difficulty Consistency Check
                        if let Some(parent_block) = blockchain.blocks.get(&parent_hash) {
                            // Check difficulty consistency with parent
                            if block.header.difficulty != parent_block.header.difficulty {
                                warn!("Block difficulty mismatch with parent: {:?}", block_hash);
                                continue;
                            }

                            // Parent exists, try to insert the block
                            if blockchain.insert(&block).is_ok() {
                                info!("Block inserted: {:?}", block_hash);
                                self.server.broadcast(Message::NewBlockHashes(vec![block_hash]));

                                // Process any orphaned children
                                // Keep processing orphans as long as we find children
                                let mut current_parent = block_hash;
                                while let Some(orphaned_children) = self.orphan_buffer.remove(&current_parent) {
                                    for orphan_block in orphaned_children {
                                        if orphan_block.hash() <= orphan_block.header.difficulty {
                                            if blockchain.insert(&orphan_block).is_ok() {
                                                let orphan_hash = orphan_block.hash();
                                                info!("Orphaned block inserted: {:?}", orphan_hash);
                                                self.server.broadcast(Message::NewBlockHashes(vec![orphan_hash]));
                                                current_parent = orphan_hash;
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Parent missing, add to orphan buffer
                            info!("Parent not found for block: {:?}. Buffering block.", block_hash);
                            self.orphan_buffer
                                .entry(parent_hash)
                                .or_insert_with(Vec::new)
                                .push(block);

                            // Request missing parent
                            peer.write(Message::GetBlocks(vec![parent_hash]));
                        }
                    }
                }
                _ => unimplemented!(),
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