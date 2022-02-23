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
    protocols::handshake::Handshake,
};
use futures::future::join_all;
use log::{
    error,
    info,
};
use std::{
    env,
    time::Duration,
};

mod common;

async fn ping(host: &String, magic: u32) -> Result<(Duration, Duration), String> {
    info!("Pinging host {} magic {}.", host, magic);
    let mut connection = match Connection::tcp_connect(&host).await {
        Ok(connection) => connection,
        Err(_) => return Err("Could not connect.".to_string()),
    };
    let connect_duration = connection.duration();
    Handshake::builder()
        .node_to_node()
        .network_magic(magic)
        .client(&mut connection)?
        .negotiate()
        .await?;
    let total_duration = connection.duration();
    Ok((connect_duration, total_duration))
}

#[tokio::main]
async fn main() {
    let cfg = common::init();

    let mut args: Vec<String> = env::args().collect();

    args.remove(0);

    /* Use configured host by default. */
    if args.len() == 0 {
        args = vec![cfg.host.clone()];
    }

    join_all(args.iter().map(|host| {
        let cfg = cfg.clone();
        async move {
            match ping(&host.clone(), cfg.magic).await {
                Ok((connect_duration, total_duration)) => {
                    info!(
                        "Ping {} success! : connect_duration: {}, total_duration: {}",
                        &host,
                        connect_duration.as_millis(),
                        total_duration.as_millis()
                    );
                }
                Err(error) => {
                    error!("Ping {} failed! : {:?}", &host, error);
                }
            }
        }
    }))
    .await;
}
