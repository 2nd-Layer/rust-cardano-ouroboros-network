use simple_logger::SimpleLogger;
use std::path::PathBuf;

pub struct Config {
    pub db: PathBuf,
    pub host: String,
    pub port: u16,
    pub magic: u32,
}

pub fn init() -> Config {
    SimpleLogger::new().init().unwrap();
    Config {
        db: PathBuf::from("sqlite.db"),
        host: "relays-new.cardano-mainnet.iohk.io".to_string(),
        port: 3001,
        magic: 764824073,
    }
}
