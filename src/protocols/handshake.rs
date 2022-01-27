/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

Re-licenses under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/

use log::{debug, error};
use serde_cbor::{Value, Value::*};
use crate::{Agency, Protocol, Error};
use crate::Message as MessageOps;
use crate::mux::Connection;
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Propose,
    Confirm,
    Done,
}

#[derive(Debug)]
pub enum Message {
    ProposeVersions(Vec<Version>, u32),
    AcceptVersion(Version, u32),
    Refuse,
}

impl MessageOps for Message {
    fn from_values(values: Vec<Value>) -> Result<Self, Error> {
        let mut values = values.into_iter();
        match values.next().ok_or("Message ID required.")? {
            Value::Integer(0) => {
                match values.next() {
                    Some(Value::Map(map)) => {
                        let items: Vec<(_, _)> = map.iter()
                            .map(|(key, value)| Version::from_values(key.clone(), value.clone()))
                            .collect::<Result<Vec<_>, _>>()?;
                        let magic = *match items.first().map(|(_, value)| value) {
                            Some(magic) => {
                                items.iter()
                                    .all(|(_, value)| value == magic)
                                    .then(|| ())
                                    .ok_or("Different magics not supported.")?;
                                magic
                            }
                            None => return Err("At least one version required.".to_string()),
                        };
                        let versions = items.into_iter()
                            .map(|(key, _)| key)
                            .collect();
                        Ok(Message::ProposeVersions(versions, magic))
                    }
                    _ => Err("Map of supported versions required.".to_string()),
                }
            }
            Value::Integer(1) => {
                let version = match values.next() {
                    Some(Value::Integer(version)) => Version::from_u16(u16::try_from(version).map_err(|e| e.to_string())?),
                    _ => return Err("Integer version number required.".to_string()),
                };
                match values.next() {
                    Some(Value::Array(array)) => {
                        let mut items = array.iter();
                        let magic = match items.next() {
                            Some(Value::Integer(magic)) => u32::try_from(*magic).map_err(|e| e.to_string())?,
                            _ => return Err("Integer version number required.".to_string()),
                        };
                        Ok(Message::AcceptVersion(version, magic))
                    }
                    _ => return Err("Array of extra parameters required.".to_string()),
                }

            }
            Value::Integer(2) => {
                match values.next() {
                    Some(Value::Array(reason)) => {
                        error!("Handshake refused with reason: {:?}", reason);
                    }
                    _ => return Err("Refuse reason required.".to_string()),
                };
                Ok(Message::Refuse)
            }
            _ => return Err("Unexpected.".to_string()),
        }
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::ProposeVersions(versions, magic) => vec![
                Integer(0),
                Value::Map(versions.iter()
                    .map(|v| v.to_values(*magic).unwrap())
                    .collect()),
            ],
            Message::AcceptVersion(version, magic) => vec![
                Value::Integer(1),
                Value::Integer(version.to_u16().into()),
                Array(vec![
                      Value::Integer((*magic).into()),
                      Bool(false),
                ]),
            ],
            Message::Refuse => vec![
                Value::Integer(1),
                // TODO: Get more information about RefuseReason format.
                Array(vec![
                      Value::Integer(2),
                      Value::Integer(0),
                      Value::Text("Refused.".to_string()),
                ]),
            ],
        }
    }

    fn info(&self) -> String {
        format!("{:?}", self)
    }
}

