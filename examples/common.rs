//
// © 2020 - 2022 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

use std::path::PathBuf;

use std::sync::Arc;

use pallas::ledger::alonzo::{
    crypto::hash_block_header,
    BlockWrapper,
    Fragment,
};

use oura::{
    mapper::ChainWellKnownInfo,
    mapper::Config as MapperConfig,
    mapper::EventWriter,
    pipelining::new_inter_stage_channel,
    pipelining::SinkProvider,
    sources::MagicArg,
    utils::{
        Utils,
        WithUtils,
    },
};

#[derive(Clone)]
pub struct Config {
    pub db: PathBuf,
    pub sdb: sled::Db,
    pub host: String,
    pub magic: u32,
    pub writer: EventWriter,
}

pub fn init() -> Config {
    Config::new()
}

impl Config {
    fn new() -> Config {
        env_logger::init();
        Config {
            db: PathBuf::from("sqlite.db"),
            sdb: sled::open(".db").unwrap(),
            host: "relays-new.cardano-mainnet.iohk.io:3001".to_string(),
            magic: 764824073,
            writer: oura_init().unwrap(),
        }
    }

    #[allow(dead_code)]
    pub fn handle_block(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let block_db = self.sdb.open_tree("blocks").unwrap();

        let block = BlockWrapper::decode_fragment(&data[..])?;
        let hash = hash_block_header(&block.1.header);
        //debug!("HASH: {}", hash);
        block_db.insert(&hash, &*data)?;
        self.writer.crawl(&block.1).unwrap();
        Ok(())
    }
}

fn oura_init() -> Result<EventWriter, Box<dyn std::error::Error>> {
    let (tx, rx) = new_inter_stage_channel(None);
    let config = MapperConfig {
        include_block_end_events: true,
        ..Default::default()
    };

    let well_known = ChainWellKnownInfo::try_from_magic(*MagicArg::default()).unwrap();
    let utils = Arc::new(Utils::new(well_known, None));
    let writer = EventWriter::standalone(tx, None, config);
    let _sink_handle = WithUtils::new(
        oura::sinks::terminal::Config {
            throttle_min_span_millis: Some(0),
        },
        utils,
    )
    .bootstrap(rx)?;
    Ok(writer)
    // sink_handle.join().unwrap();
}
