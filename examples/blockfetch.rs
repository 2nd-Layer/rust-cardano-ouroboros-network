//
// © 2020 - 2022 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::{handshake, blockfetch},
};

mod common;

async fn blockfetch() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = match Connection::tcp_connect(&cfg.host).await {
        Ok(connection) => connection,
        Err(_) => return Err("Could not connect.".to_string().into()),
    };
    handshake::builder()
        .node_to_node()
        .network_magic(cfg.magic)
        .client(&mut connection)?
        .negotiate()
        .await?;

    let mut blockfetch = blockfetch::builder()
        .first(
            26249860,
            hex::decode("915386f44ad3a7fccee949c9d3fe43f5a20459c7401f990e1cc7d52c10be1fd6")?,
        )
        .last(
            26250057,
            hex::decode("5fec758c8aaff4a7683c27b075dc3984d8d982839cc56470a682d1411c9f8198")?,
        )
        .client(&mut connection)?;
    let mut blocks = blockfetch.run().await?;

    while let Some(block) = blocks.next().await? {
        cfg.handle_block(&block)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    blockfetch().await.unwrap();
}
