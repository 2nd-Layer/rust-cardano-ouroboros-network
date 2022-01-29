/**
© 2020 - 2021 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

mod common;

/**
 * Test a handshake with the local node's unix socket
 */
#[cfg(target_family = "unix")]
#[tokio::main]
async fn main() {
    use cardano_ouroboros_network::mux::Connection;
    use log::info;
    use std::env;

    let cfg = common::init();
    let magic = cfg.magic;

    let mut args = env::args();
    args.next();
    let socket_path = &args.next().unwrap_or("test.sock".to_string());
    info!("UNIX socket path set to {:?} ", socket_path);

    let mut connection = Connection::unix_connect(socket_path)
        .await
        .unwrap();
    connection.handshake(magic).await.unwrap();
    info!("Ping UNIX socket success");
}

#[cfg(target_family = "windows")]
fn main() {
    println!("This example is not available for Windows.");
}
