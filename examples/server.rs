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
    protocols::handshake,
};
use log::info;
use tokio::net::{
    TcpListener,
    TcpStream,
};

mod common;

async fn serve() {
    let cfg = common::init();
    let listener = TcpListener::bind("127.0.0.1:3001").await.unwrap();
    loop {
        let (socket, _addr) = listener.accept().await.unwrap();
        handle(socket, &cfg).await.unwrap();
    }
}

type Error = Box<dyn std::error::Error>;

async fn handle(stream: TcpStream, cfg: &common::Config) -> Result<(), Error> {
    info!("new client!");

    let mut connection = Connection::from_tcp_stream(stream);

    handshake::Handshake::builder()
        .node_to_node()
        .network_magic(cfg.magic)
        .server(&mut connection)?
        .negotiate()
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    serve().await;
}
