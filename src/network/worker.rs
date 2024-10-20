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
    pub buffer: HashMap<H256, Block>, // Buffer for blocks waiting for parents
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
            buffer: HashMap::new(),
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
    
                        let mut blockchain = self.blockchain.lock().unwrap();
    
                        // Check if the parent block is already in the blockchain
                        if blockchain.blocks.contains_key(&parent_hash) {
                            // Parent exists, insert the block into the blockchain
                            if blockchain.insert(&block).is_ok() {
                                info!("Block inserted: {:?}", block_hash);
                                self.server.broadcast(Message::NewBlockHashes(vec![block_hash]));
    
                                // Check if any buffered blocks can now be inserted
                                if let Some(child_block) = self.buffer.remove(&block_hash) {
                                    if blockchain.insert(&child_block).is_ok() {
                                        let child_hash = child_block.hash();
                                        info!("Buffered child block inserted: {:?}", child_hash);
                                        self.server.broadcast(Message::NewBlockHashes(vec![child_hash]));
                                    }
                                }
                            }
                        } else {
                            // Parent is missing, buffer the block
                            info!("Parent not found for block: {:?}. Buffering block.", block_hash);
                            self.buffer.insert(block_hash, block);
    
                            // Send a GetBlocks request to get the missing parent
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