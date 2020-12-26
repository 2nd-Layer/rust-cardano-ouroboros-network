use cardano_ouroboros_network::{
    mux::tcp::Channel,
    protocols::pingpong::PingPongProtocol,
};
use std::net::TcpListener;
use log::info;
use futures::executor::block_on;

mod common;

fn main() {
    let cfg = common::init();
    let listener = TcpListener::bind(format!("127.0.0.1:{}", cfg.port)).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let channel = Channel::new(stream);

        info!("New client!");
        block_on(channel.serve(0x100, PingPongProtocol::new())).unwrap();
    }
}
