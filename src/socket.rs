
// udp based connection

// simple protocol that implements these base packet types
// hello {} (client -> server -> client)
// keepalive {} (client -> server -> client)
// 
// (de)serialization {u16 payload_sz, payload: [u8]} (client -> server / server -> client)
// packet structure is [opcode:u8 [data]*]

use std::collections::VecDeque;
use std::io;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::UdpSocket;

#[repr(u8)]
pub enum Packet {
    Hello,
    KeepAlive,
    Data { sz: u16, pld: Box<[u8]> },
}

impl Packet {
    // from https://doc.rust-lang.org/std/mem/fn.discriminant.html
    fn opcode(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

pub trait Socket {
    fn send(&self, packet: Packet) {
        let mut data = Vec::new();
        data.push(packet.opcode());

        match &packet {
            Packet::Data { sz, pld } => {
                data.append(&mut Vec::from(sz.to_ne_bytes()));
                data.reserve(pld.len());
                pld.iter().for_each(|&b| data.push(b));
            },
            _ => (),
        }

        self.send_raw(&data);
    }
}

struct SocketIO {
    socket: UdpSocket,
    queue: VecDeque<u8>,
}

impl SocketIO {
    // TODO: move to the server
    pub fn host(port: u16) -> io::Result<Self> {
        let sock_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
        let socket = UdpSocket::bind(sock_addr)?;
        
        Ok(Self { 
            socket,
            queue: Default::default(),
         })
    }

    fn new(socket: UdpSocket) -> Self {
        Self {
            socket,
            queue: Default::default(),
        }
    }

    fn connect()

    fn send_raw(&self, data: &[u8]) {
        self.socket.send(data);
    }

    fn receive_raw(&mut self, num_bytes: usize) -> io::Result<Box<[u8]>> {
        while self.queue.len() < num_bytes {
            let len = self.socket.peek(&mut [])?;
            let mut buf = Vec::new();
            self.socket.recv(&mut buf);

            self.queue.reserve(buf.len());
            buf.into_iter().for_each(|b| self.queue.push_back(b));
        }

        // self.queue is the head
        let tail = self.queue.split_off(num_bytes);
        self.queue.make_contiguous(); // needed because queue is actually a deque
        let ret = self.queue.as_slices().0.into();
        self.queue = tail;

        Ok(ret)
    }
}

struct Client {
    socket: SocketIO,
}

impl Client {
    // TODO: move to client
    pub fn connect(addr: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.connect(addr)?;

        Ok(Self { 
            socket,
         })
    }

}
