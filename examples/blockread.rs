/**
© 2020 - 2022 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use std::sync::Arc;

use pallas::ledger::alonzo::{
    BlockWrapper,
    Fragment,
    crypto::hash_block_header,
};

use oura::{
    sources::MagicArg,
    utils::{Utils, WithUtils},
    mapper::EventWriter,
    mapper::Config,
    mapper::ChainWellKnownInfo,
    pipelining::SinkProvider,
    pipelining::new_inter_stage_channel,
};

use log::error;

mod common;

async fn blockread() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();
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

    for record in &block_db {
        let (db_hash, block) = record.unwrap();
        let block = BlockWrapper::decode_fragment(&block[..])?;
        let hash = hash_block_header(&block.1.header);
        if db_hash == hash {
            writer.crawl(&block.1).unwrap();
        } else {
            error!("Hash doesn't match!");
        }
    }
    sink_handle.join().unwrap();
    Ok(())
}

#[tokio::main]
async fn main() {
    blockread().await.unwrap();
}
