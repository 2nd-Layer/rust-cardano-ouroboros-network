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

use crate::protocols::Message as MessageOps;
use crate::{
    Error,
    mux::{Connection, Channel},
    protocols::Agency,
    protocols::Protocol,
    protocols::Values,
};
use log::{
    debug,
    error,
};
use serde_cbor::{
    Value,
    Value::*,
};
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Propose,
    Confirm,
    Done,
}

#[derive(Debug, PartialEq)]
pub enum Message {
    ProposeVersions(Vec<(Version, u32)>),
    AcceptVersion(Version, u32),
    Refuse,
}

impl MessageOps for Message {
    fn from_iter(mut array: Values) -> Result<Self, Error> {
        match array.integer()? {
            0 => {
                let versions: Vec<(_, _)> = array.map()?
                    .iter()
                    .map(|(key, value)| Version::from_values(key.clone(), value.clone()))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Message::ProposeVersions(versions))
            },
            1 => {
                let version = Version::from_u16(array.integer()? as u16);
                let mut items = array.array()?;
                let magic = items.integer()? as u32;
                let _false = items.bool()?;
                // TODO: Handle this value.
                assert_eq!(_false, false);
                Ok(Message::AcceptVersion(version, magic))
            },
            2 => {
                let reason = array.array()?;
                error!("Handshake refused with reason: {:?}", reason);
                Ok(Message::Refuse)
            }
            _ => return Err("Unexpected.".to_string()),
        }
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::ProposeVersions(versions) => vec![
                Integer(0),
                Value::Map(
                    versions
                        .iter()
                        .map(|(v, m)| v.to_values(*m).unwrap())
                        .collect(),
                ),
            ],
            Message::AcceptVersion(version, magic) => vec![
                Value::Integer(1),
                Value::Integer(version.to_u16().into()),
                Array(vec![Value::Integer((*magic).into()), Bool(false)]),
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
}

pub struct HandshakeBuilder {
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
        match self {
            &Version::N2N(v) => match v {
                1..=3 => Ok((Value::Integer(v), Value::Integer(magic.into()))),
                4..=7 => Ok((
                    Value::Integer(v),
                    Value::Array(vec![Value::Integer(magic as i128), Value::Bool(false)]),
                )),
                _ => Err("Unsupported version.".to_string()),
            },
            &Version::C2N(v) => match v {
                1..=9 => Ok((Value::Integer(0x8000 ^ v), Value::Integer(magic.into()))),
                _ => Err("Unsupported version.".to_string()),
            },
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
                }
                .into_iter();
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

pub fn builder() -> HandshakeBuilder {
    HandshakeBuilder {
        versions: vec![Version::N2N(6), Version::N2N(7)],
        magic: 0,
    }
}

impl HandshakeBuilder {
    pub fn network_magic(&mut self, magic: u32) -> &mut Self {
        self.magic = magic;
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

    fn build<'a>(&self, connection: &'a mut Connection, role: Agency) -> Result<Handshake<'a>, Error> {
        Ok(Handshake {
            channel: connection.channel(match role {
                Agency::Client => 0x0000,
                Agency::Server => 0x8000,
                _ => panic!(),
            }),
            role,
            versions: self.versions.clone(),
            network_magic: self.magic,
            state: State::Propose,
            version: None,
        })
    }

    pub fn client<'a>(&self, connection: &'a mut Connection) -> Result<Handshake<'a>, Error> {
        self.build(connection, Agency::Client)
    }

    pub fn server<'a>(&self, connection: &'a mut Connection) -> Result<Handshake<'a>, Error> {
        self.build(connection, Agency::Server)
    }
}

pub struct Handshake<'a> {
    channel: Channel<'a>,
    role: Agency,
    versions: Vec<Version>,
    network_magic: u32,
    state: State,
    version: Option<Version>,
}

impl Handshake<'_> {
    pub async fn negotiate(&mut self) -> Result<(Version, u32), Error> {
        self.execute().await?;
        self.version
            .as_ref()
            .map(|v| (v.clone(), self.network_magic))
            .ok_or("Handshake failed.".to_string())
    }
}

impl<'a> Protocol<'a> for Handshake<'a> {
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
                Ok(Message::ProposeVersions(
                    self.versions.iter().map(|v| (v.clone(), self.network_magic)).collect(),
                ))
            }
            State::Confirm => {
                self.state = State::Done;
                let version = self
                    .versions
                    .last()
                    .ok_or("No versions available.".to_string())?;
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
            State::Confirm => match message {
                Message::AcceptVersion(version, magic) => {
                    self.state = State::Done;
                    self.version = self
                        .versions
                        .iter()
                        .filter(|v| **v == version)
                        .next()
                        .cloned();
                    assert_eq!(magic, self.network_magic);
                }
                Message::Refuse => {
                    self.state = State::Done;
                }
                _ => return Err("Unexpected message.".to_string()),
            },
            State::Done => panic!("unexpected recv"),
        }
        Ok(())
    }

    fn channel<'b>(&'b mut self) -> &mut Channel<'a>
    where
        'a: 'b
    {
        &mut self.channel
    }
}

#[cfg(test)]
mod tests {
    use serde_cbor::to_vec;
    use std::assert_eq;
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
    fn message_cbor_works() {
        let messages = [
            Message::ProposeVersions(
                (1..7).map(|n| (Version::N2N(n), 0x12345678)).collect(),
            ),
            Message::ProposeVersions(
                (1..9).map(|n| (Version::C2N(n), 0x12345678)).collect(),
            ),
            Message::AcceptVersion(
                Version::N2N(7),
                0x87564321,
            ),
        ];
        for message in messages {
            assert_eq!(
                Message::from_iter(Values::from_vec(&message.to_values())),
                Ok(message),
            );
        }
    }

    #[tokio::test]
    async fn handshake_client_works() {
        env_logger::builder().is_test(true).try_init().ok();
        let (mut connection, mut endpoint) = Connection::test_unix_pair().unwrap();
        let mut channel = endpoint.channel(0x8000);

        let magic = 0xdddddddd;
        tokio::join!(
            async {
                debug!("................");
                let mut client = builder()
                    .node_to_node()
                    .network_magic(magic)
                    .client(&mut connection)
                    .unwrap();
                assert_eq!(client.state, State::Propose);
                let result = client.negotiate().await.unwrap();
                assert_eq!(client.state, State::Done);
                assert_eq!(result, (Version::N2N(7), 0xdddddddd));
            },
            async {
                let request = channel.recv().await.unwrap();
                assert_eq!(request, propose(magic));
                channel.send(&confirm(magic)).await.unwrap();
            },
        );
    }

    #[tokio::test]
    async fn handshake_server_works() {
        env_logger::builder().is_test(true).try_init().ok();
        let (mut connection, mut endpoint) = Connection::test_unix_pair().unwrap();
        let mut channel = endpoint.channel(0x0000);

        let magic = 0xdddddddd;
        tokio::join!(
            async {
                let mut server = builder()
                    .node_to_node()
                    .network_magic(magic)
                    .server(&mut connection)
                    .unwrap();
                assert_eq!(server.state, State::Propose);
                let result = server.negotiate().await.unwrap();
                assert_eq!(server.state, State::Done);
                assert_eq!(result, (Version::N2N(7), magic));
            },
            async {
                channel.send(&propose(magic)).await.unwrap();
                let response = channel.recv().await.unwrap();
                assert_eq!(response, confirm(magic));
            },
        );
    }
}
