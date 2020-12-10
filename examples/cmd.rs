/**
© 2020 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use cardano_ouroboros_network::{
    mux,
    protocols::{
        handshake::HandshakeProtocol,
        chainsync::{ChainSyncProtocol, Mode},
        transaction::TxSubmissionProtocol,
    },
};
use futures::{
    executor::block_on,
    try_join,
};
use std::{
    time::Duration,
    thread::sleep,
    io,
    io::{Write, stdout},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    env,
};

mod common;
mod sqlite;

pub enum Cmd {
    Ping,
    Sync,
    SendTip,
}

fn ping_json_success<W: Write>(out: &mut W, connect_duration: Duration, total_duration: Duration, host: &String, port: u16) {
    write!(out, "{{\n\
        \x20\"status\": \"ok\",\n\
        \x20\"host\": \"{}\",\n\
        \x20\"port\": {},\n\
        \x20\"connectDurationMs\": {},\n\
        \x20\"durationMs\": {}\n\
    }}", host, port, connect_duration.as_millis(), total_duration.as_millis()).unwrap();
}

async fn connect(host: &str, port: u16) -> io::Result<mux::tcp::Channel> {
    let saddr = (host, port).to_socket_addrs()?.nth(0).unwrap();
    let stream = TcpStream::connect(&saddr)?;

    Ok(mux::tcp::Channel::new(stream).await)
}

pub fn start<W: Write>(out: &mut W, cmd: Cmd, db: &std::path::PathBuf, host: &String, port: u16, network_magic: u32, pooltool_api_key: &String, cardano_node_path: &std::path::PathBuf, pool_name: &String, pool_id: &String) {
    block_on(async {
        // continually retry connection
        loop {
            let channel = match connect(host, port).await {
                Ok(channel) => { channel }
                Err(_) => {
                    sleep(Duration::from_secs(5));
                    continue;
                }
            };
            let connect_duration = channel.duration();
            let handshake = HandshakeProtocol {
                network_magic,
                ..Default::default()
            };
            if channel.execute(handshake).await.is_err() {
                sleep(Duration::from_secs(5));
                continue;
            };
            let duration = channel.duration();
            match cmd {
                Cmd::Ping => {
                    ping_json_success(out, connect_duration, duration, host, port);
                }
                Cmd::Sync => {
                    try_join!(
                        channel.execute(TxSubmissionProtocol::default()),
                        channel.execute({ChainSyncProtocol {
                            mode: Mode::Sync,
                            network_magic,
                            store: Some(Box::new(sqlite::SQLiteBlockStore::new(&db).unwrap())),
                            ..Default::default()
                        }}),
                    ).unwrap();
                }
                Cmd::SendTip => {
                    try_join!(
                        channel.execute(TxSubmissionProtocol::default()),
                        channel.execute(ChainSyncProtocol {
                            mode: Mode::SendTip,
                            network_magic,
                            pooltool_api_key: pooltool_api_key.clone(),
                            cardano_node_path: cardano_node_path.clone(),
                            pool_name: pool_name.clone(),
                            pool_id: pool_id.clone(),
                            ..Default::default()
                        }),
                    ).unwrap();
                }
            }
            return;
        }
    })
}

fn main() {
    let mut args = env::args();
    let cfg = common::init();

    args.next();
    let cmd = match args.next().as_ref().map(String::as_str) {
        Some("ping") => Cmd::Ping,
        Some("sync") => Cmd::Sync,
        Some("tip") => Cmd::SendTip,
        _ => {
            println!("Usage: examples/cmd ping|sync|tip");
            return;
        }
    };

    start(&mut stdout(), cmd, &cfg.db, &cfg.host, cfg.port, cfg.magic, &String::new(), &PathBuf::new(), &String::new(), &String::new());
}
