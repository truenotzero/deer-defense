use std::convert::Infallible;
use std::fmt::Formatter;
use std::io;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::Weak;
use std::thread::sleep;
use std::time::Duration;

const DEFAULT_ADDRESS: (Ipv4Addr, u16) = (Ipv4Addr::UNSPECIFIED, 0);

#[derive(Debug)]
pub enum Error {
    NotEnoughData,
    BadAddress,
    BadOpcode,
    IoError(io::Error),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Error::NotEnoughData => "not enough data in received message",
            Error::BadAddress => "bad address/port",
            Error::BadOpcode => "bad opcode",
            Error::IoError(_) => "std::io::error: ",
        };

        write!(f, "{msg}")
    }
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unimplemented!()
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum OpCode {
    /// Sent by the client to initiat a connection
    /// Sent by the server to aknowledge the connection
    Hello,

    /// Sent by one side to indicate they are changing ports
    Port,

    /// KeepAlive request
    Ping,

    /// KeepAlive response
    Pong,

    /// allows users to have their own opcode enums without using reserved opcode values
    /// user defined enums should define:
    /// From<u8>, Into<u8>, Clone, Copy, PartialEq
    /// pub enum UserOpCodes {
    ///     MyCustomOpcode = OpCode::UserDefined as _,
    /// }
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

pub struct NoData;

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

    pub fn send_to(self, socket: &UdpSocket, address: Option<SocketAddr>) -> Result<()> {
        let mut buf = vec![self.opcode];
        buf.extend(self.data.into_iter());
        if let Some(address) = address {
            socket.send_to(&buf, address).and(Ok(())).map_err(Into::into)
        } else {
            socket.send(&buf).and(Ok(())).map_err(Into::into)
        }
    }

    pub fn recv_from(socket: &UdpSocket) -> Result<(Self, SocketAddr)> {
        const LEN: usize = 256;
        let mut buf = vec![0; LEN];
        let (_, addr) = socket.recv_from(&mut buf)?;
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

impl Client {
    pub fn new() -> Result<Self> {
        let socket = UdpSocket::bind(DEFAULT_ADDRESS)?;
        let default_timeout = Duration::from_secs(10);
        socket.set_read_timeout(Some(default_timeout)).unwrap();
        socket.set_write_timeout(Some(default_timeout)).unwrap();
        Ok(Self { socket })
    }

    pub fn connect<A: ToSocketAddrs>(&self, address: A) -> Result<()> {
        let address = address
            .to_socket_addrs()?
            .next()
            .ok_or(Error::BadAddress)?;
        self.socket.connect(address)?;
        self.send(Packet::new(OpCode::Hello, NoData))?;
        let hello_reply: Packet = self.recv()?;
        if OpCode::Hello != hello_reply.opcode() {
            Err(Error::BadOpcode)
        } else {
            Ok(())
        }
    }

    pub fn send<P: Into<Packet>>(&self, packet: P) -> Result<()> {
        packet.into().send_to(&self.socket, None)
    }

    pub fn recv<E: Into<Error>, P: TryFrom<Packet, Error=E>>(&self) -> Result<P> {
        loop {
            let (packet, _) = Packet::recv_from(&self.socket)?;
            match packet.opcode() {
                OpCode::Ping => self.send(Packet::new(OpCode::Pong, NoData))?,
                OpCode::Port => {
                    let port = Vec::from(packet)[..2].try_into().unwrap();
                    self.set_remote_port(u16::from_ne_bytes(port))?;
                }
                _ => break packet,
            }
        }.try_into().map_err(Into::into)
    }

    fn set_remote_port(&self, port: u16) -> Result<()> {
        let mut address = self.socket.peer_addr().unwrap();
        address.set_port(port);
        self.socket.connect(address).map_err(Into::into)
    }
}

pub struct Server {
    socket: UdpSocket,
    clients: Vec<Weak<Client>>
}

impl Server {
    pub fn listen(port: u16) -> Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
        Ok(Self {
            socket,
            clients: Default::default(),
         })
    }

    /// connectionless mode
    pub fn recv<E: Into<Error>, P: TryFrom<Packet, Error=E>>(&self) -> Result<(P, SocketAddr)> {
        let (packet, address) = loop {
            let (packet, address) = Packet::recv_from(&self.socket)?;
            match packet.opcode() {
                OpCode::Hello => self.send(Packet::new(OpCode::Hello, NoData), address)?,
                OpCode::Ping => self.send(Packet::new(OpCode::Pong, NoData), address)?,
                _ => break (packet, address),
            }
        };

        let packet = packet.try_into().map_err(Into::into)?;
        Ok((packet, address))
    }

    /// connectionless mode
    pub fn send<P: Into<Packet>>(&self, packet: P, address: SocketAddr) -> Result<()> {
        packet.into().send_to(&self.socket, Some(address))
    }

    /// connectionful mode
    pub fn accept(&mut self) -> Result<Arc<Client>> {
        loop {
            let (packet, address) = Packet::recv_from(&self.socket)?;
            println!("Got some data!");
            if OpCode::Hello == packet.opcode() {
                let client = Arc::new(Client::new()?);
                client.socket.connect(address)?;

                let new_port = client.socket.local_addr().unwrap().port();
                Packet::new(OpCode::Port, &new_port.to_ne_bytes()).send_to(&self.socket, Some(address))?;
                sleep(Duration::from_millis(1));
                client.send(Packet::new(OpCode::Hello, NoData))?;

                self.clients.push(Arc::downgrade(&client));
                println!("new client!");
                break Ok(client);
            }
        }
    }

    // pub fn broadcast<P: Into<Packet>>(&mut self, packet: P) -> Result<()> {
    //     let mut removals = Vec::new();
    //     let packet = packet.into();

    //     for (id, client) in self.clients.iter().enumerate() {
    //         if let Some(client) = client.upgrade() {

    //         } else {
    //             removals.push(id);
    //         }
    //     }

    //     unimplemented!()
    // }
}
