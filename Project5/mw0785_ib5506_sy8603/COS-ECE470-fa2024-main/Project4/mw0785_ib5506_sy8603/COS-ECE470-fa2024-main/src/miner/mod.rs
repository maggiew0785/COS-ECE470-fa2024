use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::types::hash::Hashable;

pub mod worker;

use log::info;

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::time;

use std::thread;

use crate::types::block::Block;

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Update, // update the block in mining, it may due to new blockchain tip or new transaction
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    finished_block_chan: Sender<Block>,
    pub blockchain: Arc<Mutex<Blockchain>>,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(blockchain: Arc<Mutex<Blockchain>>) -> (Context, Handle, Receiver<Block>) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    let (finished_block_sender, finished_block_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        finished_block_chan: finished_block_sender,
        blockchain: Arc::clone(&blockchain), // Pass blockchain
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle, finished_block_receiver)
}

#[cfg(any(test,test_utilities))]
fn test_new() -> (Context, Handle, Receiver<Block>) {
    // Create a new, empty blockchain for testing purposes
    let blockchain = Blockchain::new();
    let blockchain = Arc::new(Mutex::new(blockchain));  // Wrap it in Arc<Mutex<>> for thread-safe access

    // Call the modified new() function, passing the new blockchain
    new(Arc::clone(&blockchain))
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

    pub fn update(&self) {
        self.control_chan.send(ControlSignal::Update).unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn miner_loop(&mut self) {
        // main mining loop
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Miner shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Miner starting in continuous mode with lambda {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Update => {
                            // in paused state, don't need to update
                        }
                    };
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Miner shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Miner starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                unimplemented!()
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // TODO for student: actual mining, create a block
            use rand::Rng;
            use std::time::{SystemTime, UNIX_EPOCH};

            // 1. Get the parent block hash from the blockchain tip
            let blockchain = self.blockchain.lock().expect("Failed to lock blockchain");
            let mut parent_hash = blockchain.tip();

            // 2. Generate the current timestamp in milliseconds
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();

            // 3. Set difficulty as the same as the parent block (static for this project)
            let difficulty = [255u8; 32].into();
            //WATCH OUT
            drop(blockchain);

            // 4. Compute the Merkle root (using empty transactions in this phase)
            use crate::types::block::compute_merkle_root;
            let transactions = vec![];  // No transactions, so we use an empty list
            let merkle_root = compute_merkle_root(&transactions);

            // 5. Mining loop: Generate a random nonce and create the block
            let mut rng = rand::thread_rng();
            loop {
                let nonce: u32 = rng.gen();  // Generate a random nonce

                let header = crate::types::block::Header {
                    parent: parent_hash,
                    nonce,
                    difficulty,
                    timestamp: timestamp as u128,
                    merkle_root,
                };

                let block = crate::types::block::Block {
                    header,
                    content: crate::types::block::Content {
                        data: transactions.clone(),
                    },
                };

                // 6. Check if the block hash satisfies the difficulty (Proof-of-Work check)
                if block.hash() <= difficulty {

                    // Insert the mined block directly into the blockchain COULD BE DELETED LATER
                    {
                        let mut blockchain_guard = self.blockchain.lock().expect("Failed to lock blockchain");
                        blockchain_guard.insert(&block).expect("Failed to insert block into blockchain");
                    }

                    // Send the block through the finished_block_chan if needed (optional, if worker is bypassed)
                    self.finished_block_chan.send(block.clone()).expect("Failed to send finished block");

                    // Update the parent to the new block's hash for the next iteration
                    parent_hash = block.hash();
                    break;
                }
            }

            // TODO for student: if block mining finished, you can have something like: self.finished_block_chan.send(block.clone()).expect("Send finished block error");

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST


#[cfg(test)]
mod test {
    use ntest::timeout;
    use crate::types::hash::Hashable;

    #[test]
    #[timeout(60000)]
    fn sp2022autograder031() {
        let (miner_ctx, miner_handle, finished_block_chan) = super::test_new();
        miner_ctx.start();
        miner_handle.start(0);
        let mut block_prev = finished_block_chan.recv().unwrap();
        for _ in 0..2 {
            let block_next = finished_block_chan.recv().unwrap();
            assert_eq!(block_prev.hash(), block_next.get_parent());
            block_prev = block_next;
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST