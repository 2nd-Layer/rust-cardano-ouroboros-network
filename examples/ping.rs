/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::mux;
use std::{
    env,
    time::Duration,
};
use log::{info, error};
use futures::{
    executor::block_on,
    future::join_all,
};

mod common;

async fn ping(host: &String, port: u16, magic: u32) -> Result<(Duration, Duration), String>{
    info!("Pinging host {} port {} magic {}.", host, port, magic);
    let channel = match mux::tcp::connect(&host, port).await {
        Ok(channel) => channel,
        Err(_) => { return Err("Could not connect.".to_string()) }
    };
    let connect_duration = channel.duration();
    channel.handshake(magic).await?;
    let total_duration = channel.duration();
    Ok((connect_duration, total_duration))
}

fn main() {
    let cfg = common::init();
    let port = cfg.port;
    let magic = cfg.magic;

    block_on(async {
        let mut args: Vec<String> = env::args().collect();

        args.remove(0);

        /* Use configured host by default. */
        if args.len() == 0 {
            args = vec![cfg.host.clone()];
        }

        join_all(args.iter().map(|host| async move {
            match ping(&host.clone(), port, magic).await {
                Ok((connect_duration, total_duration)) => {
                    info!("Ping {}:{} success! : connect_duration: {}, total_duration: {}", &host, port, connect_duration.as_millis(), total_duration.as_millis());
                }
                Err(error) => {
                    error!("Ping {}:{} failed! : {:?}", &host, port, error);
                }
            }
        })).await;
    });
}
