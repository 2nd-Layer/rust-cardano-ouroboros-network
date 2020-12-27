/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux,
    protocols::pingpong,
};
use futures::executor::block_on;

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let channel = mux::tcp::connect("127.0.0.1", cfg.port).await.unwrap();
        channel.handshake(cfg.magic).await.unwrap();
        channel.execute(pingpong::PingPongProtocol::new(0x0100)).await.unwrap();
    });
}
