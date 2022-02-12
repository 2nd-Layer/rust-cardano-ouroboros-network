//
// © 2020 - 2022 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

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

use cardano_ouroboros_network::model::Point;

#[derive(Clone)]
pub struct Config {
    pub sdb: sled::Db,
    pub host: String,
    pub magic: u32,
    pub writer: EventWriter,
    pub byron_mainnet: Point,
    pub byron_testnet: Point,
    pub byron_guild: Point,
}

pub fn init() -> Config {
    Config::new()
}

impl Config {
    fn new() -> Config {
        env_logger::init();
        Config {
            sdb: sled::open(".db").unwrap(),
            host: "relays-new.cardano-mainnet.iohk.io:3001".to_string(),
            magic: 764824073,
            writer: oura_init().unwrap(),
            byron_mainnet: (
                4492799,
                "f8084c61b6a238acec985b59310b6ecec49c0ab8352249afd7268da5cff2a457",
            )
                .try_into()
                .unwrap(),
            byron_testnet: (
                1598399,
                "7e16781b40ebf8b6da18f7b5e8ade855d6738095ef2f1c58c77e88b6e45997a4",
            )
                .try_into()
                .unwrap(),
            byron_guild: (
                719,
                "e5400faf19e712ebc5ff5b4b44cecb2b140d1cca25a011e36a91d89e97f53e2e",
            )
                .try_into()
                .unwrap(),
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
