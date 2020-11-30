use self::chainsync_protocol::ChainSyncProtocol;
use self::handshake_protocol::HandshakeProtocol;
use self::transaction_protocol::TxSubmissionProtocol;

pub mod mux_protocol;
pub mod handshake_protocol;
pub mod transaction_protocol;
pub mod chainsync_protocol;

// Who has the ball?
//
// Client agency, we have stuff to send
// Server agency, wait for the server to send us something
#[derive(PartialEq)]
pub enum Agency {
    Client,
    Server,
    None,
}

// Common interface for a protocol
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

pub enum MiniProtocol {
    Handshake(HandshakeProtocol),
    TxSubmission(TxSubmissionProtocol),
    ChainSync(ChainSyncProtocol),
}

impl Protocol for MiniProtocol {
    fn protocol_id(&self) -> u16 {
        match self {
            MiniProtocol::Handshake(handshake_protocol) => { handshake_protocol.protocol_id() }
            MiniProtocol::TxSubmission(tx_submission_protocol) => { tx_submission_protocol.protocol_id() }
            MiniProtocol::ChainSync(chainsync_protocol) => { chainsync_protocol.protocol_id() }
        }
    }

    fn get_agency(&self) -> Agency {
        match self {
            MiniProtocol::Handshake(handshake_protocol) => { handshake_protocol.get_agency() }
            MiniProtocol::TxSubmission(tx_submission_protocol) => { tx_submission_protocol.get_agency() }
            MiniProtocol::ChainSync(chainsync_protocol) => { chainsync_protocol.get_agency() }
        }
    }

    fn get_state(&self) -> String {
        match self {
            MiniProtocol::Handshake(handshake_protocol) => { handshake_protocol.get_state() }
            MiniProtocol::TxSubmission(tx_submission_protocol) => { tx_submission_protocol.get_state() }
            MiniProtocol::ChainSync(chainsync_protocol) => { chainsync_protocol.get_state() }
        }
    }

    fn send_data(&mut self) -> Option<Vec<u8>> {
        match self {
            MiniProtocol::Handshake(handshake_protocol) => { handshake_protocol.send_data() }
            MiniProtocol::TxSubmission(tx_submission_protocol) => { tx_submission_protocol.send_data() }
            MiniProtocol::ChainSync(chainsync_protocol) => { chainsync_protocol.send_data() }
        }
    }

    fn receive_data(&mut self, data: Vec<u8>) {
        match self {
            MiniProtocol::Handshake(handshake_protocol) => { handshake_protocol.receive_data(data) }
            MiniProtocol::TxSubmission(tx_submission_protocol) => { tx_submission_protocol.receive_data(data) }
            MiniProtocol::ChainSync(chainsync_protocol) => { chainsync_protocol.receive_data(data) }
        }
    }
}