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
    protocols::chainsync::{
        ChainSync,
        Intersect,
        Reply,
    },
    protocols::handshake::Handshake,
};
use log::info;

mod common;

async fn tip() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await?;

    Handshake::builder()
        .node_to_node()
        .network_magic(cfg.magic)
        .client(&mut connection)?
        .negotiate()
        .await?;

    let mut chainsync = ChainSync::builder().client(&mut connection);
    let intersect = chainsync
        .find_intersect(vec![cfg.byron_mainnet, cfg.byron_testnet, cfg.byron_guild])
        .await?;
    match intersect {
        Intersect::Found(point, tip) => info!("= {:?}, {:?}", point, tip),
        _ => panic!(),
    };
    loop {
        match chainsync.request_next().await? {
            Reply::Forward(header, tip) => {
                info!("+ {:?}, {:?}", header, tip);
                if header.hash == tip.hash {
                    info!("Reached tip!");
                }
                chainsync.find_intersect(vec![tip.into()]).await?;
            }
            Reply::Backward(slot, tip) => info!("- {:?}, {:?}", slot, tip),
        }
    }
}

#[tokio::main]
async fn main() {
    tip().await.unwrap();
}
