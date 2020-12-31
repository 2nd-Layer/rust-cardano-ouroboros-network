/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use crate::{
    Agency,
    Protocol,
};
use log::{trace, error};

pub struct PingPongProtocol {
    role: Agency,
    state: State,
    idx: u16,
}

#[derive(Debug, Copy, Clone)]
enum State {
    Idle,
    Busy,
    Done,
}

#[derive(Debug)]
enum MessageType {
    Ping,
    Pong,
    //Done,
}

fn transition(state: State, agency: Agency, message: MessageType) -> State {
    trace!("Transition from {:?} by {:?} message {:?}.", state, agency, message);
    match (state, agency, message) {
        (State::Idle, Agency::Client, MessageType::Ping) => State::Busy,
        //(State::Idle, Agency::Client, MessageType::Done) => State::Done,
        (State::Busy, Agency::Server, MessageType::Pong) => State::Idle,
        _ => {
            error!("protocol violation");
            State::Done
        }
    }
}

impl PingPongProtocol {
    pub fn new(idx: u16) -> Self {
        PingPongProtocol {
            role: Agency::Client,
            state: State::Idle,
            idx,
        }
    }

    pub fn expect(idx: u16) -> Self {
        PingPongProtocol {
            role: Agency::Server,
            state: State::Idle,
            idx,
        }
    }
}

impl Protocol for PingPongProtocol {
    fn protocol_id(&self) -> u16 {
        match self.role {
            Agency::Client => self.idx,
            Agency::Server => self.idx ^ 0x8000,
            Agency::None => panic!("unexpected role"),
        }
    }

    fn role(&self) -> Agency {
        self.role
    }

    fn state(&self) -> String {
        format!("{:?}", self.state)
    }

    fn agency(&self) -> Agency {
        match self.state {
            State::Idle => Agency::Client,
            State::Busy => Agency::Server,
            State::Done => Agency::None,
        }
    }

    fn result(&self) -> Result<String, String> {
        Ok("no result".to_string())
    }

    fn receive_data(&mut self, _payload: Vec<u8>) {
        /* TODO: parse the message */
        match self.state {
            State::Idle => {
                self.state = transition(self.state, self.agency(), MessageType::Ping);
                trace!("Ping received!");
            }
            State::Busy => {
                self.state = transition(self.state, self.agency(), MessageType::Pong);
                trace!("Pong received!");
            }
            State::Done => panic!("unexpected recv"),
        }
    }

    fn send_data(&mut self) -> Option<Vec<u8>> {
        /* TODO: assemble the message */
        match self.state {
            State::Idle => {
                self.state = transition(self.state, self.agency(), MessageType::Ping);
                trace!("Sending ping!");
            }
            State::Busy => {
                self.state = transition(self.state, self.agency(), MessageType::Pong);
                trace!("Sending pong!");
            }
            State::Done => panic!("unexpected send"),
        }

        Some(vec![])
    }
}
