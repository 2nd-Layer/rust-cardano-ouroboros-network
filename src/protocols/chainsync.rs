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

use crate::Message as MessageOps;
use crate::{
    model::Point,
    model::Tip,
    model::BlockHeader,
    mux::Channel,
    mux::Connection,
    protocols::Values,
    Agency,
    Error,
    Protocol,
    protocols::execute,
};
use serde_cbor::Value;

#[derive(Debug, Clone, Copy)]
pub enum State {
    Idle,
    Intersect,
    CanAwait,
    MustReply,
    Done,
}

#[derive(Debug)]
pub enum Message {
    RequestNext,
    AwaitReply,
    RollForward(BlockHeader, Tip),
    RollBackward(Point, Tip),
    FindIntersect(Vec<Point>),
    IntersectFound(Point, Tip),
    IntersectNotFound(Tip),
    Done,
}

impl MessageOps for Message {
    fn from_values(values: Vec<Value>) -> Result<Self, Error> {
        let mut array = Values::from_values(&values);
        let message = match array.integer()? {
            0 => Message::RequestNext,
            1 => Message::AwaitReply,
            2 => Message::RollForward(
                array.array()?.try_into()?,
                array.array()?.try_into()?,
            ),
            3 => Message::RollBackward(
                array.array()?.try_into()?,
                array.array()?.try_into()?,
            ),
            5 => Message::IntersectFound(
                array.array()?.try_into()?,
                array.array()?.try_into()?,
            ),
            6 => Message::IntersectNotFound(
                array.array()?.try_into()?,
            ),
            7 => Message::Done,
            other => return Err(format!("Unexpected message: {}.", other)),
        };
        array.end()?;
        Ok(message)
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::RequestNext => vec![Value::Integer(0)],
            Message::FindIntersect(points) => vec![
                Value::Integer(4),
                Value::Array(
                    points
                        .iter()
                        .map(|Point { slot, hash }| {
                            Value::Array(vec![
                                Value::Integer(*slot as i128),
                                Value::Bytes(hash.clone()),
                            ])
                        })
                        .collect(),
                ),
            ],
            _ => panic!(),
        }
    }

    fn info(&self) -> String {
        match self {
            Message::RollForward(header, _tip) => {
                format!("block={} slot={}", header.block_number, header.slot_number,)
            }
            other => format!("{:?}", other),
        }
    }
}

pub struct ChainSyncBuilder {}

impl ChainSyncBuilder {
    pub fn build<'a>(self, connection: &'a mut Connection) -> ChainSync<'a> {
        ChainSync {
            channel: Some(connection.channel(0x0002)),
            intersect: None,
            reply: None,
            state: State::Idle,
            query: None,
        }
    }
}

#[derive(Debug)]
pub enum Intersect {
    Found(Point, Tip),
    NotFound(Tip),
}

#[derive(Debug)]
pub enum Reply {
    Forward(BlockHeader, Tip),
    Backward(Point, Tip),
}

enum Query {
    Intersect(Vec<Point>),
    Reply,
}

pub struct ChainSync<'a> {
    channel: Option<Channel<'a>>,
    query: Option<Query>,
    intersect: Option<Intersect>,
    reply: Option<Reply>,
    state: State,
}

impl<'a> ChainSync<'a> {
    pub fn builder() -> ChainSyncBuilder {
        ChainSyncBuilder {}
    }

    pub async fn find_intersect(&mut self, points: Vec<Point>) -> Result<Intersect, Error> {
        self.query = Some(Query::Intersect(points));
        self.execute().await?;
        Ok(self.intersect.take().unwrap())
    }

    pub async fn request_next(&mut self) -> Result<Reply, Error> {
        self.query = Some(Query::Reply);
        self.execute().await?;
        Ok(self.reply.take().unwrap())
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

impl Protocol for ChainSync<'_> {
    type State = State;
    type Message = Message;

    fn protocol_id(&self) -> u16 {
        0x0002
    }

    fn role(&self) -> Agency {
        Agency::Client
    }

    fn agency(&self) -> Agency {
        if self.query.is_none() {
            return Agency::None;
        }
        return match self.state {
            State::Idle => Agency::Client,
            State::Intersect => Agency::Server,
            State::CanAwait => Agency::Server,
            State::MustReply => Agency::Server,
            State::Done => Agency::None,
        };
    }

    fn state(&self) -> Self::State {
        self.state
    }

    fn send(&mut self) -> Result<Self::Message, Error> {
        match self.state {
            State::Idle => match self.query.as_ref().unwrap() {
                Query::Intersect(points) => {
                    self.state = State::Intersect;
                    Ok(Message::FindIntersect(points.clone()))
                }
                Query::Reply => {
                    self.state = State::CanAwait;
                    Ok(Message::RequestNext)
                }
            },
            other => Err(format!("Unsupported: {:?}", other)),
        }
    }

    fn recv(&mut self, message: Message) -> Result<(), Error> {
        match message {
            Message::RequestNext => {
                self.state = State::CanAwait;
            }
            Message::AwaitReply => {
                self.state = State::MustReply;
            }
            Message::RollForward(header, tip) => {
                self.reply = Some(Reply::Forward(header, tip));
                self.query = None;
                self.state = State::Idle;
            }
            Message::RollBackward(point, tip) => {
                self.reply = Some(Reply::Backward(point, tip));
                self.query = None;
                self.state = State::Idle;
            }
            Message::IntersectFound(point, tip) => {
                self.intersect = Some(Intersect::Found(point, tip));
                self.query = None;
                self.state = State::Idle;
            }
            Message::IntersectNotFound(tip) => {
                self.intersect = Some(Intersect::NotFound(tip));
                self.query = None;
                self.state = State::Idle;
            }
            Message::Done => {
                self.state = State::Done;
            }
            other => return Err(format!("Got unexpected message: {:?}", other)),
        }
        Ok(())
    }
}
