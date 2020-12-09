use cardano_ouroboros_network::{
    mux,
    protocols::{
        chainsync::{ChainSyncProtocol, Mode},
        transaction::TxSubmissionProtocol,
    },
};
use futures::{
    executor::block_on,
    try_join,
};

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let channel = mux::tcp::connect(&cfg.host, cfg.port, cfg.magic).await.unwrap();
        try_join!(
            channel.execute(TxSubmissionProtocol::default()),
            channel.execute({
                let mut chainsync = ChainSyncProtocol {
                    mode: Mode::Sync,
                    network_magic: cfg.magic,
                    ..Default::default()
                };
                chainsync.init_database(&cfg.db).expect("Database error!");
                chainsync
            }),
        ).unwrap();
    });
}
