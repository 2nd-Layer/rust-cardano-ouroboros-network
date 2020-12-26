/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux,
    protocols::chainsync::{ChainSyncProtocol, Mode},
};
use futures::{
    executor::block_on,
    try_join,
};
use std::time::Duration;

mod common;
mod sqlite;

fn main() {
    let cfg = common::init();

    block_on(async {
        let channel = mux::tcp::connect(&cfg.host, cfg.port).await.unwrap();
        channel.handshake(cfg.magic).await.unwrap();
        channel.execute({ChainSyncProtocol {
            mode: Mode::Sync,
            network_magic: cfg.magic,
            store: Some(Box::new(sqlite::SQLiteBlockStore::new(&cfg.db).unwrap())),
            ..Default::default()
        }}).await.unwrap();
    });
}
