/**
© 2020 - 2022 PERLUR Group

Re-licensed under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/
use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub db: PathBuf,
    pub host: String,
    pub magic: u32,
}

pub fn init() -> Config {
    env_logger::init();
    Config {
        db: PathBuf::from("sqlite.db"),
        host: "relays-new.cardano-mainnet.iohk.io:3001".to_string(),
        magic: 764824073,
    }
}
