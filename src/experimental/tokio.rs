/**
© 2021 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use byteorder::{ByteOrder, NetworkEndian};
use crate::{
    Protocol,
    Agency,
};
use std::{
    time::{Instant},
    sync::{Arc,Mutex},
    collections::HashMap,
};
use tokio;
use tokio::{
    task,
    sync::mpsc,
    net::TcpStream,
    io::{AsyncWriteExt,AsyncReadExt},
};

type Subchannels = Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>;

pub struct Channel {
    _send_task: Option<task::JoinHandle<()>>,
    _recv_task: Option<task::JoinHandle<()>>,
    send_tx: mpsc::UnboundedSender<(u16, Vec<u8>)>,
    subchannels: Subchannels,
}

impl Channel {
    pub async fn new(stream: TcpStream) -> Channel {
        let (send_tx, mut send_rx) = mpsc::unbounded_channel::<(u16, Vec<u8>)>();
        let subchannels: Subchannels = Arc::new(Mutex::new(HashMap::new()));
        let (mut reader, mut writer) = stream.into_split();
        let subchannels_rx = subchannels.clone();

        let send_task = Some(task::spawn(async move {
            loop {
                let (id, payload) = send_rx.recv().await.unwrap();
                let start_time = Instant::now();
                writer.write_u32(start_time.elapsed().as_micros() as u32).await.unwrap();
                writer.write_u16(id).await.unwrap();
                writer.write_u16(payload.len() as u16).await.unwrap();
                writer.write(&payload).await.unwrap();
            }
        }));
        let recv_task = Some(task::spawn(async move {
            loop {
                let mut header = [0u8; 8];
                reader.read_exact(&mut header).await.unwrap();
                let _timestamp = NetworkEndian::read_u32(&mut header[0..4]);
                let idx = NetworkEndian::read_u16(&mut header[4..6]) as u16 ^ 0x8000;
                let length = NetworkEndian::read_u16(&header[6..]) as usize;
                let mut payload = vec![0u8; length];
                reader.read_exact( &mut payload).await.unwrap();
                subchannels_rx.lock().unwrap()[&idx].send(payload).unwrap();
            }
        }));
        Channel {
            _send_task: send_task,
            _recv_task: recv_task,
            send_tx,
            subchannels,
        }
    }
    pub async fn execute(&mut self, mut protocol: Box<dyn Protocol + Send>) -> Box<dyn Protocol + Send> {
        let (ptx, mut prx) = mpsc::unbounded_channel();
        let send_tx = self.send_tx.clone();
        let idx = protocol.protocol_id();
        self.subchannels.lock().unwrap().insert(idx, ptx);

        loop {
            let agency = protocol.agency();
            if agency == Agency::None { break }
            let role = protocol.role();
            if agency == role {
                send_tx.send((protocol.protocol_id(), protocol.send_data().unwrap())).unwrap();
            } else {
                protocol.receive_data(prx.recv().await.unwrap());
            }
        }
        protocol
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        // TODO: Cancel tasks
        // self.send_task.take().unwrap().join()
        // self.recv_task.take().unwrap().join()
    }
}
