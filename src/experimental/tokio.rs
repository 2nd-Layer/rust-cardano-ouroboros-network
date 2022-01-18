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
    time::{Instant, Duration},
    sync::{Arc, Weak},
    collections::HashMap,
    net::ToSocketAddrs,
};
use tokio;
use tokio::{
    task,
    sync::{mpsc, Mutex},
    net::TcpStream,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    io::{AsyncWriteExt, AsyncReadExt},
};
use futures::Future;

type Payload = Vec<u8>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
type Subchannels = Arc<std::sync::Mutex<HashMap<u16, Sender<Payload>>>>;
type Error = String;

struct Demux {
    task: task::JoinHandle<Vec<u8>>,
}

impl Demux {
    fn new(task: task::JoinHandle<Vec<u8>>) -> Demux {
        Demux { task }
    }
}

impl Drop for Demux {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct Channel {
    start_time: Instant,
    sender: Mutex<OwnedWriteHalf>,
    receiver: Arc<Mutex<OwnedReadHalf>>,
    subchannels: Subchannels,
    demux: std::sync::Mutex<Weak<Demux>>,
}

impl Channel {
    pub fn new(stream: TcpStream) -> Channel {
        let (receiver, sender) = stream.into_split();
        Channel {
            start_time: Instant::now(),
            sender: Mutex::new(sender),
            receiver: Arc::new(Mutex::new(receiver)),
            subchannels: Default::default(),
            demux: Default::default(),
        }
    }
    pub fn duration(&self) -> Duration {
        self.start_time.elapsed()
    }
    pub fn execute<'a>(&'a mut self, protocol: &'a mut (dyn Protocol + Send))
        -> impl Future<Output=Result<(), Error>> + 'a
    {
        // Register queue before returning from function.
        let idx = protocol.protocol_id();
        let mut receiver = self.register(idx);
        // Return async block that actually executes the protocol.
        async move {
            let _demux = self.run_demux();

            loop {
                let agency = protocol.agency();
                if agency == Agency::None { break }
                let role = protocol.role();
                if agency == role {
                    self.send(idx, &protocol.send_data().unwrap()).await;
                } else {
                    protocol.receive_data(self.recv(&mut receiver).await);
                }
            }

            self.unregister(idx);
            Ok(())
        }
    }
    fn register(&mut self, idx: u16) -> Receiver<Payload> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.subchannels.lock().unwrap().insert(idx, tx);
        rx
    }
    fn unregister(&mut self, idx: u16) {
        self.subchannels.lock().unwrap().remove(&idx);
    }
    async fn send(&self, idx: u16, payload: &[u8]) {
        let mut sender = self.sender.lock().await;
        let start_time = Instant::now();
        sender.write_u32(start_time.elapsed().as_micros() as u32).await.unwrap();
        sender.write_u16(idx).await.unwrap();
        sender.write_u16(payload.len() as u16).await.unwrap();
        sender.write(&payload).await.unwrap();
    }
    async fn recv(&self, receiver: &mut Receiver<Payload>) -> Vec<u8> {
        receiver.recv().await.unwrap()
    }
    fn run_demux<'a>(&'a self) -> Arc<Demux> {
        let mut demux_lock = self.demux.lock().unwrap();
        match demux_lock.upgrade() {
            Some(demux) => demux,
            None => {
                let receiver = self.receiver.clone();
                let subchannels = self.subchannels.clone();
                let demux = Arc::new(Demux::new(task::spawn(async move {
                    let mut receiver = receiver.lock().await;
                    loop {
                        let mut header = [0u8; 8];
                        receiver.read_exact(&mut header).await.unwrap();
                        let _timestamp = NetworkEndian::read_u32(&mut header[0..4]);
                        let idx = NetworkEndian::read_u16(&mut header[4..6]) as u16 ^ 0x8000;
                        let length = NetworkEndian::read_u16(&header[6..]) as usize;
                        let mut payload = vec![0u8; length];
                        receiver.read_exact( &mut payload).await.unwrap();
                        subchannels.lock().unwrap()[&idx].send(payload).unwrap();
                    };
                })));
                *demux_lock = Arc::downgrade(&demux);
                demux
            }
        }
    }
}

pub async fn connect(host: &str, port: u16) -> Result<Channel, Error> {
    /* TODO: Consider asynchronous operations */
    let saddr = (host, port)
        .to_socket_addrs()
        .unwrap()
        .nth(0)
        .unwrap();
    let stream = tokio::time::timeout(
        Duration::from_secs(2),
        TcpStream::connect(&saddr, ),
    ).await.unwrap().unwrap();
    stream.set_nodelay(true).unwrap();
    //stream.set_keepalive_ms(Some(10_000u32)).unwrap();

    /*
     * We're currently doing blocking I/O, so enabling these helps you see where the code is blocking
     * and will throw errors instead. For now, leave these commented out and only enabled for debugging
     * purposes. Async I/O will become much more important once we're running multiple protocols in parallel.
     */
    // stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    // stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    Ok(Channel::new(stream))
}
