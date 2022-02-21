//
// © 2022 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

use crate::mux::Channel;
#[cfg(not(test))]
use crate::mux::Connection;
use crate::Message as MessageOps;
use crate::{
    Agency,
    Error,
    Protocol,
};
use serde_cbor::Value;

type Point = (u64, Vec<u8>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Idle,
    Busy,
    Streaming,
    Done,
}

#[derive(Debug, PartialEq)]
pub enum Message {
    RequestRange(Point, Point),
    ClientDone,
    StartBatch,
    NoBlocks,
    Block(Vec<u8>),
    BatchDone,
}

impl MessageOps for Message {
    fn from_values(array: Vec<Value>) -> Result<Self, Error> {
        let mut values = array.iter();
        //debug!("Parsing message: {:?}", values);
        let message = match values
            .next()
            .ok_or("Unexpected end of message.".to_string())?
        {
            //Value::Integer(0) => Message::RequestRange(),
            Value::Integer(1) => Message::ClientDone,
            Value::Integer(2) => Message::StartBatch,
            Value::Integer(3) => Message::NoBlocks,
            Value::Integer(4) => {
                match values
                    .next()
                    .ok_or("Unexpected End of message.".to_string())?
                {
                    Value::Bytes(bytes) => Message::Block(bytes.to_vec()),
                    _ => panic!("Extra data: {:?}", values.collect::<Vec<_>>()),
                }
            }
            Value::Integer(5) => Message::BatchDone,
            _ => panic!(),
        };
        match values.next() {
            Some(Value::Null) => Ok(message),
            Some(data) => Err(format!("data={:?}", data)),
            None => Ok(message),
        }
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::RequestRange(first, last) => vec![
                Value::Integer(0),
                vec![Value::Integer(first.0.into()), first.1.clone().into()].into(),
                vec![Value::Integer(last.0.into()), last.1.clone().into()].into(),
            ]
            .into(),
            _ => panic!(),
        }
    }

    fn info(&self) -> String {
        match self {
            Message::Block(bytes) => format!("Message::Block(... {} bytes ...)", bytes.len()),
            message => format!("{:?}", message),
        }
    }
}

#[derive(Default)]
pub struct Builder {
    first: Option<Point>,
    last: Option<Point>,
}

impl Builder {
    pub fn first(&mut self, slot: u64, hash: Vec<u8>) -> &mut Self {
        self.first = Some((slot, hash));
        self
    }
    pub fn last(&mut self, slot: u64, hash: Vec<u8>) -> &mut Self {
        self.last = Some((slot, hash));
        self
    }
    pub fn build<'a>(
        &mut self,
        #[cfg(not(test))] connection: &'a mut Connection,
    ) -> Result<BlockFetch<'a>, Error> {
        Ok(BlockFetch {
            #[cfg(not(test))]
            channel: Some(connection.channel(0x0003)),
            #[cfg(test)]
            channel: None,
            config: Config {
                first: self.first.as_ref().ok_or("First point required.")?.clone(),
                last: self.last.as_ref().ok_or("Last point required.")?.clone(),
            },
            state: State::Idle,
            result: Vec::new(),
            running: false,
            done: false,
        })
    }
}

pub struct Config {
    first: Point,
    last: Point,
}

pub struct BlockFetch<'a> {
    channel: Option<Channel<'a>>,
    config: Config,
    state: State,
    result: Vec<Box<[u8]>>,
    running: bool,
    done: bool,
}

impl<'a> BlockFetch<'a> {
    pub fn builder() -> Builder {
        Default::default()
    }

    pub async fn run<'b>(&'b mut self) -> Result<BlockStream<'a, 'b>, Error>
    where
        'a: 'b,
    {
        // Start the protocol and prefetch first block into `self.result`.
        //self.running = true;
        // TODO: Do something with the Option trick.
        let mut channel = self
            .channel
            .take()
            .ok_or("Channel not available.".to_string())?;
        channel.execute(self).await?;
        self.channel = Some(channel);
        Ok(BlockStream { blockfetch: self })
    }
}

pub struct BlockStream<'a, 'b> {
    blockfetch: &'b mut BlockFetch<'a>,
}

impl BlockStream<'_, '_> {
    pub async fn next(&mut self) -> Result<Option<Box<[u8]>>, Error> {
        if self.blockfetch.result.is_empty() {
            match self.blockfetch.state() {
                State::Streaming => {
                    self.blockfetch.running = true;
                    //self.blockfetch.channel.execute(self.blockfetch).await?
                }
                State::Idle => return Ok(None),
                _ => panic!("Unexpected state."),
            }
        }
        if self.blockfetch.result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.blockfetch.result.remove(0)))
        }
    }
}

