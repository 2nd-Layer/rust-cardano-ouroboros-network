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
    let port = cfg.port;
    let magic = cfg.magic;

    block_on(async {
        let mut args: Vec<String> = env::args().collect();

        args.remove(0);

        /* Use configured host by default. */
        if args.len() == 0 {
            args = vec![cfg.host.clone()];
        }

        join_all(args.iter().map(|host| async move {
            info!("Pinging host {} port {} magic {}.", host, port, magic);
            let channel = mux::tcp::connect(&host, port).await.unwrap();
            info!("Connected.");
            channel.handshake(magic).await.unwrap();
            info!("Handshaked.");
        })).await;
    });
}
