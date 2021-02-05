/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use std::{
    time::{Duration, Instant},
    io,
    ops::Sub,
};

use log::{debug, error, info, trace, warn};
use serde_cbor::{de, ser, Value};

use crate::{
    Agency,
    Protocol,
    BlockStore,
    BlockHeader,
};

use blake2b_simd::Params;

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

#[derive(Debug)]
pub struct Tip {
    pub block_number: i64,
    pub slot_number: i64,
    pub hash: Vec<u8>,
}

pub trait Listener {
    fn handle_tip(&mut self, msg_roll_forward: &BlockHeader);
}

pub struct ChainSyncProtocol {
    pub mode: Mode,
    pub last_log_time: Instant,
    pub last_insert_time: Instant,
    pub store: Option<Box<dyn BlockStore>>,
    pub network_magic: u32,
    pub pending_blocks: Vec<BlockHeader>,
    pub state: State,
    pub result: Option<Result<String, String>>,
    pub is_intersect_found: bool,
    pub tip_to_intersect: Option<Tip>,
    pub notify: Option<Box<dyn Listener>>,
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
            tip_to_intersect: None,
            notify: None,
        }
    }
}

impl ChainSyncProtocol {
    const FIVE_SECS: Duration = Duration::from_secs(5);

    fn save_block(&mut self, msg_roll_forward: &BlockHeader, is_tip: bool) -> io::Result<()> {
        match self.store.as_mut() {
            Some(store) => {
                self.pending_blocks.push((*msg_roll_forward).clone());

                if is_tip || self.last_insert_time.elapsed() > ChainSyncProtocol::FIVE_SECS {
                    store.save_block(&mut self.pending_blocks, self.network_magic)?;
                    self.last_insert_time = Instant::now();
                }
            }
            None => {}
        }

        Ok(())
    }

    fn notify_tip(&mut self, msg_roll_forward: &BlockHeader) {
        match &mut self.notify {
            Some(listener) => listener.handle_tip(msg_roll_forward),
            None => {}
        }
    }

