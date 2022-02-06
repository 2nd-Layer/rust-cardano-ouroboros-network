/**
© 2020 - 2022 PERLUR Group

Re-licensed under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/

use cardano_ouroboros_network::{
    mux::Connection,
    BlockHeader,
    protocols::chainsync::{ChainSync, Mode, Listener},
};
use log::info;

mod common;

struct Handler {}

impl Listener for Handler {
    fn handle_tip(&mut self, msg_roll_forward: &BlockHeader) {
        info!("Tip reached: {:?}!", msg_roll_forward);
    }
}

async fn tip() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = common::init();

    let mut connection = Connection::tcp_connect(&cfg.host).await?;
    connection.handshake(cfg.magic).await?;

    let mut chainsync = ChainSync {
        mode: Mode::SendTip,
        network_magic: cfg.magic,
        notify: Some(Box::new(Handler {})),
        ..Default::default()
    };

    chainsync.run(&mut connection).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    tip().await.unwrap();
}
