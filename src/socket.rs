use std::io;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::ops::Deref;

const DEFAULT_ADDRESS: (Ipv4Addr, u16) = (Ipv4Addr::UNSPECIFIED, 0);

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum OpCode {
    // client: initializes a connection
    // server: sets the private port for communication
    Hello,
    KeepAlive,
    UserDefined,
}

impl From<OpCode> for u8 {
    fn from(value: OpCode) -> Self {
        value as _
    }
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct Packet {
    opcode: u8,
    data: Vec<u8>,
}

struct NoData;

impl AsRef<[u8]> for NoData {
    fn as_ref(&self) -> &[u8] {
        &[]
    }
}

impl From<Packet> for Vec<u8> {
    fn from(value: Packet) -> Self {
        value.data
    }
}

impl Packet {
    pub fn new<O, T>(op: O, data: T) -> Self
    where
        O: Into<u8>,
        T: AsRef<[u8]>,
    {
        Self {
            opcode: op.into(),
            data: Vec::from(data.as_ref()),
        }
    }

    pub fn opcode<T: From<u8>>(&self) -> T {
        self.opcode.into()
    }

    pub fn send_to(self, socket: &UdpSocket, address: Option<SocketAddr>) -> io::Result<()> {
        let mut buf = vec![self.opcode];
        buf.extend(self.data.into_iter());
        if let Some(address) = address {
            socket.send_to(&buf, address).and(Ok(()))
        } else {
            socket.send(&buf).and(Ok(()))
        }
    }

    pub fn recv_from(socket: &UdpSocket) -> io::Result<(Self, SocketAddr)> {
        const LEN: usize = 256;
        let mut buf = vec![0; LEN];
        let (len, addr) = socket.recv_from(&mut buf)?;
        Ok((
            Self {
                opcode: buf.remove(0),
                data: buf,
            },
            addr,
        ))
    }
}

pub struct Client {
    socket: UdpSocket,
}

impl Deref for Client {
    type Target = UdpSocket;

    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

impl Client {
    pub fn new() -> io::Result<Self> {
        let socket = UdpSocket::bind(DEFAULT_ADDRESS)?;
        Ok(Self { socket })
    }

    pub fn connect<A: ToSocketAddrs>(&self, address: A) -> io::Result<()> {
        let mut address = address
            .to_socket_addrs()?
            .next()
            .ok_or(io::ErrorKind::AddrNotAvailable)?;
        let hello = Packet::new(OpCode::Hello, NoData);
        hello.send_to(self, Some(address))?;
        let (port_packet, _) = Packet::recv_from(self)?;
        if port_packet.opcode != OpCode::Hello.into() {
            Err(io::ErrorKind::InvalidData.into())
        } else {
            let port_data: Vec<u8> = port_packet.into();
            let port_buf = port_data[..2].try_into().unwrap();
            let port = u16::from_ne_bytes(port_buf);
            address.set_port(port);
            self.socket.connect(address)?;
            Ok(())
        }
    }

    pub fn keep_alive(&self) -> io::Result<()> {
        self.ping_pong(OpCode::KeepAlive)
    }

    fn ping_pong(&self, op: OpCode) -> io::Result<()> {
        let ping = Packet::new(op, NoData);
        ping.send_to(self, None)?;
        let (pong, _) = Packet::recv_from(self)?;
        if pong.opcode == op.into() {
            Ok(())
        } else {
            Err(io::ErrorKind::InvalidData.into())
        }
    }
}

pub struct Server {
    socket: UdpSocket,
}

impl Deref for Server {
    type Target = UdpSocket;

    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

impl Server {
    pub fn listen(port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
        Ok(Self { socket })
    }

    pub fn accept(&self) -> io::Result<UdpSocket> {
        loop {
            let (packet, address) = Packet::recv_from(&self)?;
            println!("Got some data!");
            if packet.opcode == OpCode::Hello as _ {
                let client = UdpSocket::bind(DEFAULT_ADDRESS)?;
                let new_port = client.local_addr().unwrap().port();
                Packet::new(OpCode::Hello, &new_port.to_ne_bytes())
                    .send_to(&self, Some(address))?;
                client.connect(address)?;
                println!("new client!");
                break Ok(client);
            }
        }
    }
}
