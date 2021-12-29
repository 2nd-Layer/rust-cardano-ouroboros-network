/**
© 2020 - 2021 PERLUR Group

SPDX-License-Identifier: GPL-3.0-only OR LGPL-3.0-only

*/

use std::{
    cell::RefCell,
    cmp::max,
    io,
    io::{Error, ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use byteorder::{ByteOrder, NetworkEndian, WriteBytesExt};
use log::{log_enabled, trace};
use net2::TcpStreamExt;
#[cfg(target_family = "unix")]
use std::os::unix::net::UnixStream;

use crate::mux::connection::Stream::Tcp;
#[cfg(target_family = "unix")]
use crate::mux::connection::Stream::Unix;
use crate::protocols::handshake::ConnectionType;
use crate::{protocols::handshake::HandshakeProtocol, Agency, Protocol};

pub enum Stream {
    Tcp(TcpStream),
    #[cfg(target_family = "unix")]
    Unix(UnixStream),
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        match self {
            Tcp(tcp_stream) => tcp_stream.write(buf),
            #[cfg(target_family = "unix")]
            Unix(unix_stream) => unix_stream.write(buf),
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        match self {
            Tcp(tcp_stream) => tcp_stream.flush(),
            #[cfg(target_family = "unix")]
            Unix(unix_stream) => unix_stream.flush(),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        match self {
            Tcp(tcp_stream) => tcp_stream.read(buf),
            #[cfg(target_family = "unix")]
            Unix(unix_stream) => unix_stream.read(buf),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        match self {
            Tcp(tcp_stream) => tcp_stream.read_exact(buf),
            #[cfg(target_family = "unix")]
            Unix(unix_stream) => unix_stream.read_exact(buf),
        }
    }
}

pub async fn connect(host: &str, port: u16) -> io::Result<Channel> {
    /* TODO: Consider asynchronous operations */
    let saddr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or(Error::new(ErrorKind::NotFound, "No valid host found!"))?;
    let stream = TcpStream::connect_timeout(&saddr, Duration::from_secs(2))?;
    stream.set_nodelay(true).unwrap();
    stream.set_keepalive_ms(Some(10_000u32)).unwrap();

    /*
     * We're currently doing blocking I/O, so enabling these helps you see where the code is blocking
     * and will throw errors instead. For now, leave these commented out and only enabled for debugging
     * purposes. Async I/O will become much more important once we're running multiple protocols in parallel.
     */
    // stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    // stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    Ok(Channel::new(Tcp(stream)))
}

#[cfg(target_family = "unix")]
pub async fn connect_unix(socket_path: &str) -> io::Result<Channel> {
    /* TODO: Consider asynchronous operations */
    let stream = UnixStream::connect(socket_path)?;

    /*
     * We're currently doing blocking I/O, so enabling these helps you see where the code is blocking
     * and will throw errors instead. For now, leave these commented out and only enabled for debugging
     * purposes. Async I/O will become much more important once we're running multiple protocols in parallel.
     */
    // stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    // stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    Ok(Channel::new(Unix(stream)))
}

pub struct Channel {
    shared: Rc<RefCell<ChannelShared>>,
}

impl Channel {
    pub fn new(stream: Stream) -> Self {
        Channel {
            shared: Rc::new(RefCell::new(ChannelShared {
                start_time: Instant::now(),
                stream,
                protocols: vec![],
            })),
        }
    }

    pub fn duration(&self) -> Duration {
        self.shared.borrow().start_time.elapsed()
    }

    pub async fn handshake(&self, magic: u32) -> Result<String, String> {
        let connection_type = match self.shared.borrow().stream {
            Tcp(_) => ConnectionType::Tcp,
            #[cfg(target_family = "unix")]
            Unix(_) => ConnectionType::Unix,
        };
        self.execute(HandshakeProtocol::new(magic, connection_type))
            .await
    }

    pub async fn execute(&self, protocol: impl Protocol + 'static) -> Result<String, String> {
        let shared = self.shared.clone();
        let proto = Rc::new(RefCell::new(Box::new(protocol) as Box<dyn Protocol>));
        {
            let mut shared = shared.borrow_mut();
            let id = proto.borrow().protocol_id() as usize;
            let newlen = max(shared.protocols.len(), id + 1);
            shared.protocols.resize(newlen, Weak::new());
            shared.protocols[id] = Rc::downgrade(&proto);
            trace!("started subchannel {:04x}", id);
        }
        loop {
            let agency = proto.borrow_mut().agency();
            if agency == Agency::None {
                return match Rc::try_unwrap(proto) {
                    Ok(protocol) => protocol.into_inner().result(),
                    Err(_) => panic!("Unexpected reference to a subchannel."),
                };
            }

            {
                let mut shared = shared.borrow_mut();

                /* TODO: Consider using async operations and select! */
                shared.process_rx().await?;
                shared.process_tx().await;
            }
        }
    }
}

struct ChannelShared {
    start_time: Instant,
    stream: Stream,
    protocols: Vec<Weak<RefCell<Box<dyn Protocol>>>>,
}

impl ChannelShared {
    async fn process_tx(&mut self) {
        for subchannel in &self.protocols {
            match subchannel.upgrade() {
                Some(protocol) => {
                    let mut protocol = protocol.borrow_mut();
                    if protocol.agency() == protocol.role() {
                        match protocol.send_data() {
                            Some(payload) => {
                                let id = protocol.protocol_id();
                                let mut msg = Vec::new();
                                msg.write_u32::<NetworkEndian>(
                                    self.start_time.elapsed().as_micros() as u32,
                                )
                                .unwrap();
                                msg.write_u16::<NetworkEndian>(id).unwrap();
                                msg.write_u16::<NetworkEndian>(payload.len() as u16)
                                    .unwrap();
                                msg.write_all(&payload[..]).unwrap();
                                /* TODO:
                                 *   * Asynchronous Rx.
                                 *   * Handle errors.
                                 */
                                if log_enabled!(log::Level::Trace) {
                                    trace!("tx bytes: {}", hex::encode(&msg));
                                }

                                let len = self.stream.write(&msg).unwrap();
                                trace!("tx size: {}", len);
                                self.stream.flush().unwrap();
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    async fn process_rx(&mut self) -> Result<(), String> {
        let mut should_receive = false;
        for subchannel in &self.protocols {
            match subchannel.upgrade() {
                Some(protocol) => {
                    let protocol = protocol.borrow();
                    if protocol.agency() != protocol.role() {
                        // We're waiting for at least one protocol
                        should_receive = true;
                        break;
                    }
                }
                _ => {}
            }
        }

        if should_receive {
            let mut header = [0u8; 8];
            /* TODO:
             *   * Asynchronous Rx.
             *   * Handle errors.
             */
            match self.stream.read_exact(&mut header) {
                Ok(_) => {
                    let length = NetworkEndian::read_u16(&header[6..]) as usize;
                    let mut payload = vec![0u8; length];
                    match self.stream.read_exact(&mut payload) {
                        Ok(_) => {
                            trace!(
                                "rx bytes: {} {}",
                                hex::encode(&header),
                                hex::encode(&payload)
                            );
                            let _timestamp = NetworkEndian::read_u32(&mut header[0..4]);
                            let idx = NetworkEndian::read_u16(&mut header[4..6]) as usize ^ 0x8000;
                            match self.lookup(idx) {
                                Some(cell) => {
                                    /* TODO: Verify agency */
                                    let mut protocol = cell.borrow_mut();
                                    protocol.receive_data(payload);
                                }
                                None => {}
                            }
                        }
                        Err(error) => {
                            return Err(format!("payload read error: {:?}", error));
                        }
                    }
                }
                Err(error) => {
                    return Err(format!("header read error: {:?}", error));
                }
            }
        }

        Ok(())
    }
    fn lookup(&self, id: usize) -> Option<Rc<RefCell<Box<dyn Protocol>>>> {
        match self.protocols.get(id) {
            Some(weakref) => weakref.upgrade(),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use futures::executor::block_on;
    use simple_logger::SimpleLogger;

    use super::*;

    #[test]
    fn connection_works() {
        SimpleLogger::new().init().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let cli = thread::spawn(move || {
            block_on(async move {
                let client = connect("127.0.0.1", port).await.unwrap();
                std::thread::sleep(Duration::from_secs(1));
                client.handshake(764824073).await.unwrap();
            })
        });
        let srv = thread::spawn(move || {
            block_on(async move {
                let server = Channel::new(Tcp(listener.accept().unwrap().0));
                server
                    .execute(HandshakeProtocol::expect(764824073, ConnectionType::Tcp))
                    .await
                    .unwrap();
            })
        });

        cli.join().unwrap();
        srv.join().unwrap();
    }
}
