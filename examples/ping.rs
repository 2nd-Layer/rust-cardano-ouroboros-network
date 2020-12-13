/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::mux;
use futures::{
    executor::block_on,
    future::join_all,
};
use log::info;
use std::env;

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let mut args = env::args();

        args.next();
        join_all(args.map(|arg| async {
            let host = arg;
            let port = cfg.port;
            let channel = mux::tcp::connect(&host, port).await.unwrap();
            info!("Connected.");
            channel.handshake(cfg.magic).await.unwrap();
            info!("Handshaked.");
        })).await;
    });
}
