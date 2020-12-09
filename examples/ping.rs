use cardano_ouroboros_network::mux;
use futures::{
    executor::block_on,
    future::join_all,
};
use log::info;
use std::env;

mod common;

fn main() {
    let cfg = common::init();

    block_on(async {
        let mut args = env::args();

        args.next();
        join_all(args.map(|arg| async {
            let host = arg;
            let port = cfg.port;
            let _channel = mux::tcp::connect(&host, port, cfg.magic).await;
            info!("Ping {}:{} finished.", &host, port);
        })).await;
    });
}