impl Protocol for BlockFetch<'_> {
    type State = State;
    type Message = Message;

    fn protocol_id(&self) -> u16 {
        0x0003
    }

    fn role(&self) -> Agency {
        Agency::Client
    }

    fn agency(&self) -> Agency {
        match self.running {
            true => match self.state {
                State::Idle => Agency::Client,
                State::Busy => Agency::Server,
                State::Streaming => Agency::Server,
                State::Done => Agency::None,
            },
            false => Agency::None,
        }
    }

    fn state(&self) -> Self::State {
        self.state
    }

    fn send(&mut self) -> Result<Message, Error> {
        debug_assert!(self.running);
        match self.done {
            false => match self.state {
                State::Idle => {
                    self.state = State::Busy;
                    Ok(Message::RequestRange(
                        self.config.first.clone(),
                        self.config.last.clone(),
                    ))
                }
                other => Err(format!("Unexpected state: {:?}", other)),
            },
            true => match self.state {
                State::Idle => {
                    self.state = State::Done;
                    Ok(Message::ClientDone)
                }
                other => Err(format!("Unexpected state: {:?}", other)),
            },
        }
    }

    fn recv(&mut self, message: Message) -> Result<(), Error> {
        // `self.running` may be false in case of pipelining.
        Ok(self.state = match (self.state, message) {
            (State::Busy, Message::NoBlocks) => State::Idle,
            (State::Busy, Message::StartBatch) => State::Streaming,
            (State::Streaming, Message::Block(bytes)) => {
                self.running = false;
                self.result.push(bytes.into_boxed_slice());
                State::Streaming
            }
            (State::Streaming, Message::BatchDone) => {
                self.running = false;
                State::Idle
            }
            (state, message) => panic!("Unexpected message {:?} in state {:?}.", message, state),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static MOCK_DATA: &'static [(u64, &[u8], &[u8])] = &[
        (42, b"mock-hash-1", b"mock-block-1"),
        (43, b"mock-hash-2", b"mock-block-2"),
        (44, b"mock-hash-3", b"mock-block-3"),
    ];

    #[test]
    fn client_accepts_blocks() {
        let &(first_slot, first_hash, _) = MOCK_DATA.first().unwrap();
        let &(last_slot, last_hash, _) = MOCK_DATA.last().unwrap();
        let mut client = BlockFetch::builder()
            .first(first_slot, first_hash.to_vec())
            .last(last_slot, last_hash.to_vec())
            .build()
            .unwrap();
        assert_eq!(client.state, State::Idle);
        // Start the exchange.
        client.running = true;
        let message = client.send().unwrap();
        assert!(client.running);
        assert_eq!(
            message,
            Message::RequestRange(
                (first_slot, first_hash.to_vec()),
                (last_slot, last_hash.to_vec()),
            ),
        );
        assert_eq!(client.state, State::Busy);
        assert!(client.result.is_empty());
        client.recv(Message::StartBatch).unwrap();
        assert!(client.running);
        assert_eq!(client.state, State::Streaming);
        assert!(client.result.is_empty());
        // Accept blocks one by one.
        for (_, _, block) in MOCK_DATA {
            client.recv(Message::Block(block.to_vec())).unwrap();
            assert!(!client.running);
            assert_eq!(client.state, State::Streaming);
            assert_eq!(client.result.remove(0), Box::from(*block));
            assert!(client.result.is_empty());
            client.running = true;
        }
        // Accept blocks as bulk.
        for (_, _, block) in MOCK_DATA {
            client.recv(Message::Block(block.to_vec())).unwrap();
            assert!(!client.running);
            assert_eq!(client.state, State::Streaming);
            client.running = true;
        }
        for (_, _, block) in MOCK_DATA {
            assert_eq!(client.result.remove(0), Box::from(*block));
        }
        assert!(client.result.is_empty());
        // Stop streaming.
        client.recv(Message::BatchDone).unwrap();
        assert!(!client.running);
        assert_eq!(client.state, State::Idle);
        assert!(client.result.is_empty());
        // Close the channel.
        client.running = true;
        client.done = true;
        let message = client.send().unwrap();
        assert!(client.running);
        assert_eq!(message, Message::ClientDone);
        assert_eq!(client.state, State::Done);
        assert!(client.result.is_empty());
    }

    #[test]
    fn client_accepts_no_blocks() {
        let &(first_slot, first_hash, _) = MOCK_DATA.first().unwrap();
        let &(last_slot, last_hash, _) = MOCK_DATA.last().unwrap();
        let mut client = BlockFetch::builder()
            .first(first_slot, first_hash.to_vec())
            .last(last_slot, last_hash.to_vec())
            .build()
            .unwrap();
        assert_eq!(client.state, State::Idle);
        // Start the exchange.
        client.running = true;
        let message = client.send().unwrap();
        assert!(client.running);
        assert_eq!(
            message,
            Message::RequestRange(
                (first_slot, first_hash.to_vec()),
                (last_slot, last_hash.to_vec()),
            ),
        );
        assert_eq!(client.state, State::Busy);
        assert!(client.result.is_empty());
        client.recv(Message::NoBlocks).unwrap();
        assert!(client.running);
        assert_eq!(client.state, State::Idle);
        assert!(client.result.is_empty());
        assert!(client.result.is_empty());
        // Close the channel.
        client.running = true;
        client.done = true;
        let message = client.send().unwrap();
        assert!(client.running);
        assert_eq!(message, Message::ClientDone);
        assert_eq!(client.state, State::Done);
        assert!(client.result.is_empty());
    }
}
