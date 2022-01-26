/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use crate::{Agency, Protocol, Error};
use crate::Message as MessageOps;
use byteorder::WriteBytesExt;
use log::{debug, error};
use serde_cbor::{Value, to_vec};

#[derive(Debug, Clone, Copy)]
pub enum State {
    Idle,
    TxIdsBlocking,
    TxIdsNonBlocking,
    //Txs,
    Done,
}

#[derive(Debug)]
pub enum Message {
    Array(Vec<Value>),
    Raw(Vec<u8>),
}

impl MessageOps for Message {
    fn from_values(values: Vec<Value>) -> Result<Self, Error> {
        Ok(Message::Array(values))
    }

    fn to_values(&self) -> Vec<Value> {
        match self {
            Message::Array(values) => values.clone(),
            Message::Raw(_data) => panic!(),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Message::Raw(data) => data.clone(),
            message => {
                let value = self.to_values();
                debug!("Tx: message {:?}", message);
                to_vec(&value).unwrap()
            }
        }
    }

    fn info(&self) -> String {
        format!("{:?}", self)
    }
}

pub struct TxSubmission {
    pub(crate) state: State,
}

impl Default for TxSubmission {
    fn default() -> Self {
        TxSubmission {
            state: State::Idle
        }
    }
}

impl TxSubmission {
    fn msg_reply_tx_ids(&self) -> Message {
        // We need to do manual cbor encoding to do the empty indefinite array for txs.
        // We always just tell the server we have no transactions to send it.
        let mut message: Vec<u8> = Vec::new();
        message.write_u8(0x82).unwrap(); // array of length 2
        message.write_u8(0x01).unwrap(); // message id for ReplyTxIds is 1
        message.write_u8(0x9f).unwrap(); // indefinite array start
        message.write_u8(0xff).unwrap(); // indefinite array end
        Message::Raw(message)
    }
}

impl Protocol for TxSubmission {
    type State = State;
    type Message = Message;

    fn protocol_id(&self) -> u16 {
        0x0004
    }

    fn role(&self) -> Agency {
        Agency::Client
    }

    fn agency(&self) -> Agency {
        return match self.state {
            State::Idle => Agency::None,
            State::TxIdsBlocking => Agency::None,
            State::TxIdsNonBlocking => Agency::None,
            State::Done => Agency::None,
        };
    }

    fn state(&self) -> Self::State {
        self.state
    }

    fn send(&mut self) -> Result<Self::Message, Error> {
        return match self.state {
            State::TxIdsBlocking => {
                debug!("TxSubmission::State::TxIdsBlocking");
                // Server will wait on us forever. Just move to Done state.
                self.state = State::Done;
                Err("Unexpected.".to_string())
            }
            State::TxIdsNonBlocking => {
                debug!("TxSubmission::State::TxIdsNonBlocking");
                // Tell the server that we have no transactions to send them
                let payload = self.msg_reply_tx_ids();
                self.state = State::Idle;
                Ok(payload)
            }
            state => Err(format!("Unexpected state: {:?}", state))
        };
    }

    fn recv(&mut self, message: Self::Message) -> Result<(), Error> {
        match message {
            Message::Array(cbor_array) => match cbor_array[0] {
                Value::Integer(message_id) => {
                    match message_id {
                        //msgRequestTxIds = [0, tsBlocking, txCount, txCount]
                        //msgReplyTxIds   = [1, [ *txIdAndSize] ]
                        //msgRequestTxs   = [2, tsIdList ]
                        //msgReplyTxs     = [3, tsIdList ]
                        //tsMsgDone       = [4]
                        //msgReplyKTnxBye = [5]
                        0 => {
                            debug!("TxSubmission received MsgRequestTxIds");
                            let is_blocking = cbor_array[1] == Value::Bool(true);
                            self.state = if is_blocking {
                                State::TxIdsBlocking
                            } else {
                                State::TxIdsNonBlocking
                            }
                        }
                        _ => {
                            error!("unexpected message_id: {}", message_id);
                        }
                    }
                }
                _ => {
                    error!("Unexpected cbor!")
                }
            }
            _ => panic!(),
        }
        Ok(())
    }
}