pub struct HandshakeBuilder {
    role: Agency,
    versions: Vec<Version>,
    magic: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Version {
    N2N(i128),
    // 1: initial version
    // 2: added local-query mini-protocol
    // 3:
    // 4: new queries added to local state query mini-protocol
    // 5: Allegra
    // 6: Mary
    // 7: Alonzo
    C2N(i128),
    // 1: initial version
    // 2: added local-query mini-protocol
    // 3: 
    // 4: new queries added to local state query mini-protocol
    // 5: Allegra
    // 6: Mary
    // 7: new queries added to local state query mini-protocol
    // 8: codec changed for local state query mini-protocol
    // 9: Updates for Alonzo
}

impl Version {
    fn to_values(&self, magic: u32) -> Result<(Value, Value), Error> {
        debug!("VERSION {:?}", self);
        match self {
            &Version::N2N(v) => match v {
                1..=3 => Ok((
                    Value::Integer(v),
                    Value::Integer(magic.into()),
                )),
                4..=7 => Ok((
                    Value::Integer(v),
                    Value::Array(vec![
                        Value::Integer(magic as i128),
                        Value::Bool(false),
                    ]),
                )),
                _ => Err("Unsupported version.".to_string()),
            },
            &Version::C2N(v) => match v {
                1..=9 => Ok((
                    Value::Integer(0x8000 ^ v),
                    Value::Integer(magic.into()),
                )),
                _ => Err("Unsupported version.".to_string()),
            }
        }
    }

    fn from_values(key: Value, value: Value) -> Result<(Version, u32), Error> {
        let version = match key {
            Value::Integer(version) => version as u32,
            _ => return Err("Version required.".to_string()),
        };
        match version {
            0x0001..=0x0003 => {
                let magic = match value {
                    Value::Integer(magic) => magic as u32,
                    _ => return Err("Magic required.".to_string()),
                };
                Ok((Version::N2N(version.into()), magic))
            }
            0x0004..=0x0007 => {
                let mut values = match value {
                    Value::Array(params) => params,
                    _ => return Err("Parameters required.".to_string()),
                }.into_iter();
                let magic = match values.next() {
                    Some(Value::Integer(magic)) => magic as u32,
                    _ => return Err("Magic required.".to_string()),
                };
                match values.next() {
                    Some(Value::Bool(false)) => (),
                    _ => return Err("False expected.".to_string()),
                }
                Ok((Version::N2N(version.into()), magic))
            }
            0x8001..=0x8009 => {
                let magic = match value {
                    Value::Integer(magic) => magic as u32,
                    _ => return Err("Magic required.".to_string()),
                };
                Ok((Version::C2N((0x8000 ^ version).into()), magic))
            }
            _ => return Err(format!("Unsupported version number: {}", version)),
        }
    }
}

impl Version {
    fn from_u16(value: u16) -> Self {
        match value & 0x8000 != 0 {
            false => Version::N2N(value.into()),
            true => Version::C2N((value ^ 0x8000).into()),
        }
    }

    fn to_u16(&self) -> u16 {
        match self {
            Version::N2N(version) => u16::try_from(*version).unwrap(),
            Version::C2N(version) => 0x8000 ^ u16::try_from(*version).unwrap(),
        }
    }
}

impl HandshakeBuilder {
    pub fn network_magic(&mut self, magic: u32) -> &mut Self {
        self.magic = magic;
        self
    }

    pub fn client(&mut self) -> &mut Self {
        self.role = Agency::Client;
        self
    }

    pub fn server(&mut self) -> &mut Self {
        self.role = Agency::Server;
        self
    }

    pub fn node_to_node(&mut self) -> &mut Self {
        self.versions = vec![Version::N2N(6), Version::N2N(7)];
        self
    }

    pub fn client_to_node(&mut self) -> &mut Self {
        self.versions = vec![Version::C2N(9)];
        self
    }

    pub fn build(&mut self) -> Result<Handshake, Error> {
        Ok(Handshake {
            role: self.role,
            versions: self.versions.clone(),
            network_magic: self.magic,
            state: State::Propose,
            version: None,
        })
    }
}

pub struct Handshake {
    role: Agency,
    versions: Vec<Version>,
    network_magic: u32,
    state: State,
    version: Option<Version>,
}

impl Handshake {
    pub fn builder() -> HandshakeBuilder {
        HandshakeBuilder {
            role: Agency::Client,
            versions: vec![Version::N2N(6), Version::N2N(7)],
            magic: 0,
        }
    }

