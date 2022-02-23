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
    de::Deserializer,
    to_vec,
    Value,
};
use log::{trace, debug};
use blake2b_simd::Params;
use async_trait::async_trait;
use std::collections::BTreeMap;

#[async_trait]
pub(crate) trait Protocol<'a> {
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
                    let message = Self::Message::from_iter(Values::from_vec(&values)).unwrap();
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

    async fn execute(&mut self) -> Result<(), Error> {
        trace!("Executing on channel 0x{:04x}.", self.channel().get_index());
        while self.agency() != Agency::None {
            let agency = self.agency();
            let role = self.role();
            assert!(agency != Agency::None);
            if agency == role {
                let data = self.send_bytes().unwrap();
                self.channel().send(&data).await?;
            } else {
                let mut bytes = std::mem::replace(&mut self.channel().bytes, Vec::new());
                let new_data = self.channel().recv().await?;
                bytes.extend(new_data);
                self.channel().bytes = self
                    .receive_bytes(bytes)
                    .unwrap_or(Box::new([]))
                    .into_vec();
                if !self.channel().bytes.is_empty() {
                    trace!("Keeping {} bytes for the next frame.", self.channel().bytes.len());
                }
            }
        }
        Ok(())
    }

    fn channel<'b>(&'b mut self) -> &mut Channel<'a> where 'a: 'b;
}

pub(crate) trait Message: std::fmt::Debug + Sized {
    fn from_values(array: Vec<Value>) -> Result<Self, Error> { let _ = array; panic!() }
    fn from_iter(array: Values) -> Result<Self, Error> { Self::from_values(array.to_vec()) }
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
    pub(crate) fn from_vec(values: &'a Vec<Value>) -> Self {
        Values(values.iter())
    }

    pub(crate) fn to_vec(self) -> Vec<Value> {
        self.0.cloned().collect()
    }

    pub(crate) fn array(&mut self) -> Result<Self, Error> {
        match self.0.next() {
            Some(Value::Array(values)) => Ok(Values::from_vec(values)),
            other => Err(format!("Integer required: {:?}", other)),
        }
    }

    pub(crate) fn map(&mut self) -> Result<&BTreeMap<Value, Value>, Error> {
        match self.0.next() {
            Some(Value::Map(map)) => Ok(map),
            other => Err(format!("Integer required: {:?}", other)),
        }
    }


    pub(crate) fn integer(&mut self) -> Result<i128, Error> {
        match self.0.next() {
            Some(&Value::Integer(value)) => Ok(value),
            other => Err(format!("Integer required, found {:?}", other)),
        }
    }

