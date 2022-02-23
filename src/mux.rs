//
// © 2020 - 2022 PERLUR Group
//
// Re-licenses under MPLv2
// © 2022 PERLUR Group
//
// SPDX-License-Identifier: MPL-2.0
//

use byteorder::{
    ByteOrder,
    NetworkEndian,
};
use log::trace;
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc,
        Weak,
    },
    time::{
        Duration,
        Instant,
    },
};
use tokio;
use tokio::{
    io::{
        AsyncRead,
        AsyncReadExt,
        AsyncWrite,
        AsyncWriteExt,
    },
    net::TcpStream,
    net::ToSocketAddrs,
    sync::{
        mpsc,
        Mutex,
    },
    task,
};

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
    fn new(
        receiver: Box<dyn AsyncRead + Unpin + Send>,
        sender: Box<dyn AsyncWrite + Unpin + Send>,
    ) -> Self {
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
        let stream =
            tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await??;
        stream.set_nodelay(true)?;
        //stream.set_keepalive_ms(Some(10_000u32)).unwrap();
        Ok(Self::from_tcp_stream(stream))
    }

    #[cfg(target_family = "unix")]
    pub async fn unix_connect(addr: &str) -> Result<Self, io::Error> {
        let stream =
            tokio::time::timeout(Duration::from_secs(2), UnixStream::connect(addr)).await??;
        Ok(Self::from_unix_stream(stream))
    }

    pub fn duration(&self) -> Duration {
        self.start_time.elapsed()
    }

    #[cfg(test)]
    #[cfg(target_family = "unix")]
    pub fn test_unix_pair() -> Result<(Connection, Connection), io::Error> {
        let (left, right) = UnixStream::pair()?;
        Ok((Connection::from_unix_stream(left), Connection::from_unix_stream(right)))
    }

    pub fn channel<'a>(&'a mut self, idx: u16) -> Channel<'a> {
        let receiver = self.register(idx);
        let demux = self.run_demux();
        Channel {
            idx,
            receiver,
            connection: self,
            _demux: demux,
            bytes: Vec::new(),
        }
    }

    fn register(&mut self, idx: u16) -> Receiver<Payload> {
        trace!("Registering channel {}.", idx);
        let (tx, rx) = mpsc::unbounded_channel();
        self.channels.lock().unwrap().insert(idx, tx);
        rx
    }
    fn unregister(&mut self, idx: u16) {
        trace!("Unregistering channel {}.", idx);
        self.channels.lock().unwrap().remove(&idx);
    }
    async fn send(&self, idx: u16, payload: &[u8]) {
        let mut sender = self.sender.lock().await;
        let start_time = Instant::now();
        sender
            .write_u32(start_time.elapsed().as_micros() as u32)
            .await
            .unwrap();
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
                        match channels.lock().unwrap().get(&idx) {
                            Some(channel) => channel.send(payload).unwrap(),
                            None => error!("Channel 0x{:04x} not attached.", idx),
                        }
                    }
                })));
                *demux_lock = Arc::downgrade(&demux);
                demux
            }
        }
    }
}

pub struct Channel<'a> {
    idx: u16,
    receiver: Receiver<Payload>,
    connection: &'a mut Connection,
    _demux: Arc<Demux>,
    pub(crate) bytes: Vec<u8>,
}

impl<'a> Channel<'a> {
    pub(crate) fn get_index(&self) -> u16 {
        self.idx
    }

    pub(crate) async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        Ok(self.connection.send(self.idx, &data).await)
    }

    pub(crate) async fn recv(&mut self) -> Result<Vec<u8>, Error> {
        Ok(self.connection.recv(&mut self.receiver).await)
    }

    #[cfg(test)]
    pub(crate) async fn expect(&mut self, data: &[u8]) {
        assert_eq!(
            self.recv().await.unwrap(),
            data,
        );
    }
}

impl Drop for Channel<'_> {
    fn drop(&mut self) {
        self.connection.unregister(self.idx);
    }
}
