//
// Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
// © 2020 Andrew Westberg licensed under Apache-2.0
//
// Re-licensed under GPLv3 or LGPLv3
// © 2020 - 2021 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

pub mod model;
pub mod mux;
pub mod protocols;

use log::debug;
use serde_cbor::{
    de::Deserializer,
    to_vec,
    Value,
};
use std::io;

//
// Error will be string for now. But please use `Result<_, dyn error::Error` if
// you want to stay compatible.
//
type Error = String;

pub trait Message: std::fmt::Debug + Sized {
    fn from_values(array: Vec<Value>) -> Result<Self, Error>;
    fn to_values(&self) -> Vec<Value>;
    fn info(&self) -> String;

    fn to_bytes(&self) -> Vec<u8> {
        let values = self.to_values();
        to_vec(&values).unwrap()
    }
}

pub trait Protocol {
    type State: std::fmt::Debug;
    type Message: Message;

    //
    // Static information
    //
    fn protocol_id(&self) -> u16;

    //
    // Runtime information
    //
    fn role(&self) -> Agency;
    fn state(&self) -> Self::State;
    fn agency(&self) -> Agency;

    //
    // Communication
    //
    fn send(&mut self) -> Result<Self::Message, Error>;
    fn recv(&mut self, message: Self::Message) -> Result<(), Error>;

    //
    // Binary data
    //

    fn send_bytes(&mut self) -> Option<Vec<u8>> {
        debug_assert_eq!(self.agency(), self.role());
        // TODO: Protocol should really return an error.
        let message = self.send().unwrap();
        let info = message.info();
        debug!("Tx: message {}", info);
        let bytes = message.to_bytes();
        debug!("State: {:?}", self.state());
        Some(bytes)
    }

    fn receive_bytes(&mut self, data: Vec<u8>) -> Option<Box<[u8]>> {
        debug_assert!(self.agency() != Agency::None);
        debug_assert!(self.agency() != self.role());
        //debug!("Received data length={}", data.len());
        debug!("receive_bytes {:?}", data.chunks(32).next());
        let mut d = Deserializer::from_slice(&data).into_iter::<Vec<Value>>();
        debug!("----");
        let mut last_offset = 0;
        while let Some(chunk) = d.next() {
            match chunk {
                Ok(values) => {
                    let message = Self::Message::from_values(values).unwrap();
                    let info = message.info();
                    self.recv(message).unwrap();
                    debug!("Rx: message {}", info);
                    debug!("State: {:?}", self.state());
                    debug!("Demux offset: {}", d.byte_offset());
                    last_offset = d.byte_offset();
                }
                Err(e) => match e.is_eof() {
                    true => {
                        return Some(Box::from(&data[last_offset..]));
                    }
                    false => panic!("Error: {:?}", e),
                },
            }
        }
        assert_eq!(d.byte_offset(), data.len());
        None
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Agency {
    // Client continues
    Client,
    // Server continues
    Server,
    // End of exchange
    None,
}

pub trait BlockStore: Send {
    fn save_block(
        &mut self,
        pending_blocks: &mut Vec<BlockHeader>,
        network_magic: u32,
    ) -> io::Result<()>;
    fn load_blocks(&mut self) -> Option<Vec<(i64, Vec<u8>)>>;
}

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub block_number: i64,
    pub slot_number: i64,
    pub hash: Vec<u8>,
    pub prev_hash: Vec<u8>,
    pub node_vkey: Vec<u8>,
    pub node_vrf_vkey: Vec<u8>,
    pub eta_vrf_0: Vec<u8>,
    pub eta_vrf_1: Vec<u8>,
    pub leader_vrf_0: Vec<u8>,
    pub leader_vrf_1: Vec<u8>,
    pub block_size: i64,
    pub block_body_hash: Vec<u8>,
    pub pool_opcert: Vec<u8>,
    pub unknown_0: i64,
    pub unknown_1: i64,
    pub unknown_2: Vec<u8>,
    pub protocol_major_version: i64,
    pub protocol_minor_version: i64,
}
