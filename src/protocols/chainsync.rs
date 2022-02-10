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

use serde_cbor::{
    de,
    Value,
};

use crate::Message as MessageOps;
use crate::{
    Agency,
    Error,
    Protocol,
    mux::Connection,
    mux::Channel,
    model::Point,
    model::Tip,
    protocols::Values,
};
use crate::{
    BlockHeader,
};

use blake2b_simd::Params;

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
                parse_wrapped_header(array.array()?)?,
                parse_tip(array.array()?)?,
            ),
            3 => Message::RollBackward(
                parse_point(array.array()?)?,
                parse_tip(array.array()?)?,
            ),
            5 => Message::IntersectFound(
                parse_point(array.array()?)?,
                parse_tip(array.array()?)?,
            ),
            6 => Message::IntersectNotFound(
                parse_tip(array.array()?)?,
            ),
            7 => Message::Done,
            other => return Err(format!("Unexpected message: {}.", other)),
        };
        array.end()?;
        Ok(message)
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::RequestNext => vec![
                Value::Integer(0),
            ],
            Message::FindIntersect(points) => vec![
                Value::Integer(4),
                Value::Array(
                    points
                        .iter()
                        .map(|Point {slot, hash}| {
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
            Message::RollForward(header, _tip) => format!(
                "block={} slot={}",
                header.block_number,
                header.slot_number,
            ),
            other => format!("{:?}", other),
        }
    }
}

pub struct ChainSyncBuilder {
}

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
        ChainSyncBuilder {
        }
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
        let mut channel = self.channel.take().ok_or("Channel not available.".to_string())?;
        channel.execute(self).await?;
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
            State::Idle => {
                match self.query.as_ref().unwrap() {
                    Query::Intersect(points) => {
                        self.state = State::Intersect;
                        Ok(Message::FindIntersect(points.clone()))
                    }
                    Query::Reply => {
                        self.state = State::CanAwait;
                        Ok(Message::RequestNext)
                    }
                }
            }
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

trait UnwrapValue {
    fn array(&self) -> Result<&Vec<Value>, Error>;
    fn integer(&self) -> Result<i128, Error>;
    fn bytes(&self) -> Result<&Vec<u8>, Error>;
}

impl UnwrapValue for Value {
    fn array(&self) -> Result<&Vec<Value>, Error> {
        match self {
            Value::Array(array) => Ok(array),
            _ => Err(format!("Integer required: {:?}", self)),
        }
    }

    fn integer(&self) -> Result<i128, Error> {
        match self {
            Value::Integer(value) => Ok(*value),
            _ => Err(format!("Integer required: {:?}", self)),
        }
    }

    fn bytes(&self) -> Result<&Vec<u8>, Error> {
        match self {
            Value::Bytes(vec) => Ok(vec),
            _ => Err(format!("Bytes required: {:?}", self)),
        }
    }
}
fn parse_wrapped_header(mut array: Values) -> Result<BlockHeader, Error> {
    let mut msg_roll_forward = BlockHeader {
        block_number: 0,
        slot_number: 0,
        hash: vec![],
        prev_hash: vec![],
        node_vkey: vec![],
        node_vrf_vkey: vec![],
        eta_vrf_0: vec![],
        eta_vrf_1: vec![],
        leader_vrf_0: vec![],
        leader_vrf_1: vec![],
        block_size: 0,
        block_body_hash: vec![],
        pool_opcert: vec![],
        unknown_0: 0,
        unknown_1: 0,
        unknown_2: vec![],
        protocol_major_version: 0,
        protocol_minor_version: 0,
    };

    array.integer()?;
    let wrapped_block_header_bytes = array.bytes()?.clone();
    array.end()?;

    // calculate the block hash
    let hash = Params::new()
        .hash_length(32)
        .to_state()
        .update(&*wrapped_block_header_bytes)
        .finalize();
    msg_roll_forward.hash = hash.as_bytes().to_owned();

    let block_header: Value =
        de::from_slice(&wrapped_block_header_bytes[..]).unwrap();
    match block_header {
        Value::Array(block_header_array) => match &block_header_array[0] {
            Value::Array(block_header_array_inner) => {
                msg_roll_forward.block_number =
                    block_header_array_inner[0].integer()? as i64;
                msg_roll_forward.slot_number =
                    block_header_array_inner[1].integer()? as i64;
                msg_roll_forward
                    .prev_hash
                    .append(&mut block_header_array_inner[2].bytes()?.clone());
                msg_roll_forward
                    .node_vkey
                    .append(&mut block_header_array_inner[3].bytes()?.clone());
                msg_roll_forward
                    .node_vrf_vkey
                    .append(&mut block_header_array_inner[4].bytes()?.clone());
                match &block_header_array_inner[5] {
                    Value::Array(nonce_array) => {
                        msg_roll_forward
                            .eta_vrf_0
                            .append(&mut nonce_array[0].bytes()?.clone());
                        msg_roll_forward
                            .eta_vrf_1
                            .append(&mut nonce_array[1].bytes()?.clone());
                    }
                    _ => return Err("invalid cbor! code: 340".to_string()),
                }
                match &block_header_array_inner[6] {
                    Value::Array(leader_array) => {
                        msg_roll_forward
                            .leader_vrf_0
                            .append(&mut leader_array[0].bytes()?.clone());
                        msg_roll_forward
                            .leader_vrf_1
                            .append(&mut leader_array[1].bytes()?.clone());
                    }
                    _ => return Err("invalid cbor! code: 341".to_string()),
                }
                msg_roll_forward.block_size =
                    block_header_array_inner[7].integer()? as i64;
                msg_roll_forward
                    .block_body_hash
                    .append(&mut block_header_array_inner[8].bytes()?.clone());
                msg_roll_forward
                    .pool_opcert
                    .append(&mut block_header_array_inner[9].bytes()?.clone());
                msg_roll_forward.unknown_0 =
                    block_header_array_inner[10].integer()? as i64;
                msg_roll_forward.unknown_1 =
                    block_header_array_inner[11].integer()? as i64;
                msg_roll_forward
                    .unknown_2
                    .append(&mut block_header_array_inner[12].bytes()?.clone());
                msg_roll_forward.protocol_major_version =
                    block_header_array_inner[13].integer()? as i64;
                msg_roll_forward.protocol_minor_version =
                    block_header_array_inner[14].integer()? as i64;
            }
            _ => return Err("invalid cbor! code: 342".to_string()),
        },
        _ => return Err("invalid cbor! code: 343".to_string()),
    }
    Ok(msg_roll_forward)
}

fn parse_tip(mut array: Values) -> Result<Tip, Error> {
    let mut tip_info_array = array.array()?;
    let slot_number = tip_info_array.integer()? as i64;
    let hash = tip_info_array.bytes()?.clone();
    tip_info_array.end()?;
    let block_number = array.integer()? as i64;
    array.end()?;
    Ok(Tip {
        block_number,
        slot_number,
        hash,
    })
}

fn parse_point(mut array: Values) -> Result<Point, Error> {
    let slot = array.integer()? as i64;
    let hash = array.bytes()?.clone();
    array.end()?;
    Ok(Point { slot, hash })
}
