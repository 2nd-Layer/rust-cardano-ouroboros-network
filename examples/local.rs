/**
© 2020 - 2021 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::mux;
use futures::executor::block_on;
use log::{error, info};
use std::env;

mod common;

/**
 * Test a handshake with the local node's unix socket
 */
#[cfg(target_family = "unix")]
fn main() {
    let cfg = common::init();
    let args: Vec<String> = env::args().collect();
    let magic = cfg.magic;

    let socket_path = &args[1];
    info!("UNIX socket path set to {} ", socket_path);

    block_on(async {
        let channel = mux::connection::connect_unix(socket_path)
            .await
            .unwrap();
        channel.handshake(magic).await.unwrap();
        info!("Ping UNIX socket success");
    });
}

#[cfg(target_family = "windows")]
fn main() {
    let cfg = common::init();

    error!("This test is not available on Windows!");
}