    fn jump_to_tip(&mut self, tip: Tip) {
        self.tip_to_intersect = Some(tip);
        self.is_intersect_found = false;
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

    fn result(&self) -> Result<String, String> {
        self.result.clone().unwrap()
    }

    fn role(&self) -> Agency {
        Agency::Client
    }

    fn agency(&self) -> Agency {
        return match self.state {
            State::Idle => { Agency::Client }
            State::Intersect => { Agency::Server }
            State::CanAwait => { Agency::Server }
            State::MustReply => { Agency::Server }
            State::Done => { Agency::None }
        };
    }

    fn state(&self) -> String {
        format!("{:?}", self.state)
    }

    fn send_data(&mut self) -> Option<Vec<u8>> {
        return match self.state {
            State::Idle => {
                trace!("ChainSyncProtocol::State::Idle");
                if !self.is_intersect_found {
                    let mut chain_blocks: Vec<(i64, Vec<u8>)> = vec![];

                    /* Classic sync: Use blocks from store if available. */
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

                    /* Tip discovery: Use discovered tip to retrieve header. */
                    if self.tip_to_intersect.is_some() {
                        let tip = self.tip_to_intersect.as_ref().unwrap();
                        chain_blocks.push((tip.slot_number, tip.hash.clone()));
                    }

                    // Last byron block of mainnet
                    chain_blocks.push((4492799, hex::decode("f8084c61b6a238acec985b59310b6ecec49c0ab8352249afd7268da5cff2a457").unwrap()));
                    // Last byron block of testnet
                    chain_blocks.push((1598399, hex::decode("7e16781b40ebf8b6da18f7b5e8ade855d6738095ef2f1c58c77e88b6e45997a4").unwrap()));
                    // Last byron block of guild
                    chain_blocks.push((359, hex::decode("baa280a8c640c186e44e2b78de82930e7524d8c7548c5c674aa280e671ce8a45").unwrap()));

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

        match de::from_slice(&data[..]) {
            Ok(cbor_value) => {
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
                                        match parse_msg_roll_forward(cbor_array) {
                                            None => { warn!("Probably a byron block. skipping...") }
                                            Some((msg_roll_forward, tip)) => {
                                                let is_tip = msg_roll_forward.slot_number == tip.slot_number && msg_roll_forward.hash == tip.hash;
                                                trace!("block {} of {}, {:.2}% synced", msg_roll_forward.block_number, tip.block_number, (msg_roll_forward.block_number as f64 / tip.block_number as f64) * 100.0);
                                                if is_tip || self.last_log_time.elapsed() > ChainSyncProtocol::FIVE_SECS {
                                                    if self.mode == Mode::Sync {
                                                        info!("block {} of {}, {:.2}% synced", msg_roll_forward.block_number, tip.block_number, (msg_roll_forward.block_number as f64 / tip.block_number as f64) * 100.0);
                                                    }
                                                    self.last_log_time = Instant::now()
                                                }

                                                /* Classic sync: Store header data. */
                                                /* TODO: error handling */
                                                let _ = self.save_block(&msg_roll_forward, is_tip);

                                                if is_tip {
                                                    /* Got complete tip header. */
                                                    self.notify_tip(&msg_roll_forward);
                                                } else {
                                                    match self.mode {
                                                        /* Next time get tip header. */
                                                        Mode::SendTip => self.jump_to_tip(tip),
                                                        _ => {}
                                                    }
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
                                        warn!("MsgIntersectNotFound: {:?}", cbor_array);
                                        self.is_intersect_found = true; // should start syncing at first byron block. We will just skip all byron blocks.
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
            Err(err) => { error!("cbor decode error!: {}, hex: {}", err, hex::encode(&data)) }
        }
    }
}

trait UnwrapValue {
    fn integer(&self) -> i128;
    fn bytes(&self) -> Vec<u8>;
}

impl UnwrapValue for Value {
    fn integer(&self) -> i128 {
        match self {
            Value::Integer(integer_value) => { *integer_value }
            _ => { panic!("not an integer!") }
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Value::Bytes(bytes_vec) => { bytes_vec.clone() }
            _ => { panic!("not a byte array!") }
        }
    }
}

pub fn parse_msg_roll_forward(cbor_array: Vec<Value>) -> Option<(BlockHeader, Tip)> {
    let mut msg_roll_forward = BlockHeader {
        block_number: 0,
        slot_number: 0,
        hash: vec![],
        prev_hash: vec![],
        node_vkey: vec![],
        node_vrf_vkey: vec![],
        eta_vrf_0: vec![],
        eta_vrf_1: vec![],
        leader_vrf_0: vec![],
        leader_vrf_1: vec![],
        block_size: 0,
        block_body_hash: vec![],
        pool_opcert: vec![],
        unknown_0: 0,
        unknown_1: 0,
        unknown_2: vec![],
        protocol_major_version: 0,
        protocol_minor_version: 0,
    };
    let mut tip = Tip {
        block_number: 0,
        slot_number: 0,
        hash: vec![],
    };

    match &cbor_array[1] {
        Value::Array(header_array) => {
            match &header_array[1] {
                Value::Bytes(wrapped_block_header_bytes) => {
                    // calculate the block hash
                    let hash = Params::new().hash_length(32).to_state().update(&*wrapped_block_header_bytes).finalize();
                    msg_roll_forward.hash = hash.as_bytes().to_owned();

                    let block_header: Value = de::from_slice(&wrapped_block_header_bytes[..]).unwrap();
                    match block_header {
                        Value::Array(block_header_array) => {
                            match &block_header_array[0] {
                                Value::Array(block_header_array_inner) => {
                                    msg_roll_forward.block_number = block_header_array_inner[0].integer() as i64;
                                    msg_roll_forward.slot_number = block_header_array_inner[1].integer() as i64;
                                    msg_roll_forward.prev_hash.append(&mut block_header_array_inner[2].bytes());
                                    msg_roll_forward.node_vkey.append(&mut block_header_array_inner[3].bytes());
                                    msg_roll_forward.node_vrf_vkey.append(&mut block_header_array_inner[4].bytes());
                                    match &block_header_array_inner[5] {
                                        Value::Array(nonce_array) => {
                                            msg_roll_forward.eta_vrf_0.append(&mut nonce_array[0].bytes());
                                            msg_roll_forward.eta_vrf_1.append(&mut nonce_array[1].bytes());
                                        }
                                        _ => {
                                            warn!("invalid cbor! code: 340");
                                            return None;
                                        }
                                    }
                                    match &block_header_array_inner[6] {
                                        Value::Array(leader_array) => {
                                            msg_roll_forward.leader_vrf_0.append(&mut leader_array[0].bytes());
                                            msg_roll_forward.leader_vrf_1.append(&mut leader_array[1].bytes());
                                        }
                                        _ => {
                                            warn!("invalid cbor! code: 341");
                                            return None;
                                        }
                                    }
                                    msg_roll_forward.block_size = block_header_array_inner[7].integer() as i64;
                                    msg_roll_forward.block_body_hash.append(&mut block_header_array_inner[8].bytes());
                                    msg_roll_forward.pool_opcert.append(&mut block_header_array_inner[9].bytes());
                                    msg_roll_forward.unknown_0 = block_header_array_inner[10].integer() as i64;
                                    msg_roll_forward.unknown_1 = block_header_array_inner[11].integer() as i64;
                                    msg_roll_forward.unknown_2.append(&mut block_header_array_inner[12].bytes());
                                    msg_roll_forward.protocol_major_version = block_header_array_inner[13].integer() as i64;
                                    msg_roll_forward.protocol_minor_version = block_header_array_inner[14].integer() as i64;
                                }
                                _ => {
                                    warn!("invalid cbor! code: 342");
                                    return None;
                                }
                            }
                        }
                        _ => {
                            warn!("invalid cbor! code: 343");
                            return None;
                        }
                    }
                }
                _ => {
                    warn!("invalid cbor! code: 344");
                    return None;
                }
            }
        }
        _ => {
            warn!("invalid cbor! code: 345");
            return None;
        }
    }

    match &cbor_array[2] {
        Value::Array(tip_array) => {
            match &tip_array[0] {
                Value::Array(tip_info_array) => {
                    tip.slot_number = tip_info_array[0].integer() as i64;
                    tip.hash.append(&mut tip_info_array[1].bytes());
                }
                _ => {
                    warn!("invalid cbor! code: 346");
                    return None;
                }
            }
            tip.block_number = tip_array[1].integer() as i64;
        }
        _ => {
            warn!("invalid cbor! code: 347");
            return None;
        }
    }

    Some((msg_roll_forward, tip))
}

pub fn parse_msg_roll_backward(cbor_array: Vec<Value>) -> i64 {
    let mut slot: i64 = 0;
    match &cbor_array[1] {
        Value::Array(block) => {
            if block.len() > 0 {
                match block[0] {
                    Value::Integer(parsed_slot) => { slot = parsed_slot as i64 }
                    _ => { error!("invalid cbor"); }
                }
            }
        }
        _ => { error!("invalid cbor"); }
    }

    slot
}
