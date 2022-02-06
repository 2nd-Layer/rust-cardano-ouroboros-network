/**
© 2020 - 2022 PERLUR Group

Re-licensed under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::chainsync::{ChainSync, Mode},
};

mod common;
mod sqlite;

async fn chainsync() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await?;
    connection.handshake(cfg.magic).await?;

    let mut chainsync = ChainSync {
        mode: Mode::Sync,
        network_magic: cfg.magic,
        store: Some(Box::new(sqlite::SQLiteBlockStore::new(&cfg.db)?)),
        ..Default::default()
    };

    chainsync.run(&mut connection).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    chainsync().await.unwrap();
}
