/**
© 2020 - 2022 PERLUR Group

Re-licensed under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/

use cardano_ouroboros_network::{
    mux::Connection,
    protocols::handshake::Handshake,
    protocols::blockfetch::BlockFetch,
};

use std::sync::Arc;

use pallas::ledger::alonzo::{
    BlockWrapper,
    Fragment,
    crypto::hash_block_header,
};

use blake2b_simd::Params;

use oura::{
    sources::MagicArg,
    utils::{Utils, WithUtils},
    mapper::EventWriter,
    mapper::Config,
    mapper::ChainWellKnownInfo,
    pipelining::SinkProvider,
    pipelining::new_inter_stage_channel,
};

use log::debug;

mod common;

async fn blockfetch() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = match Connection::tcp_connect(&cfg.host).await {
        Ok(connection) => connection,
        Err(_) => return Err("Could not connect.".to_string().into()),
    };
    Handshake::builder()
        .client()
        .node_to_node()
        .network_magic(cfg.magic)
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

    let well_known = ChainWellKnownInfo::try_from_magic(*MagicArg::default()).unwrap();
    let utils = Arc::new(Utils::new(well_known, None));
    let writer = EventWriter::standalone(tx, None, config);
    let sink_handle = WithUtils::new(oura::sinks::terminal::Config { throttle_min_span_millis: Some(0)  }, utils).bootstrap(rx)?;

    let block_db = cfg.sdb.open_tree("blocks").unwrap();

    while let Some(block) = blocks.next().await? {
        let block_raw = block.clone();
        let block = BlockWrapper::decode_fragment(&block[..])?;
        let hash = hash_block_header(&block.1.header);
        //debug!("HASH: {}", hash);
        block_db.insert(&hash, &*block_raw);
        writer.crawl(&block.1).unwrap();
    }

    sink_handle.join().unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    blockfetch().await.unwrap();
}
