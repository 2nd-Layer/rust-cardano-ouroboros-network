/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

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
use std::path::PathBuf;

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let channel = mux::tcp::connect(&cfg.host, cfg.port).await.unwrap();
        channel.handshake(cfg.magic).await.unwrap();
        try_join!(
            channel.execute(TxSubmissionProtocol::default()),
            channel.execute(ChainSyncProtocol {
                mode: Mode::SendTip,
                network_magic: cfg.magic,
                pooltool_api_key: String::new(),
                cardano_node_path: PathBuf::new(),
                pool_name: String::new(),
                pool_id: String::new(),
                ..Default::default()
            }),
        ).unwrap();
    });
}
