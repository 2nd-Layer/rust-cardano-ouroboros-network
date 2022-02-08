/**
© 2020 - 2022 PERLUR Group

Re-licensed under MPLv2
© 2022 PERLUR Group

SPDX-License-Identifier: MPL-2.0

*/
use cardano_ouroboros_network::{
    mux::Connection,
    protocols::handshake::Handshake,
};
use log::info;
use std::env;

mod common;

/**
 * Test a handshake with the local node's unix socket
 */
#[cfg(target_family = "unix")]
async fn local(magic: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    args.next();
    let socket_path = &args.next().unwrap_or("test.sock".to_string());
    info!("UNIX socket path set to {:?} ", socket_path);

    let mut connection = Connection::unix_connect(socket_path).await?;

    Handshake::builder()
        .client()
        .client_to_node()
        .network_magic(magic)
        .build()?
        .run(&mut connection)
        .await?;

    info!("Ping UNIX socket success");
    Ok(())
}

#[cfg(target_family = "unix")]
#[tokio::main]
async fn main() {
    let cfg = common::init();

    local(cfg.magic).await.unwrap();
}

#[cfg(target_family = "windows")]
fn main() {
    println!("This example is not available for Windows.");
}
