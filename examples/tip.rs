/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    BlockHeader,
    mux,
    protocols::chainsync::{ChainSyncProtocol, Mode, Listener},
};
use futures::{
    executor::block_on,
};
use log::info;

mod common;

struct Handler {}

impl Listener for Handler {
    fn handle_tip(&mut self, msg_roll_forward: &BlockHeader) {
        info!("Tip reached: {:?}!", msg_roll_forward);
    }
}

fn main() {
    let cfg = common::init();

    block_on(async {
        let channel = mux::connection::connect(&cfg.host, cfg.port).await.unwrap();
        channel.handshake(cfg.magic).await.unwrap();
        channel.execute(ChainSyncProtocol {
            mode: Mode::SendTip,
            network_magic: cfg.magic,
            notify: Some(Box::new(Handler {})),
            ..Default::default()
        }).await.unwrap();
    });
}
