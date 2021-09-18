/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::pingpong,
};
use futures::executor::block_on;

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let mut connection = Connection::tcp_connect("127.0.0.1:3001").await.unwrap();
        connection.handshake(cfg.magic).await.unwrap();
        connection.execute(&mut pingpong::PingPongProtocol::new(0x0100)).await.unwrap();
    });
}
