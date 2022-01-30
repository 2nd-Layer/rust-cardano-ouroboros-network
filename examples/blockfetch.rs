/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::handshake::Handshake,
    protocols::blockfetch::BlockFetch,
};

use pallas::ledger::alonzo::{
    BlockWrapper,
    Fragment,
};

use oura::{
    mapper::EventWriter,
    mapper::Config,
    pipelining::SinkProvider,
    pipelining::new_inter_stage_channel,
};

mod common;

async fn blockfetch(host: &String, magic: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = match Connection::tcp_connect(&host).await {
        Ok(connection) => connection,
        Err(_) => return Err("Could not connect.".to_string().into()),
    };
    Handshake::builder()
        .client()
        .node_to_node()
        .network_magic(magic)
        .build()?
        .run(&mut connection).await?;

    let mut blockfetch = BlockFetch::builder()
            .first(26249860, hex::decode("915386f44ad3a7fccee949c9d3fe43f5a20459c7401f990e1cc7d52c10be1fd6")?)
            .last(26250057, hex::decode("5fec758c8aaff4a7683c27b075dc3984d8d982839cc56470a682d1411c9f8198")?)
            .build()?;
    let mut blocks = blockfetch.run(&mut connection).await?;

    let (tx, rx) = new_inter_stage_channel(None);
    let config = Config {
        include_block_end_events: true,
        ..Default::default()
    };
    let writer = EventWriter::standalone(tx, None, config);
    let sink_handle = oura::sinks::terminal::Config::default().bootstrap(rx)?;

    while let Some(block) = blocks.next().await? {
        let block = BlockWrapper::decode_fragment(&block[..])?;
        writer.crawl(&block.1).unwrap();
    }

    sink_handle.join().unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    let cfg = common::init();
    blockfetch(&cfg.host, cfg.magic).await.unwrap();
}
