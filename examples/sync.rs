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
    protocols::{
        chainsync,
        handshake,
    },
};

use log::info;

mod common;

async fn chainsync() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await?;

    handshake::builder()
        .node_to_node()
        .network_magic(cfg.magic)
        .client(&mut connection)?
        .negotiate()
        .await?;

    let mut chainsync = chainsync::builder().client(&mut connection);
    chainsync
        .find_intersect(vec![cfg.byron_mainnet, cfg.byron_testnet, cfg.byron_guild])
        .await?;
    loop {
        match chainsync.request_next().await? {
            chainsync::Reply::Forward(header, _tip) => {
                info!(
                    "Block header: block={} slot={}",
                    header.block_number, header.slot_number
                );
            }
            chainsync::Reply::Backward(point, _tip) => {
                info!("Roll backward: slot={}", point.slot);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    chainsync().await.unwrap();
}
