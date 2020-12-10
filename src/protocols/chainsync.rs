/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use std::ops::Sub;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::io;

use chrono::{SecondsFormat, Utc};
use log::{debug, error, info, trace, warn};
use regex::Regex;
use serde::Serialize;
use serde_cbor::{de, ser, Value};

use crate::{
    Agency,
    Protocol,
    BlockStore,
    storage::{
        msg_roll_backward::parse_msg_roll_backward,
        msg_roll_forward::{MsgRollForward, parse_msg_roll_forward, Tip},
    },
};

#[derive(Debug)]
pub enum State {
    Idle,
    Intersect,
    CanAwait,
    MustReply,
    Done,
}

#[derive(PartialEq)]
pub enum Mode {
    Sync,
    SendTip,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PooltoolStats {
    api_key: String,
    pool_id: String,
    data: PooltoolData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PooltoolData {
    node_id: String,
    version: String,
    at: String,
    block_no: i64,
    slot_no: i64,
    block_hash: String,
    parent_hash: String,
    leader_vrf: String,
    platform: String,
}

pub struct ChainSyncProtocol {
    pub mode: Mode,
    pub last_log_time: Instant,
    pub last_insert_time: Instant,
    pub store: Option<Box<dyn BlockStore>>,
    pub network_magic: u32,
    pub pending_blocks: Vec<MsgRollForward>,
    pub state: State,
    pub result: Option<Result<String, String>>,
    pub is_intersect_found: bool,
    pub pooltool_api_key: String,
    pub cardano_node_path: PathBuf,
    pub node_version: String,
    pub last_node_version_time: Instant,
    pub pool_name: String,
    pub pool_id: String,
    pub tip_to_intersect: Option<Tip>,
}

impl Default for ChainSyncProtocol {
    fn default() -> Self {
        ChainSyncProtocol {
            mode: Mode::Sync,
            last_log_time: Instant::now().sub(Duration::from_secs(6)),
            last_insert_time: Instant::now(),
            store: None,
            network_magic: 764824073,
            pending_blocks: Vec::new(),
            state: State::Idle,
            result: None,
            is_intersect_found: false,
            pooltool_api_key: String::new(),
            cardano_node_path: PathBuf::new(),
            node_version: String::new(),
            last_node_version_time: Instant::now().sub(Duration::from_secs(7200)), // 2 hours ago
            pool_name: String::new(),
            pool_id: String::new(),
            tip_to_intersect: None,
        }
    }
}

impl ChainSyncProtocol {
    const FIVE_SECS: Duration = Duration::from_secs(5);

    fn save_block(&mut self, msg_roll_forward: MsgRollForward) -> io::Result<()> {
        self.pending_blocks.push(msg_roll_forward);

        if self.last_insert_time.elapsed() > ChainSyncProtocol::FIVE_SECS {
            match self.store.as_mut() {
                Some(store) => {
                    store.save_block(&mut self.pending_blocks, self.network_magic)?;
                }
                None => {}
            }
            self.last_insert_time = Instant::now();
        }

        Ok(())
    }

    fn msg_find_intersect(&self, chain_blocks: Vec<(i64, Vec<u8>)>) -> Vec<u8> {

        // figure out how to fix this extra clone later
        let msg: Value = Value::Array(
            vec![
                Value::Integer(4), // message_id
                // Value::Array(points),
                Value::Array(chain_blocks.iter().map(|(slot, hash)| Value::Array(vec![Value::Integer(*slot as i128), Value::Bytes(hash.clone())])).collect())
            ]
        );

        ser::to_vec_packed(&msg).unwrap()
    }

    fn msg_request_next(&self) -> Vec<u8> {
        // we just send an array containing the message_id for this one.
        ser::to_vec_packed(&Value::Array(vec![Value::Integer(0)])).unwrap()
    }
}

impl Protocol for ChainSyncProtocol {
    fn protocol_id(&self) -> u16 {
        return 0x0002u16;
    }

    fn get_agency(&self) -> Agency {
        return match self.state {
            State::Idle => { Agency::Client }
            State::Intersect => { Agency::Server }
            State::CanAwait => { Agency::Server }
            State::MustReply => { Agency::Server }
            State::Done => { Agency::None }
        };
    }

    fn get_state(&self) -> String {
        format!("{:?}", self.state)
    }

    fn send_data(&mut self) -> Option<Vec<u8>> {
        return match self.state {
            State::Idle => {
                trace!("ChainSyncProtocol::State::Idle");
                if !self.is_intersect_found {
                    let mut chain_blocks: Vec<(i64, Vec<u8>)> = vec![];
                    match self.mode {
                        Mode::Sync => {
                            match self.store.as_mut() {
                                Some(store) => {
                                    let blocks = (*store).load_blocks()?;
                                    for (i, block) in blocks.iter().enumerate() {
                                        // all powers of 2 including 0th element 0, 2, 4, 8, 16, 32
                                        if (i == 0) || ((i > 1) && (i & (i - 1) == 0)) {
                                            chain_blocks.push(block.clone());
                                        }
                                    }
                                }
                                None => {}
                            }
                        }
                        Mode::SendTip => {
                            if self.tip_to_intersect.is_some() {
                                let tip = self.tip_to_intersect.as_ref().unwrap();
                                chain_blocks.push((tip.slot_number, tip.hash.clone()));
                            }
                        }
                    }
                    // Last byron block of mainnet
                    chain_blocks.push((4492799, hex::decode("f8084c61b6a238acec985b59310b6ecec49c0ab8352249afd7268da5cff2a457").unwrap()));
                    // Last byron block of testnet
                    chain_blocks.push((1598399, hex::decode("7e16781b40ebf8b6da18f7b5e8ade855d6738095ef2f1c58c77e88b6e45997a4").unwrap()));

                    trace!("intersect");
                    let payload = self.msg_find_intersect(chain_blocks);
                    self.state = State::Intersect;
                    Some(payload)
                } else {
                    // request the next block from the server.
                    trace!("msg_request_next");
                    let payload = self.msg_request_next();
                    self.state = State::CanAwait;
                    Some(payload)
                }
            }
            State::Intersect => {
                debug!("ChainSyncProtocol::State::Intersect");
                None
            }
            State::CanAwait => {
                debug!("ChainSyncProtocol::State::CanAwait");
                None
            }
            State::MustReply => {
                debug!("ChainSyncProtocol::State::MustReply");
                None
            }
            State::Done => {
                debug!("ChainSyncProtocol::State::Done");
                None
            }
        };
    }

    fn receive_data(&mut self, data: Vec<u8>) {
        //msgRequestNext         = [0]
        //msgAwaitReply          = [1]
        //msgRollForward         = [2, wrappedHeader, tip]
        //msgRollBackward        = [3, point, tip]
        //msgFindIntersect       = [4, points]
        //msgIntersectFound      = [5, point, tip]
        //msgIntersectNotFound   = [6, tip]
        //chainSyncMsgDone       = [7]

        let cbor_value: Value = de::from_slice(&data[..]).unwrap();
        match cbor_value {
            Value::Array(cbor_array) => {
                match cbor_array[0] {
                    Value::Integer(message_id) => {
                        match message_id {
                            1 => {
                                // Server wants us to wait a bit until it gets a new block
                                self.state = State::MustReply;
                            }
                            2 => {
                                // MsgRollForward
                                let (msg_roll_forward, tip) = parse_msg_roll_forward(cbor_array);

                                trace!("block {} of {}, {:.2}% synced", msg_roll_forward.block_number, tip.block_number, (msg_roll_forward.block_number as f64 / tip.block_number as f64) * 100.0);
                                if self.last_log_time.elapsed().as_millis() > 5_000 {
                                    if self.mode == Mode::Sync {
                                        info!("block {} of {}, {:.2}% synced", msg_roll_forward.block_number, tip.block_number, (msg_roll_forward.block_number as f64 / tip.block_number as f64) * 100.0);
                                    }
                                    self.last_log_time = Instant::now()
                                }

                                match self.mode {
                                    Mode::Sync => { self.save_block(msg_roll_forward).unwrap(); }
                                    Mode::SendTip => {
                                        if msg_roll_forward.slot_number == tip.slot_number && msg_roll_forward.hash == tip.hash {
                                            if self.last_node_version_time.elapsed() > Duration::from_secs(3600) {
                                                // Our node version is outdated. Make a call to update it.
                                                let output = Command::new(&self.cardano_node_path)
                                                    .arg("--version")
                                                    .stdin(Stdio::null())
                                                    .stdout(Stdio::piped())
                                                    .output()
                                                    .expect(&*format!("Failed to execute {:?}", &self.cardano_node_path));
                                                let version_string = String::from_utf8_lossy(&output.stdout);
                                                let cap = Regex::new("cardano-node (\\d+\\.\\d+\\.\\d+) .*\ngit rev ([a-f0-9]{5}).*").unwrap().captures(&*version_string).unwrap();
                                                self.node_version = format!("{}:{}", cap.get(1).map_or("", |m| m.as_str()), cap.get(2).map_or("", |m| m.as_str()));
                                                info!("Checking cardano-node version: {}", &self.node_version);
                                                self.last_node_version_time = Instant::now();
                                            }
                                            let client = reqwest::blocking::Client::new();
                                            let pooltool_result = client.post("https://api.pooltool.io/v0/sendstats").body(
                                                serde_json::ser::to_string(
                                                    &PooltoolStats {
                                                        api_key: self.pooltool_api_key.clone(),
                                                        pool_id: self.pool_id.clone(),
                                                        data: PooltoolData {
                                                            node_id: "".to_string(),
                                                            version: self.node_version.clone(),
                                                            at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                                                            block_no: tip.block_number,
                                                            slot_no: tip.slot_number,
                                                            block_hash: hex::encode(&tip.hash),
                                                            parent_hash: hex::encode(&msg_roll_forward.prev_hash),
                                                            leader_vrf: hex::encode(&msg_roll_forward.leader_vrf_0),
                                                            platform: "cncli".to_string(),
                                                        },
                                                    }
                                                ).unwrap()
                                            ).send();

                                            match pooltool_result {
                                                Ok(response) => {
                                                    match response.text() {
                                                        Ok(text) => {
                                                            info!("Pooltool ({}, {}): ({}, {}), json: {}", &self.pool_name, &self.pool_id[..8], &tip.block_number, hex::encode(&tip.hash[..8]), text);
                                                        }
                                                        Err(error) => { error!("PoolTool error: {}", error); }
                                                    }
                                                }
                                                Err(error) => { error!("PoolTool error: {}", error); }
                                            }
                                        } else {
                                            self.tip_to_intersect = Some(tip);
                                            self.is_intersect_found = false;
                                        }
                                    }
                                }

                                self.state = State::Idle;

                                // testing only so we sync only a single block
                                // self.state = State::Done;
                                // self.result = Some(Ok(String::from("Done")))
                            }
                            3 => {
                                // MsgRollBackward
                                let slot = parse_msg_roll_backward(cbor_array);
                                warn!("rollback to slot: {}", slot);
                                self.state = State::Idle;
                            }
                            5 => {
                                debug!("MsgIntersectFound: {:?}", cbor_array);
                                self.is_intersect_found = true;
                                self.state = State::Idle;
                            }
                            6 => {
                                error!("MsgIntersectNotFound: {:?}", cbor_array);
                                self.is_intersect_found = true; // should start syncing at first byron block. will probably crash later, but oh well.
                                self.state = State::Idle;
                            }
                            7 => {
                                warn!("MsgDone: {:?}", cbor_array);
                                self.state = State::Done;
                                self.result = Some(Ok(String::from("Done")))
                            }
                            _ => {
                                error!("Got unexpected message_id: {}", message_id);
                            }
                        }
                    }
                    _ => {
                        error!("Unexpected cbor!")
                    }
                }
            }
            _ => {
                error!("Unexpected cbor!")
            }
        }
    }
}
