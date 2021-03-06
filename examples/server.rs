/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux::connection::Channel,
    protocols::{
        handshake,
        pingpong,
    },
};
use std::net::{TcpListener, TcpStream};
use log::{info, error};
use futures::executor::block_on;
use cardano_ouroboros_network::protocols::handshake::ConnectionType;
use cardano_ouroboros_network::mux::connection::Stream::Tcp;

mod common;

fn main() {
    let cfg = common::init();
    let listener = TcpListener::bind(format!("127.0.0.1:{}", cfg.port)).unwrap();

    for stream in listener.incoming() {
        match handle(stream.unwrap(), &cfg) {
            Ok(_) => info!("connection closed"),
            Err(e) => error!("connection failed: {}", e),
        }
    }
}

fn handle(stream: TcpStream, cfg: &common::Config) -> Result<(), String> {
    let channel = Channel::new(Tcp(stream));

    info!("new client!");
    block_on(async {
        channel.execute(handshake::HandshakeProtocol::expect(cfg.magic, ConnectionType::Tcp)).await?;
        channel.execute(pingpong::PingPongProtocol::expect(0x0100)).await?;
        Ok(())
    })
}
