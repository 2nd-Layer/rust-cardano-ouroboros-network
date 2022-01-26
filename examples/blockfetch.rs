/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::blockfetch::BlockFetch,
};

mod common;

async fn blockfetch() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await?;
    connection.handshake(cfg.magic).await?;

    let mut blockfetch = BlockFetch::builder()
            .first(26249860, hex::decode("915386f44ad3a7fccee949c9d3fe43f5a20459c7401f990e1cc7d52c10be1fd6")?)
            .last(26250057, hex::decode("5fec758c8aaff4a7683c27b075dc3984d8d982839cc56470a682d1411c9f8198")?)
            .build()?;
    let mut blocks = blockfetch.run(&mut connection).await?;

    while let Some(block) = blocks.next().await? {
        println!("BLOCK: {:?}", block.len());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    blockfetch().await.unwrap();
}
