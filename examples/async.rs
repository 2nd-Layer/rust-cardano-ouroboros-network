/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use std::{
    time::{Duration},
    net::ToSocketAddrs,
};
use tokio;
use tokio::net::TcpStream;
use cardano_ouroboros_network::{
    experimental::tokio::Channel,
    protocols::handshake::{HandshakeProtocol, ConnectionType},
    protocols::chainsync::{ChainSyncProtocol, Mode},
};

mod common;
mod sqlite;

#[tokio::main]
async fn main() {
    let cfg = common::init();
    let host = cfg.host.clone();
    let port = cfg.port;
    let magic = cfg.magic;

    let saddr = (host, port)
        .to_socket_addrs().unwrap()
        .nth(0)
        .unwrap();
    let stream = tokio::time::timeout(
        Duration::from_secs(2),
        TcpStream::connect(&saddr, ),
    ).await.unwrap().unwrap();
    stream.set_nodelay(true).unwrap();
    //stream.set_keepalive_ms(Some(10_000u32)).unwrap();

    let mut channel = Channel::new(stream).await;

    // Handshake
    channel.execute(Box::new(HandshakeProtocol::new(magic, ConnectionType::Tcp))).await;
    channel.execute(Box::new(ChainSyncProtocol {
        mode: Mode::Sync,
        network_magic: magic,
        store: Some(Box::new(sqlite::SQLiteBlockStore::new(&cfg.db).unwrap())),
        ..Default::default()
    })).await;
}
