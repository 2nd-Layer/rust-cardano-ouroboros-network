/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::chainsync::{ChainSyncProtocol, Mode},
};

mod common;
mod sqlite;

#[tokio::main]
async fn main() {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await.unwrap();
    connection.handshake(cfg.magic).await.unwrap();
    connection.execute(&mut ChainSyncProtocol {
        mode: Mode::Sync,
        network_magic: cfg.magic,
        store: Some(Box::new(sqlite::SQLiteBlockStore::new(&cfg.db).unwrap())),
        ..Default::default()
    }).await.unwrap();
}
