/**
Forked-off from https://github.com/AndrewWestberg/cncli/ on 2020-11-30
© 2020 Andrew Westberg licensed under Apache-2.0

Re-licensed under GPLv3 or LGPLv3
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/
pub mod mux;
pub mod protocols;

pub trait Protocol {
    // Each protocol has a unique hardcoded id
    fn protocol_id(&self) -> u16;

    // Tells us what agency state the protocol is in
    fn get_agency(&self) -> Agency;

    // Printable version of the state for the Protocol
    fn get_state(&self) -> String;

    // Fetch the next piece of data this protocol wants to send, or None if the client doesn't
    // have agency.
    fn send_data(&mut self) -> Option<Vec<u8>>;

    // Process data received from the remote server destined for this protocol
    fn receive_data(&mut self, data: Vec<u8>);
}

#[derive(PartialEq)]
pub enum Agency {
    // Client continues
    Client,
    // Server continues
    Server,
    // End of exchange
    None,
}
