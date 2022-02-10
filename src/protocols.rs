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

use crate::Error;
use serde_cbor::Value;

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
