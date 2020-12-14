/**
© 2020 PERLUR Group

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

use crate::{
    Agency, Protocol,
    protocols::handshake::HandshakeProtocol,
};

pub async fn connect(host: &str, port: u16) -> io::Result<Channel> {
    /* TODO: Consider asynchronous operations */
    let saddr = (host, port).to_socket_addrs()?.nth(0)
        .ok_or(Error::new(ErrorKind::NotFound, "No valid host found!"))?;
    Ok(Channel::new(TcpStream::connect_timeout(&saddr, Duration::from_secs(2))?))
}

pub struct Channel {
    shared: Rc<RefCell<ChannelShared>>,
}

impl Channel {
    pub fn new(stream: TcpStream) -> Self {
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
        self.execute(HandshakeProtocol::new(magic)).await
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
        }
        loop {
            if proto.borrow_mut().get_agency() == Agency::None {
                return match Rc::try_unwrap(proto) {
                    Ok(protocol) => protocol.into_inner().result(),
                    Err(_) => panic!("Unexpected reference to a subchannel."),
                }
            }

            {
                let mut shared = shared.borrow_mut();
                /* TODO: Consider using async operations and select! */
                shared.process_tx().await;
                shared.process_rx().await;
            }
        }
    }
}

struct ChannelShared {
    start_time: Instant,
    stream: TcpStream,
    protocols: Vec<Weak<RefCell<Box<dyn Protocol>>>>,
}

impl ChannelShared {
    async fn process_tx(&mut self) {
        for subchannel in &self.protocols {
            match subchannel.upgrade() {
                Some(protocol) => {
                    let mut protocol = protocol.borrow_mut();
                    match protocol.get_agency() {
                        Agency::Client => {
                            let payload = protocol.send_data().unwrap();
                            let id = protocol.protocol_id();
                            let mut msg = Vec::new();
                            msg.write_u32::<NetworkEndian>(self.start_time.elapsed().as_micros() as u32).unwrap();
                            msg.write_u16::<NetworkEndian>(id).unwrap();
                            msg.write_u16::<NetworkEndian>(payload.len() as u16).unwrap();
                            msg.write(&payload[..]).unwrap();
                            /* TODO:
                             *   * Asynchronous Tx.
                             *   * Handle errors.
                             */
                            self.stream.write(&msg).unwrap();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    async fn process_rx(&mut self) {
        let mut header = [0u8; 8];
        /* TODO:
         *   * Asynchronous Rx.
         *   * Handle errors.
         */
        self.stream.read_exact(&mut header).unwrap(); // TODO: Handle/ignore error.
        let length = NetworkEndian::read_u16(&header[6..]) as usize;
        let mut payload = vec![0u8; length];
        self.stream.read_exact(&mut payload).unwrap(); // TODO: Handle/ignore error.
        let _timestamp = NetworkEndian::read_u32(&mut header[0..4]);
        let idx = NetworkEndian::read_u16(&mut header[4..6]) as usize ^ 0x8000;
        match self.lookup(idx) {
            Some(cell) => {
                let mut protocol = cell.borrow_mut();
                protocol.receive_data(payload);
            }
            None => {}
        }
    }
    fn lookup(&self, id: usize) -> Option<Rc<RefCell<Box<dyn Protocol>>>> {
        match self.protocols.get(id) {
            Some(weakref) => weakref.upgrade(),
            None => None,
        }
    }
}
