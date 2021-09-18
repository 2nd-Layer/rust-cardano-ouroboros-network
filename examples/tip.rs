/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux::Connection,
    BlockHeader,
    protocols::chainsync::{ChainSyncProtocol, Mode, Listener},
};
use log::info;

mod common;

struct Handler {}

impl Listener for Handler {
    fn handle_tip(&mut self, msg_roll_forward: &BlockHeader) {
        info!("Tip reached: {:?}!", msg_roll_forward);
    }
}

#[tokio::main]
async fn main() {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await.unwrap();
    connection.handshake(cfg.magic).await.unwrap();
    connection.execute(&mut ChainSyncProtocol {
        mode: Mode::SendTip,
        network_magic: cfg.magic,
        notify: Some(Box::new(Handler {})),
        ..Default::default()
    }).await.unwrap();
}
