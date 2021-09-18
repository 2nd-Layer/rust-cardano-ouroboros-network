/**
© 2021 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use byteorder::{ByteOrder, NetworkEndian};
use crate::{
    Protocol,
    Agency,
    protocols::handshake::HandshakeProtocol,
};
use std::{
    io,
    time::{Instant, Duration},
    sync::{Arc, Weak},
    collections::HashMap,
};
use tokio;
use tokio::{
    task,
    sync::{mpsc, Mutex},
    net::TcpStream,
    net::ToSocketAddrs,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, AsyncReadExt},
};
use log::trace;
use std::future::Future;

#[cfg(target_family = "unix")]
use tokio::net::UnixStream;

type Payload = Vec<u8>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
type Channels = Arc<std::sync::Mutex<HashMap<u16, Sender<Payload>>>>;
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

pub struct Connection {
    start_time: Instant,
    sender: Mutex<Box<dyn AsyncWrite + Unpin + Send>>,
    receiver: Arc<Mutex<Box<dyn AsyncRead + Unpin + Send>>>,
    channels: Channels,
    demux: std::sync::Mutex<Weak<Demux>>,
}

impl Connection {
    fn new(receiver: Box<dyn AsyncRead + Unpin + Send>, sender: Box<dyn AsyncWrite + Unpin + Send>) -> Self {
        Connection {
            start_time: Instant::now(),
            sender: Mutex::new(sender),
            receiver: Arc::new(Mutex::new(receiver)),
            channels: Default::default(),
            demux: Default::default(),
        }
    }

    // TODO: Check naming, `from_*` is suspicious.
    pub fn from_tcp_stream(stream: TcpStream) -> Self {
        let (receiver, sender) = stream.into_split();
        Connection::new(Box::new(receiver), Box::new(sender))
    }

    pub fn from_unix_stream(stream: UnixStream) -> Self {
        let (receiver, sender) = stream.into_split();
        Connection::new(Box::new(receiver), Box::new(sender))
    }

    pub async fn tcp_connect(addr: impl ToSocketAddrs) -> Result<Self, io::Error> {
        let stream = tokio::time::timeout(
            Duration::from_secs(2),
            TcpStream::connect(&addr),
        ).await??;
        stream.set_nodelay(true)?;
        //stream.set_keepalive_ms(Some(10_000u32)).unwrap();
        Ok(Self::from_tcp_stream(stream))
    }

    #[cfg(target_family = "unix")]
    pub async fn unix_connect(addr: &str) -> Result<Self, io::Error> {
        let stream = tokio::time::timeout(
            Duration::from_secs(2),
            UnixStream::connect(addr),
        ).await??;
        Ok(Self::from_unix_stream(stream))
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
    pub async fn handshake(&mut self, magic: u32) -> Result<(), Error> {
        self.execute(&mut HandshakeProtocol::builder()
            .client()
            .node_to_node()
            .network_magic(magic)
            .build()?).await
    }
    fn register(&mut self, idx: u16) -> Receiver<Payload> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.channels.lock().unwrap().insert(idx, tx);
        rx
    }
    fn unregister(&mut self, idx: u16) {
        self.channels.lock().unwrap().remove(&idx);
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
                let channels = self.channels.clone();
                let demux = Arc::new(Demux::new(task::spawn(async move {
                    let mut receiver = receiver.lock().await;
                    loop {
                        let mut header = [0u8; 8];
                        receiver.read_exact(&mut header).await.unwrap();
                        trace!("Header: {}", hex::encode(&header));
                        let _timestamp = NetworkEndian::read_u32(&mut header[0..4]);
                        let idx = NetworkEndian::read_u16(&header[4..6]) as u16 ^ 0x8000;
                        let length = NetworkEndian::read_u16(&header[6..]) as usize;
                        //trace!("Reading payload, idx={} length={}.", idx, length);
                        let mut payload = vec![0u8; length];
                        receiver.read_exact(&mut payload).await.unwrap();
                        channels.lock().unwrap()[&idx].send(payload).unwrap();
                    };
                })));
                *demux_lock = Arc::downgrade(&demux);
                demux
            }
        }
    }
}
