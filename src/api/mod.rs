use serde::Serialize;
use crate::miner::Handle as MinerHandle;
use crate::network::server::Handle as NetworkServerHandle;
use crate::network::message::Message;
use crate::generator::TransactionGenerator;
use crate::types::mempool::Mempool;  // Update the path
use crate::types::block::Block;
use crate::types::hash::H256;
use crate::types::hash::Hashable;
use crate::Blockchain;

use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::Response;
use tiny_http::Server as HTTPServer;
use url::Url;

use serde_json;
use hex;
use tiny_http::{StatusCode, Header};

pub struct Server {
    handle: HTTPServer,
    miner: MinerHandle,
    network: NetworkServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    tx_generator: Arc<Mutex<TransactionGenerator>>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

macro_rules! respond_result {
    ( $req:expr, $success:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let payload = ApiResponse {
            success: $success,
            message: $message.to_string(),
        };
        let resp = Response::from_string(serde_json::to_string_pretty(&payload).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}
macro_rules! respond_json {
    ( $req:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let resp = Response::from_string(serde_json::to_string(&$message).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}

impl Server {
    pub fn start(
        addr: std::net::SocketAddr,
        miner: &MinerHandle,
        network: &NetworkServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
        mempool: &Arc<Mutex<Mempool>>,  // Add this parameter
    ) {
        let handle = HTTPServer::http(&addr).unwrap();
        let tx_generator = Arc::new(Mutex::new(
            TransactionGenerator::new(
                network.clone(), 
                Arc::clone(mempool), 
                Arc::clone(blockchain),
                network.p2p_addr,
            )
        ));
        let server = Arc::new(Self {
            handle,
            miner: miner.clone(),
            network: network.clone(),
            blockchain: Arc::clone(blockchain),
            tx_generator: tx_generator,
        });
        thread::spawn(move || {
            let server_clone = Arc::clone(&server);
            for req in server_clone.handle.incoming_requests() {
                let server_clone = Arc::clone(&server_clone);
                let miner = server_clone.miner.clone();
                let network = server_clone.network.clone();
                let blockchain = Arc::clone(&server_clone.blockchain);
                
                thread::spawn(move || {
                    // a valid url requires a base
                    let base_url = Url::parse(&format!("http://{}/", &addr)).unwrap();
                    let url = match base_url.join(req.url()) {
                        Ok(u) => u,
                        Err(e) => {
                            respond_result!(req, false, format!("error parsing url: {}", e));
                            return;
                        }
                    };
                    match url.path() {
                        "/miner/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let lambda = match params.get("lambda") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing lambda");
                                    return;
                                }
                            };
                            let lambda = match lambda.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing lambda: {}", e)
                                    );
                                    return;
                                }
                            };
                            miner.start(lambda);
                            respond_result!(req, true, "ok");
                        }
                        "/tx-generator/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let theta = match params.get("theta") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing theta parameter");
                                    return;
                                }
                            };
                            let theta = match theta.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing theta: {}", e)
                                    );
                                    return;
                                }
                            };
                            
                            // Clone the generator outside the lock
                            let generator = {
                                let tx_generator = server_clone.tx_generator.lock().unwrap();
                                tx_generator.clone()
                            };  // Lock is dropped here
                            
                            // Start the generator after dropping the lock
                            generator.start(theta);
                            respond_result!(req, true, "transaction generator started");
                        }
                        "/network/ping" => {
                            network.broadcast(Message::Ping(String::from("Test ping")));
                            respond_result!(req, true, "ok");
                        }
                        "/blockchain/state" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            
                            // Get block parameter
                            let block_height = match params.get("block") {
                                Some(v) => match v.parse::<u32>() {
                                    Ok(h) => h,
                                    Err(e) => {
                                        respond_result!(req, false, format!("error parsing block height: {}", e));
                                        return;
                                    }
                                },
                                None => {
                                    respond_result!(req, false, "missing block parameter");
                                    return;
                                }
                            };

                            // Get state at specified block height
                            let result = {
                                let blockchain = blockchain.lock().unwrap();
                                let chain = blockchain.all_blocks_in_longest_chain();
                                
                                if (block_height as usize) >= chain.len() {
                                    respond_result!(req, false, "block height exceeds chain length");
                                    return;
                                }

                                // Get block hash at specified height
                                let block_hash = chain[block_height as usize];
                                
                                // Get state for that block
                                if let Some(state) = blockchain.states.get(&block_hash) {
                                    // Convert state entries to strings
                                    let mut entries: Vec<String> = state
                                        .accounts
                                        .iter()
                                        .map(|(addr, account)| {
                                            format!("({}, {}, {})", 
                                                hex::encode(addr.as_bytes()),
                                                account.nonce,
                                                account.balance
                                            )
                                        })
                                        .collect();
                                    
                                    // Sort entries for consistent ordering
                                    entries.sort();
                                    entries
                                } else {
                                    respond_result!(req, false, "state not found for block");
                                    return;
                                }
                            };
                            drop(blockchain);
                            respond_json!(req, result);
                        }
                        "/blockchain/longest-chain" => {
                            let blockchain = blockchain.lock().unwrap();
                            let v = blockchain.all_blocks_in_longest_chain();
                            let v_string: Vec<String> = v.into_iter().map(|h|h.to_string()).collect();
                            respond_json!(req, v_string);
                        }
                        "/blockchain/longest-chain-tx" => {
                            let result = {
                                let blockchain = blockchain.lock().unwrap();
                                let chain = blockchain.all_blocks_in_longest_chain();
                                let mut result: Vec<Vec<String>> = Vec::new();
                                
                                // Process each block
                                for block_hash in chain {
                                    if let Some(block) = blockchain.get_block(&block_hash) {
                                        // Format transaction hashes with hex encoding
                                        let block_txs: Vec<String> = block.content.data
                                            .iter()
                                            .map(|tx| hex::encode(tx.hash().as_ref()))  // Use hex::encode instead of format!
                                            .collect();
                                        
                                        result.push(block_txs);
                                    }
                                }
                                
                                drop(blockchain);
                                result
                            };
                            
                            respond_json!(req, result);
                        }
                        "/blockchain/longest-chain-tx-count" => {
                            // unimplemented!()
                            respond_result!(req, false, "unimplemented!");
                        }
                        _ => {
                            let content_type =
                                "Content-Type: application/json".parse::<Header>().unwrap();
                            let payload = ApiResponse {
                                success: false,
                                message: "endpoint not found".to_string(),
                            };
                            let resp = Response::from_string(
                                serde_json::to_string_pretty(&payload).unwrap(),
                            )
                            .with_header(content_type)
                            .with_status_code(404);
                            req.respond(resp).unwrap();
                        }
                    }
                });
            }
        });
        info!("API server listening at {}", &addr);
    }
}
