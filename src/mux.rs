/**
© 2020 - 2022 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use byteorder::{ByteOrder, NetworkEndian};
use crate::{
    Protocol,
    Agency,
    protocols::handshake::Handshake,
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

    #[cfg(target_family = "unix")]
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

    pub(crate) fn execute<'a, P>(&'a mut self, protocol: &'a mut P) -> Channel<'a, P>
    where
        P: Protocol,
    {
        Channel::new(protocol, self)
    }
    pub async fn handshake(&mut self, magic: u32) -> Result<(), Error> {
        Handshake::builder()
            .client()
            .node_to_node()
            .network_magic(magic)
            .build()?
            .run(self).await
    }
    fn register(&mut self, idx: u16) -> Receiver<Payload> {
        trace!("Registering protocol {}.", idx);
        let (tx, rx) = mpsc::unbounded_channel();
        self.channels.lock().unwrap().insert(idx, tx);
        rx
    }
    fn unregister(&mut self, idx: u16) {
        trace!("Unregistering protocol {}.", idx);
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

pub(crate) struct Channel<'a, P: Protocol> {
    idx: u16,
    receiver: Receiver<Payload>,
    pub(crate) protocol: &'a mut P,
    connection: &'a mut Connection,
    _demux: Arc<Demux>,
    bytes: Vec<u8>,
}

impl<'a, P: Protocol> Channel<'_, P> {
    fn new(protocol: &'a mut P, connection: &'a mut Connection) -> Channel<'a, P> {
        let idx = protocol.protocol_id();
        let receiver = connection.register(idx);
        let demux = connection.run_demux();
        Channel {
            idx,
            receiver,
            protocol,
            connection,
            _demux: demux,
            bytes: Vec::new(),
        }
    }

    pub(crate) async fn execute(&mut self) -> Result<(), Error> {
        trace!("Executing protocol {}.", self.idx);
        loop {
            let agency = self.protocol.agency();
            if agency == Agency::None {
                break;
            }
            let role = self.protocol.role();
            if agency == role {
                self.connection.send(self.idx, &self.protocol.send_bytes().unwrap()).await;
            } else {
                let mut bytes = std::mem::replace(&mut self.bytes, Vec::new());
                let new_data = self.connection.recv(&mut self.receiver).await;
                bytes.extend(new_data);
                self.bytes = self.protocol.receive_bytes(bytes).unwrap_or(Box::new([])).into_vec();
                if !self.bytes.is_empty() {
                    trace!("Keeping {} bytes for the next frame.", self.bytes.len());
                }
            }
        }
        Ok(())
    }
}

impl<'a, P: Protocol> Drop for Channel<'_, P> {
    fn drop(&mut self) {
        self.connection.unregister(self.idx);
    }
}
