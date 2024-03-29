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
    model::BlockHeader,
    model::Point,
    model::Tip,
    mux::Channel,
    mux::Connection,
    protocols::point_to_vec,
    protocols::tip_to_vec,
    protocols::Agency,
    protocols::Protocol,
    protocols::Values,
    protocols::WrappedBlockHeader,
    Error,
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

#[derive(Debug, PartialEq)]
pub(crate) enum Message {
    RequestNext,
    AwaitReply,
    RollForward(WrappedBlockHeader, Tip),
    RollBackward(Point, Tip),
    FindIntersect(Vec<Point>),
    IntersectFound(Point, Tip),
    IntersectNotFound(Tip),
    Done,
}

impl MessageOps for Message {
    fn from_iter(mut array: Values) -> Result<Self, Error> {
        let message = match array.integer()? {
            0 => Message::RequestNext,
            1 => Message::AwaitReply,
            2 => Message::RollForward(array.array()?.try_into()?, array.array()?.try_into()?),
            3 => Message::RollBackward(array.array()?.try_into()?, array.array()?.try_into()?),
            4 => Message::FindIntersect({
                let mut points = Vec::new();
                let mut items = array.array()?;
                while let Ok(item) = items.array() {
                    points.push(item.try_into()?);
                }
                items.end()?;
                points
            }),
            5 => Message::IntersectFound(array.array()?.try_into()?, array.array()?.try_into()?),
            6 => Message::IntersectNotFound(array.array()?.try_into()?),
            7 => Message::Done,
            other => return Err(format!("Unexpected message: {}.", other)),
        };
        array.end()?;
        Ok(message)
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::RequestNext => vec![Value::Integer(0)],
            Message::AwaitReply => vec![Value::Integer(1)],
            Message::RollForward(header, tip) => vec![
                Value::Integer(2),
                Value::Array(vec![Value::Integer(0), Value::Bytes(header.bytes.clone())]),
                Value::Array(tip_to_vec(tip)),
            ],
            Message::RollBackward(point, tip) => vec![
                Value::Integer(3),
                Value::Array(point_to_vec(point)),
                Value::Array(tip_to_vec(tip)),
            ],
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
            Message::IntersectFound(point, tip) => vec![
                Value::Integer(5),
                Value::Array(point_to_vec(point)),
                Value::Array(tip_to_vec(tip)),
            ],
            Message::IntersectNotFound(tip) => {
                vec![Value::Integer(6), Value::Array(tip_to_vec(tip))]
            }
            Message::Done => vec![Value::Integer(7)],
        }
    }

    fn info(&self) -> String {
        match self {
            Message::RollForward(_header, tip) => format!("Message::RollForward(..., {:?})", tip),
            other => format!("{:?}", other),
        }
    }
}

pub fn builder() -> ChainSyncBuilder {
    ChainSyncBuilder {}
}

pub struct ChainSyncBuilder {}

impl ChainSyncBuilder {
    pub fn client<'a>(self, connection: &'a mut Connection) -> ChainSync<'a> {
        ChainSync {
            channel: connection.channel(0x0002),
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
    channel: Channel<'a>,
    query: Option<Query>,
    intersect: Option<Intersect>,
    reply: Option<Reply>,
    state: State,
}

impl<'a> ChainSync<'a> {
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
}

impl<'a> Protocol<'a> for ChainSync<'a> {
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
                self.reply = Some(Reply::Forward(header.try_into()?, tip));
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

    fn channel<'b>(&'b mut self) -> &mut Channel<'a>
    where
        'a: 'b,
    {
        &mut self.channel
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_cbor_works() {
        let point = Point {
            slot: 0x1234,
            hash: b"mock-point-hash".to_vec(),
        };
        let tip = Tip {
            slot_number: 0x5678,
            hash: b"mock-tip-hash".to_vec(),
            block_number: 0xabcd,
        };
        let header = WrappedBlockHeader {
            bytes: b"mock-block-header".to_vec(),
        };
        let messages = [
            Message::RequestNext,
            Message::AwaitReply,
            Message::RollForward(header, tip.clone()),
            Message::RollBackward(point.clone(), tip.clone()),
            Message::FindIntersect(vec![point.clone(), point.clone()]),
            Message::IntersectFound(point.clone(), tip.clone()),
            Message::IntersectNotFound(tip.clone()),
            Message::Done,
        ];
        for message in messages {
            assert_eq!(
                Message::from_iter(Values::from_vec(&message.to_values())),
                Ok(message),
            );
        }
    }
}
