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

pub mod blockfetch;
pub mod chainsync;
pub mod handshake;
pub mod txsubmission;

use crate::{
    Error,
    mux::Channel,
    model::{Point, Tip, BlockHeader},
};
use serde_cbor::{
    de,
    de::Deserializer,
    to_vec,
    Value,
};
use log::{trace, debug};
use blake2b_simd::Params;

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

pub async fn execute<P>(channel: &mut Channel<'_>, protocol: &mut P) -> Result<(), Error>
where
    P: Protocol,
{
    trace!("Executing protocol {}.", channel.get_index());
    while protocol.agency() != Agency::None {
        let agency = protocol.agency();
        let role = protocol.role();
        assert!(agency != Agency::None);
        if agency == role {
            channel.send(&protocol.send_bytes().unwrap()).await?;
        } else {
            let mut bytes = std::mem::replace(&mut channel.bytes, Vec::new());
            let new_data = channel.recv().await?;
            bytes.extend(new_data);
            channel.bytes = protocol
                .receive_bytes(bytes)
                .unwrap_or(Box::new([]))
                .into_vec();
            if !channel.bytes.is_empty() {
                trace!("Keeping {} bytes for the next frame.", channel.bytes.len());
            }
        }
    }
    Ok(())
}

pub trait Message: std::fmt::Debug + Sized {
    fn from_values(array: Vec<Value>) -> Result<Self, Error>;
    fn to_values(&self) -> Vec<Value>;

    fn to_bytes(&self) -> Vec<u8> {
        let values = self.to_values();
        to_vec(&values).unwrap()
    }

    fn info(&self) -> String {
        format!("{:?}", self)
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

#[derive(Debug)]
pub(crate) struct Values<'a>(std::slice::Iter<'a, Value>);

impl<'a> Values<'a> {
    pub(crate) fn from_values(values: &'a Vec<Value>) -> Self {
        Values(values.iter())
    }

    pub(crate) fn array(&mut self) -> Result<Self, Error> {
        match self.0.next() {
            Some(Value::Array(values)) => Ok(Values::from_values(values)),
            other => Err(format!("Integer required: {:?}", other)),
        }
    }

    pub(crate) fn integer(&mut self) -> Result<i128, Error> {
        match self.0.next() {
            Some(&Value::Integer(value)) => Ok(value),
            other => Err(format!("Integer required, found {:?}", other)),
        }
    }

    pub(crate) fn bytes(&mut self) -> Result<&Vec<u8>, Error> {
        match self.0.next() {
            Some(Value::Bytes(vec)) => Ok(vec),
            other => Err(format!("Bytes required, found {:?}", other)),
        }
    }

    pub(crate) fn end(mut self) -> Result<(), Error> {
        match self.0.next() {
            None => Ok(()),
            other => Err(format!("End of array required, found {:?}", other)),
        }
    }
}

impl TryInto<Point> for Values<'_> {
    type Error = Error;

    fn try_into(mut self) -> Result<Point, Error> {
        let slot = self.integer()? as u64;
        let hash = self.bytes()?.clone();
        self.end()?;
        Ok(Point { slot, hash })
    }
}

impl TryInto<Tip> for Values<'_> {
    type Error = Error;

    fn try_into(mut self) -> Result<Tip, Error> {
        let mut tip_info_self = self.array()?;
        let slot_number = tip_info_self.integer()? as u64;
        let hash = tip_info_self.bytes()?.clone();
        tip_info_self.end()?;
        let block_number = self.integer()? as i64;
        self.end()?;
        Ok(Tip {
            block_number,
            slot_number,
            hash,
        })
    }
}

trait UnwrapValue {
    fn array(&self) -> Result<&Vec<Value>, Error>;
    fn integer(&self) -> Result<i128, Error>;
    fn bytes(&self) -> Result<&Vec<u8>, Error>;
}

impl UnwrapValue for Value {
    fn array(&self) -> Result<&Vec<Value>, Error> {
        match self {
            Value::Array(array) => Ok(array),
            _ => Err(format!("Integer required: {:?}", self)),
        }
    }

    fn integer(&self) -> Result<i128, Error> {
        match self {
            Value::Integer(value) => Ok(*value),
            _ => Err(format!("Integer required: {:?}", self)),
        }
    }

    fn bytes(&self) -> Result<&Vec<u8>, Error> {
        match self {
            Value::Bytes(vec) => Ok(vec),
            _ => Err(format!("Bytes required: {:?}", self)),
        }
    }
}

impl TryInto<BlockHeader> for Values<'_> {
    type Error = Error;

    fn try_into(self) -> Result<BlockHeader, Error> {
        let mut array = self;
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

        array.integer()?;
        let wrapped_block_header_bytes = array.bytes()?.clone();
        array.end()?;

        // calculate the block hash
        let hash = Params::new()
            .hash_length(32)
            .to_state()
            .update(&*wrapped_block_header_bytes)
            .finalize();
        msg_roll_forward.hash = hash.as_bytes().to_owned();

        let block_header: Value = de::from_slice(&wrapped_block_header_bytes[..]).unwrap();
        match block_header {
            Value::Array(block_header_array) => match &block_header_array[0] {
                Value::Array(block_header_array_inner) => {
                    msg_roll_forward.block_number = block_header_array_inner[0].integer()? as i64;
                    msg_roll_forward.slot_number = block_header_array_inner[1].integer()? as i64;
                    msg_roll_forward
                        .prev_hash
                        .append(&mut block_header_array_inner[2].bytes()?.clone());
                    msg_roll_forward
                        .node_vkey
                        .append(&mut block_header_array_inner[3].bytes()?.clone());
                    msg_roll_forward
                        .node_vrf_vkey
                        .append(&mut block_header_array_inner[4].bytes()?.clone());
                    match &block_header_array_inner[5] {
                        Value::Array(nonce_array) => {
                            msg_roll_forward
                                .eta_vrf_0
                                .append(&mut nonce_array[0].bytes()?.clone());
                            msg_roll_forward
                                .eta_vrf_1
                                .append(&mut nonce_array[1].bytes()?.clone());
                        }
                        _ => return Err("invalid cbor! code: 340".to_string()),
                    }
                    match &block_header_array_inner[6] {
                        Value::Array(leader_array) => {
                            msg_roll_forward
                                .leader_vrf_0
                                .append(&mut leader_array[0].bytes()?.clone());
                            msg_roll_forward
                                .leader_vrf_1
                                .append(&mut leader_array[1].bytes()?.clone());
                        }
                        _ => return Err("invalid cbor! code: 341".to_string()),
                    }
                    msg_roll_forward.block_size = block_header_array_inner[7].integer()? as i64;
                    msg_roll_forward
                        .block_body_hash
                        .append(&mut block_header_array_inner[8].bytes()?.clone());
                    msg_roll_forward
                        .pool_opcert
                        .append(&mut block_header_array_inner[9].bytes()?.clone());
                    msg_roll_forward.unknown_0 = block_header_array_inner[10].integer()? as i64;
                    msg_roll_forward.unknown_1 = block_header_array_inner[11].integer()? as i64;
                    msg_roll_forward
                        .unknown_2
                        .append(&mut block_header_array_inner[12].bytes()?.clone());
                    msg_roll_forward.protocol_major_version =
                        block_header_array_inner[13].integer()? as i64;
                    msg_roll_forward.protocol_minor_version =
                        block_header_array_inner[14].integer()? as i64;
                }
                _ => return Err("invalid cbor! code: 342".to_string()),
            },
            _ => return Err("invalid cbor! code: 343".to_string()),
        }
        Ok(msg_roll_forward)
    }
}
