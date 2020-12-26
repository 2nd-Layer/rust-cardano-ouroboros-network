use cardano_ouroboros_network::{
    mux::tcp::Channel,
    protocols::{
        handshake,
        pingpong,
    },
};
use std::net::{TcpListener, TcpStream};
use log::{info, error};
use futures::executor::block_on;

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
    let channel = Channel::new(stream);

    info!("new client!");
    block_on(async {
        channel.execute(handshake::HandshakeProtocol::expect(cfg.magic)).await?;
        channel.execute(pingpong::PingPongProtocol::expect(0x0100)).await?;
        Ok(())
    })
}
