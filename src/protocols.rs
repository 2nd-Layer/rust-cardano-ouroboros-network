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
    Protocol,
    Agency,
    Error,
    mux::Channel,
    model::Point,
};
use log::trace;
use serde_cbor::Value;

pub async fn execute<P>(channel: &mut Channel<'_>, protocol: &mut P) -> Result<(), Error>
where
    P: Protocol,
{
    trace!("Executing protocol {}.", channel.get_index());
    loop {
        let agency = protocol.agency();
        if agency == Agency::None {
            break;
        }
        let role = protocol.role();
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
