/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux,
    Notifier,
    protocols::chainsync::{ChainSyncProtocol, Mode},
    storage::msg_roll_forward::{Tip, MsgRollForward},
};
use futures::{
    executor::block_on,
};
use log::info;

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let channel = mux::tcp::connect(&cfg.host, cfg.port).await.unwrap();
        channel.handshake(cfg.magic).await.unwrap();
        channel.execute(ChainSyncProtocol {
            mode: Mode::SendTip,
            network_magic: cfg.magic,
            ..Default::default()
        }).await.unwrap();
    });
}
