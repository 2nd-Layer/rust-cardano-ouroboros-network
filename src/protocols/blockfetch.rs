//
// © 2022 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

use crate::mux::Channel;
use crate::mux::Connection;
use crate::protocols::Message as MessageOps;
use crate::{
    Error,
    protocols::Agency,
    protocols::Protocol,
    model::Point,
    protocols::execute,
    protocols::Values,
};
use serde_cbor::Value;

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
    fn from_values(values: Vec<Value>) -> Result<Self, Error> {
        let mut array = Values::from_values(&values);
        let message = match array.integer()? {
            0 => Message::RequestRange(
                array.array()?.try_into()?,
                array.array()?.try_into()?,
            ),
            1 => Message::ClientDone,
            2 => Message::StartBatch,
            3 => Message::NoBlocks,
            4 => Message::Block(
                array.bytes()?.to_vec(),
            ),
            5 => Message::BatchDone,
            _ => panic!(),
        };
        array.end()?;
        Ok(message)
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::RequestRange(first, last) => vec![
                Value::Integer(0),
                vec![Value::Integer(first.slot.into()), first.hash.clone().into()].into(),
                vec![Value::Integer(last.slot.into()), last.hash.clone().into()].into(),
            ],
            Message::ClientDone => vec![
                Value::Integer(1),
            ],
            Message::StartBatch => vec![
                Value::Integer(2),
            ],
            Message::NoBlocks => vec![
                Value::Integer(3),
            ],
            Message::Block(block)=> vec![
                Value::Integer(4),
                Value::Bytes(block.clone()),
            ],
            Message::BatchDone => vec![
                Value::Integer(5),
            ],
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
        self.first = Some((slot, hash.as_slice()).into());
        self
    }
    pub fn last(&mut self, slot: u64, hash: Vec<u8>) -> &mut Self {
        self.last = Some((slot, hash.as_slice()).into());
        self
    }
    pub fn build<'a>(
        &mut self,
        connection: &'a mut Connection,
    ) -> Result<BlockFetch<'a>, Error> {
        Ok(BlockFetch {
            channel: Some(connection.channel(0x0003)),
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
        self.running = true;
        self.execute().await?;
        Ok(BlockStream { blockfetch: self })
    }

    pub async fn done(mut self) -> Result<(), Error> {
        self.running = true;
        self.done = true;
        self.execute().await?;
        Ok(())
    }

    async fn execute(&mut self) -> Result<(), Error> {
        // TODO: Do something with the Option trick.
        let mut channel = self
            .channel
            .take()
            .ok_or("Channel not available.".to_string())?;
        execute(&mut channel, self).await?;
        self.channel = Some(channel);
        Ok(())
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
                    self.blockfetch.execute().await?;
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
            (State::Busy, Message::NoBlocks) => {
                self.running = false;
                State::Idle
            }
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
    use crate::mux::Connection;

    static MOCK_DATA: &'static [(u64, &[u8], &[u8])] = &[
        (42, b"mock-hash-1", b"mock-block-1"),
        (43, b"mock-hash-2", b"mock-block-2"),
        (44, b"mock-hash-3", b"mock-block-3"),
    ];

    #[test]
    fn message_cbor_works() {
        let &(first_slot, first_hash, first_block) = MOCK_DATA.first().unwrap();
        let &(last_slot, last_hash, _) = MOCK_DATA.last().unwrap();
        let messages = [
            Message::RequestRange(
                (first_slot, first_hash).into(),
                (last_slot, last_hash).into(),
            ),
            Message::ClientDone,
            Message::StartBatch,
            Message::NoBlocks,
            Message::Block(
                first_block.to_vec(),
            ),
            Message::BatchDone,
        ];
        for message in messages {
            assert_eq!(
                Message::from_values(message.to_values()),
                Ok(message),
            );
        }
    }

    #[tokio::test]
    async fn client_works() {
        env_logger::builder().is_test(true).try_init().ok();
        let (mut connection, mut endpoint) = Connection::test_unix_pair().unwrap();
        let mut channel = endpoint.channel(0x8003);

        let &(first_slot, first_hash, _) = MOCK_DATA.first().unwrap();
        let &(last_slot, last_hash, _) = MOCK_DATA.last().unwrap();
        let mut client = BlockFetch::builder()
            .first(first_slot, first_hash.to_vec())
            .last(last_slot, last_hash.to_vec())
            .build(&mut connection)
            .unwrap();
        assert_eq!(client.state, State::Idle);
        // Client collects a range of blocks.
        tokio::join!(
            async {
                let mut blocks = client.run().await.unwrap();
                for (_, _, block) in MOCK_DATA {
                    assert_eq!(
                        blocks.next().await,
                        Ok(Some(Box::from(*block))),
                    );
                }
                assert_eq!(
                    blocks.next().await,
                    Ok(None),
                );
            },
            async {
                channel.expect(&Message::RequestRange(
                    (first_slot, first_hash).into(),
                    (last_slot, last_hash).into(),
                ).to_bytes()).await;
                channel.send(&Message::StartBatch.to_bytes()).await.unwrap();
                for (_, _, block) in MOCK_DATA {
                    channel.send(&Message::Block(block.to_vec()).to_bytes()).await.unwrap();
                }
                channel.send(&Message::BatchDone.to_bytes()).await.unwrap();
            },
        );
        assert_eq!(client.state, State::Idle);
        // Client understands negative answer.
        tokio::join!(
            async {
                let mut blocks = client.run().await.unwrap();
                assert_eq!(
                    blocks.next().await,
                    Ok(None),
                );
            },
            async {
                channel.expect(&Message::RequestRange(
                    (first_slot, first_hash).into(),
                    (last_slot, last_hash).into(),
                ).to_bytes()).await;
                channel.send(&Message::NoBlocks.to_bytes()).await.unwrap();
            },
        );
        assert_eq!(client.state, State::Idle);
        // Client closes the channel.
        tokio::join!(
            async {
                client.done().await.unwrap();
            },
            async {
                channel.expect(&Message::ClientDone.to_bytes()).await;
            },
        );
    }
}