    pub async fn run(&mut self, connection: &mut Connection) -> Result<(), Error> {
        connection.execute(self).execute().await?;
        Ok(())
    }
}

impl Protocol for Handshake {
    type State = State;
    type Message = Message;

    fn protocol_id(&self) -> u16 {
        let idx: u16 = 0;
        match self.role {
            Agency::Client => idx,
            Agency::Server => idx ^ 0x8000,
            _ => panic!("unknown role"),
        }
    }

    fn role(&self) -> Agency {
        self.role
    }

    fn agency(&self) -> Agency {
        return match self.state {
            State::Propose => Agency::Client,
            State::Confirm => Agency::Server,
            State::Done => Agency::None,
        };
    }

    fn state(&self) -> Self::State {
        self.state
    }

    fn send(&mut self) -> Result<Message, Error> {
        debug!("send: {:?}", self.state);
        match self.state {
            State::Propose => {
                self.state = State::Confirm;
                Ok(Message::ProposeVersions(self.versions.clone(), self.network_magic))
            }
            State::Confirm => {
                self.state = State::Done;
                let version = self.versions.last().ok_or("No versions available.".to_string())?;
                self.version = Some(version.clone());
                Ok(Message::AcceptVersion(version.clone(), self.network_magic))
            }
            State::Done => panic!("unexpected send"),
        }
    }

    fn recv(&mut self, message: Message) -> Result<(), Error> {
        debug!("recv: {:?}", self.state);
        match self.state {
            State::Propose => {
                self.state = State::Confirm;
            }
            State::Confirm => {
                match message {
                    Message::AcceptVersion(version, magic) => {
                        self.state = State::Done;
                        self.version = self.versions.iter().filter(|v| **v == version).next().cloned();
                        assert_eq!(magic, self.network_magic);
                    }
                    Message::Refuse => {
                        self.state = State::Done;
                    }
                    _ => return Err("Unexpected message.".to_string()),
                }
            }
            State::Done => panic!("unexpected recv"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;
    use serde_cbor::to_vec;
    use std::collections::BTreeMap;

    use super::*;

    fn propose(magic: u32) -> Vec<u8> {
        to_vec(&Array(vec![
            Integer(0),
            Map(vec![
                (Integer(6), Array(vec![Integer(magic.into()), Bool(false)])),
                (Integer(7), Array(vec![Integer(magic.into()), Bool(false)])),
            ]
            .into_iter()
            .collect::<BTreeMap<Value, Value>>()),
        ]))
        .unwrap()
    }

    fn confirm(magic: u32) -> Vec<u8> {
        to_vec(&Array(vec![
            Integer(1),
            Integer(7),
            Array(vec![Integer(magic.into()), Bool(false)]),
        ]))
        .unwrap()
    }

    #[test]
    fn handshake_client_works() {
        let magic = 0xdddddddd;
        let mut client = Handshake::builder()
            .client()
            .node_to_node()
            .network_magic(magic)
            .build()
            .unwrap();
        assert_eq!(client.state, State::Propose);
        let data = client.send_bytes().unwrap();
        assert_eq!(client.state, State::Confirm);
        assert_eq!(data, propose(magic));
        client.receive_bytes(confirm(magic));
        assert_eq!(client.state, State::Done);
    }

    #[test]
    fn handshake_server_works() {
        env_logger::init();
        let magic = 0xdddddddd;
        let mut server = Handshake::builder()
            .server()
            .node_to_node()
            .network_magic(magic)
            .build()
            .unwrap();
        assert_eq!(server.state, State::Propose);
        server.receive_bytes(propose(magic));
        assert_eq!(server.state, State::Confirm);
        let data = server.send_bytes().unwrap();
        assert_eq!(server.state, State::Done);
        assert_eq!(data, confirm(magic));
    }
}
