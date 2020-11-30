/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use byteorder::WriteBytesExt;
use log::{debug, error, warn};
use serde_cbor::{de, Value};

use super::{Agency, Protocol};

#[derive(Debug)]
pub enum State {
    Idle,
    TxIdsBlocking,
    TxIdsNonBlocking,
    //Txs,
    Done,
}

pub struct TxSubmissionProtocol {
    pub(crate) state: State,
    pub(crate) result: Option<Result<String, String>>,
}

impl Default for TxSubmissionProtocol {
    fn default() -> Self {
        TxSubmissionProtocol { state: State::Idle, result: None }
    }
}

impl TxSubmissionProtocol {
    fn msg_reply_tx_ids(&self) -> Vec<u8> {
        // We need to do manual cbor encoding to do the empty indefinite array for txs.
        // We always just tell the server we have no transactions to send it.
        let mut message: Vec<u8> = Vec::new();
        message.write_u8(0x82).unwrap(); // array of length 2
        message.write_u8(0x01).unwrap(); // message id for ReplyTxIds is 1
        message.write_u8(0x9f).unwrap(); // indefinite array start
        message.write_u8(0xff).unwrap(); // indefinite array end
        return message;
    }
}

impl Protocol for TxSubmissionProtocol {
    fn protocol_id(&self) -> u16 {
        return 0x0004u16;
    }

    fn get_agency(&self) -> Agency {
        return match self.state {
            State::Idle => { Agency::Server }
            State::TxIdsBlocking => {
                // Typically, this would be client agency, but we pretend it's none since
                // we just hang on to Client agency and never send any transactions.
                Agency::None
            }
            State::TxIdsNonBlocking => { Agency::Client }
            State::Done => { Agency::None }
        };
    }

    fn get_state(&self) -> String {
        format!("{:?}", self.state)
    }

    fn send_data(&mut self) -> Option<Vec<u8>> {
        return match self.state {
            State::Idle => {
                debug!("TxSubmissionProtocol::State::Idle");
                None
            }
            State::TxIdsBlocking => {
                debug!("TxSubmissionProtocol::State::TxIdsBlocking");
                // Server will wait on us forever. Just move to Done state.
                self.state = State::Done;
                None
            }
            State::TxIdsNonBlocking => {
                debug!("TxSubmissionProtocol::State::TxIdsNonBlocking");
                // Tell the server that we have no transactions to send them
                let payload = self.msg_reply_tx_ids();
                self.state = State::Idle;
                return Some(payload);
            }
            //State::Txs => { None }
            State::Done => {
                warn!("TxSubmissionProtocol::State::Done");
                self.result = Option::Some(Ok(String::from("Done")));
                return None;
            }
        };
    }

    fn receive_data(&mut self, data: Vec<u8>) {
        let cbor_value: Value = de::from_slice(&data[..]).unwrap();
        match cbor_value {
            Value::Array(cbor_array) => {
                match cbor_array[0] {
                    Value::Integer(message_id) => {
                        match message_id {
                            //msgRequestTxIds = [0, tsBlocking, txCount, txCount]
                            //msgReplyTxIds   = [1, [ *txIdAndSize] ]
                            //msgRequestTxs   = [2, tsIdList ]
                            //msgReplyTxs     = [3, tsIdList ]
                            //tsMsgDone       = [4]
                            //msgReplyKTnxBye = [5]
                            0 => {
                                debug!("TxSubmissionProtocol received MsgRequestTxIds");
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
            }
            _ => {
                error!("Unexpected cbor!")
            }
        }
    }
}