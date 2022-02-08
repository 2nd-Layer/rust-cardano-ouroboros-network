use crate::mux::{
    Channel,
    Connection,
};
use crate::Message as MessageOps;
/**
© 2022 PERLUR Group

Re-licenses under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/
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
    pub fn build(&mut self) -> Result<BlockFetch, Error> {
        Ok(BlockFetch {
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

pub struct BlockFetch {
    config: Config,
    state: State,
    result: Vec<Box<[u8]>>,
    running: bool,
    done: bool,
}

impl BlockFetch {
    pub fn builder() -> Builder {
        Default::default()
    }

    pub async fn run<'a>(
        &'a mut self,
        connection: &'a mut Connection,
    ) -> Result<BlockStream<'a>, Error> {
        // Start the protocol and prefetch first block into `self.result`.
        self.running = true;
        let mut channel = connection.execute(self);
        channel.execute().await?;
        Ok(BlockStream { channel })
    }
}

pub struct BlockStream<'a> {
    channel: Channel<'a, BlockFetch>,
}

impl BlockStream<'_> {
    pub async fn next(&mut self) -> Result<Option<Box<[u8]>>, Error> {
        if self.channel.protocol.result.is_empty() {
            match self.channel.protocol.state() {
                State::Streaming => {
                    self.channel.protocol.running = true;
                    self.channel.execute().await?
                }
                State::Idle => return Ok(None),
                _ => panic!("Unexpected state."),
            }
        }
        if self.channel.protocol.result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.channel.protocol.result.remove(0)))
        }
    }
}

impl Protocol for BlockFetch {
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

    #[test]
    fn client_accepts_blocks() {
        let mut client = BlockFetch::builder()
            .first(42, b"fake-hash-1".to_vec())
            .last(43, b"fake-hash-2".to_vec())
            .build()
            .unwrap();
        assert_eq!(client.state, State::Idle);
        // Start the exchange.
        client.running = true;
        let message = client.send().unwrap();
        assert!(client.running);
        assert_eq!(
            message,
            Message::RequestRange((42, b"fake-hash-1".to_vec()), (43, b"fake-hash-2".to_vec()),)
        );
        assert_eq!(client.state, State::Busy);
        assert!(client.result.is_empty());
        client.recv(Message::StartBatch).unwrap();
        assert!(client.running);
        assert_eq!(client.state, State::Streaming);
        assert!(client.result.is_empty());
        // Accept blocks one by one.
        for block in [b"fake-block-1", b"fake-block-2", b"fake-block-3"] {
            client.recv(Message::Block(block.to_vec())).unwrap();
            assert!(!client.running);
            assert_eq!(client.state, State::Streaming);
            assert_eq!(client.result.remove(0), Box::from(block.as_slice()));
            assert!(client.result.is_empty());
            client.running = true;
        }
        // Accept blocks as bulk.
        for block in [b"fake-block-1", b"fake-block-2", b"fake-block-3"] {
            client.recv(Message::Block(block.to_vec())).unwrap();
            assert!(!client.running);
            assert_eq!(client.state, State::Streaming);
            client.running = true;
        }
        for block in [b"fake-block-1", b"fake-block-2", b"fake-block-3"] {
            assert_eq!(client.result.remove(0), Box::from(block.as_slice()));
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
        let mut client = BlockFetch::builder()
            .first(42, b"fake-hash-1".to_vec())
            .last(43, b"fake-hash-2".to_vec())
            .build()
            .unwrap();
        assert_eq!(client.state, State::Idle);
        // Start the exchange.
        client.running = true;
        let message = client.send().unwrap();
        assert!(client.running);
        assert_eq!(
            message,
            Message::RequestRange((42, b"fake-hash-1".to_vec()), (43, b"fake-hash-2".to_vec()),)
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