    pub(crate) fn bool(&mut self) -> Result<bool, Error> {
        match self.0.next() {
            Some(&Value::Bool(value)) => Ok(value),
            other => Err(format!("Boolean required, found {:?}", other)),
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

pub(crate) fn point_to_vec(point: &Point) -> Vec<Value> {
    vec![
        Value::Integer(point.slot.into()),
        Value::Bytes(point.hash.clone()),
    ]
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

pub(crate) fn tip_to_vec(tip: &Tip) -> Vec<Value> {
    vec![
        Value::Array(vec![
            Value::Integer(tip.slot_number.into()),
            Value::Bytes(tip.hash.clone()),
        ]),
        Value::Integer(tip.block_number.into()),
    ]
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WrappedBlockHeader {
    bytes: Vec<u8>,
}

impl WrappedBlockHeader {
    fn hash(&self) -> Vec<u8> {
        Params::new()
            .hash_length(32)
            .to_state()
            .update(&*self.bytes)
            .finalize()
            .as_bytes()
            .to_vec()
    }
}

impl TryInto<WrappedBlockHeader> for Values<'_> {
    type Error = Error;

    fn try_into(self) -> Result<WrappedBlockHeader, Error> {
        let mut array = self;
        array.integer()?;
        let bytes = array.bytes()?.clone();
        array.end()?;
        Ok(WrappedBlockHeader { bytes })
    }
}

impl TryInto<BlockHeader> for WrappedBlockHeader {
    type Error = Error;

    fn try_into(self) -> Result<BlockHeader, Self::Error> {
        let hash = self.hash();
        let value = serde_cbor::from_slice(&self.bytes).map_err(|e| format!("{:?}", e))?;
        let mut outer_array = Values::from_vec(&value);
        let mut array = outer_array.array()?;
        let block_number = array.integer()? as i64;
        let slot_number = array.integer()? as i64;
        let prev_hash = array.bytes()?.to_vec();
        let node_vkey = array.bytes()?.to_vec();
        let node_vrf_vkey = array.bytes()?.to_vec();
        let mut eta_vrf = array.array()?;
        let eta_vrf_0 = eta_vrf.bytes()?.to_vec();
        let eta_vrf_1 = eta_vrf.bytes()?.to_vec();
        eta_vrf.end()?;
        let mut leader_vrf = array.array()?;
        let leader_vrf_0 = leader_vrf.bytes()?.to_vec();
        let leader_vrf_1 = leader_vrf.bytes()?.to_vec();
        leader_vrf.end()?;
        let block_size = array.integer()? as i64;
        let block_body_hash = array.bytes()?.to_vec();
        let pool_opcert = array.bytes()?.to_vec();
        let unknown_0 = array.integer()? as i64;
        let unknown_1 = array.integer()? as i64;
        let unknown_2 = array.bytes()?.to_vec();
        let protocol_major_version = array.integer()? as i64;
        let protocol_minor_version = array.integer()? as i64;
        array.end()?;
        outer_array.end()?;
        Ok(BlockHeader {
            block_number,
            slot_number,
            hash,
            prev_hash,
            node_vkey,
            node_vrf_vkey,
            eta_vrf_0,
            eta_vrf_1,
            leader_vrf_0,
            leader_vrf_1,
            block_size,
            block_body_hash,
            pool_opcert,
            unknown_0,
            unknown_1,
            unknown_2,
            protocol_major_version,
            protocol_minor_version,
        })
    }
}

impl TryFrom<BlockHeader> for WrappedBlockHeader {
    type Error = Error;

    fn try_from(header: BlockHeader) -> Result<Self, Self::Error> {
        let value = Value::Array(vec![
            Value::Array(vec![
                Value::Integer(header.block_number.into()),
                Value::Integer(header.slot_number.into()),
                Value::Bytes(header.prev_hash),
                Value::Bytes(header.node_vkey),
                Value::Bytes(header.node_vrf_vkey),
                Value::Array(vec![
                    Value::Bytes(header.eta_vrf_0),
                    Value::Bytes(header.eta_vrf_1),
                ]),
                Value::Array(vec![
                    Value::Bytes(header.leader_vrf_0),
                    Value::Bytes(header.leader_vrf_1),
                ]),
                Value::Integer(header.block_size.into()),
                Value::Bytes(header.block_body_hash),
                Value::Bytes(header.pool_opcert),
                Value::Integer(header.unknown_0.into()),
                Value::Integer(header.unknown_1.into()),
                Value::Bytes(header.unknown_2),
                Value::Integer(header.protocol_major_version.into()),
                Value::Integer(header.protocol_minor_version.into()),
            ]),
        ]);
        let bytes = to_vec(&value).map_err(|e| format!("{:?}", e))?.to_vec();
        Ok(WrappedBlockHeader { bytes })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn point_converts() {
        let point = Point {
            slot: 0x1122334455667788,
            hash: b"mock-hash".to_vec(),
        };
        assert_eq!(
            point.clone(),
            Values::from_vec(&point_to_vec(&point)).try_into().unwrap(),
        );
    }

    #[test]
    fn tip_converts() {
        let tip = Tip {
            block_number: 0x1234,
            slot_number: 0x5678,
            hash: b"mock-hash".to_vec(),
        };
        assert_eq!(
            tip.clone(),
            Values::from_vec(&tip_to_vec(&tip)).try_into().unwrap(),
        );
    }

    #[test]
    fn header_converts() {
        let mut header = BlockHeader {
            block_number: 1,
            slot_number: 2,
            hash: vec![],
            prev_hash: b"mock-prev-hash".to_vec(),
            node_vkey: b"mock-node-vkey".to_vec(),
            node_vrf_vkey: b"mock-node-vrf_vkey".to_vec(),
            eta_vrf_0: b"mock-eta-vrf-0".to_vec(),
            eta_vrf_1: b"mock-eta-vrf-1".to_vec(),
            leader_vrf_0: b"mock-leader-vrf-0".to_vec(),
            leader_vrf_1: b"mock-leader-vrf-1".to_vec(),
            block_size: 3,
            block_body_hash: b"mock-block-body-hash".to_vec(),
            pool_opcert: b"mock-pool-opcert".to_vec(),
            unknown_0: 4,
            unknown_1: 5,
            unknown_2: b"mock-unknown-2".to_vec(),
            protocol_major_version: 6,
            protocol_minor_version: 7,
        };
        // Get the hash computed first.
        let wrapped: WrappedBlockHeader = header.clone().try_into().unwrap();
        header.hash = wrapped.hash();
        let wrapped: WrappedBlockHeader = header.clone().try_into().unwrap();
        assert_eq!(header.hash, wrapped.hash());
        assert_eq!(
            header,
            wrapped.try_into().unwrap(),
        );
    }
}
