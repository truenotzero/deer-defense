use std::convert::Infallible;
use std::env;
use std::io;
use std::io::stdin;
use std::io::stdout;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::UdpSocket;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use engine_2d::ecs;
use engine_2d::render;

extern crate engine_2d;
use engine_2d::render::window as glfw;
use engine_2d::render::window::Action;
use engine_2d::render::window::Context;
use engine_2d::render::window::Key;
use socket::NoData;
use socket::OpCode;
use socket::Packet;

mod client;
mod common;
mod comp;
mod server;
mod socket;

struct Engine {
    // windowing
    glfw: glfw::Glfw,
    handle: glfw::PWindow,
    event_queue: glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,

    // ecs
    entity_man: ecs::EntityManager<'static>,
}

impl Default for Engine {
    fn default() -> Self {
        // defaults
        let default_width = 1200;
        let default_height = 1200;
        let default_title = "Deer Defense";

        // hints

        // actual creation
        let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();
        let (mut handle, event_queue) = glfw
            .create_window(
                default_width,
                default_height,
                default_title,
                glfw::WindowMode::Windowed,
            )
            .unwrap();

        // window configuration
        handle.set_key_polling(true);
        handle.make_current();

        // initialize openGL
        render::init(|proc| handle.get_proc_address(proc));

        Self {
            glfw,
            handle,
            event_queue,

            entity_man: Default::default(),
        }
    }
}

impl Engine {
    fn run(mut self) {
        while !self.handle.should_close() {
            // update logic
            self.glfw.poll_events();
            for (_, event) in glfw::flush_messages(&self.event_queue) {
                match event {
                    glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                        self.handle.set_should_close(true)
                    }
                    _ => (),
                }
            }

            self.tick();

            // draw logic
            render::clear();
            self.render();
            self.handle.swap_buffers();
        }
    }

    fn tick(&mut self) {}

    fn render(&mut self) {}
}

const DEFAULT_ADDRESS: (Ipv4Addr, u16) = (Ipv4Addr::UNSPECIFIED, 0);

// fn client() {
//     let udp = UdpSocket::bind(DEFAULT_ADDRESS).unwrap();
//     let remote_addr = Ipv4Addr::LOCALHOST;
//     let remote_port = 7777;
//     udp.connect((remote_addr, remote_port)).unwrap();
//     loop {
//         print!("> ");
//         stdout().flush().unwrap();

//         let mut buf = String::new();
//         stdin().read_line(&mut buf).unwrap();
//         // udp.send_to(buf.as_bytes(), remote).unwrap();
//         udp.send(buf.as_bytes()).unwrap();

//         let mut port_buf = [0; 2];
//         let (_, addy) = udp.recv_from(&mut port_buf).unwrap();
//         println!("Got a message from {addy}");

//         let private_port = u16::from_ne_bytes(port_buf);
//         udp.connect((remote_addr, private_port)).unwrap();
//         println!("Connected to private port {private_port}");

//         let mut bbuf = vec![0; 256];
//         udp.recv(&mut bbuf).unwrap();
//         let inbound = String::from_utf8(bbuf).unwrap();
//         println!("< {inbound}");
//     }
// }

// fn server() {
//     let udp = UdpSocket::bind("0.0.0.0:7777").unwrap();
//     loop {
//         let mut buf = vec![0; 256];
//         let (len, addy) = udp.recv_from(&mut buf).unwrap();
//         println!("Got a message from {addy}");

//         let client = UdpSocket::bind(DEFAULT_ADDRESS).unwrap();
//         let port = client.local_addr().unwrap().port();
//         udp.send_to(&port.to_ne_bytes(), addy).unwrap();

//         // empirical value
//         // prevents a bug where the next data is received before
//         // the client can reconnect
//         sleep(Duration::from_micros(10));
//         client.connect(addy).unwrap();
//         client.send(&buf[..len]).unwrap();

//         // udp.send_to(&buf, addy).unwrap();

//         let inbound = String::from_utf8(buf).unwrap();
//         println!("= {inbound}");
//     }
// }

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
enum MyOpcodes {
    StringData = socket::OpCode::UserDefined as _,
}

impl From<MyOpcodes> for u8 {
    fn from(value: MyOpcodes) -> Self {
        value as _
    }
}

impl From<u8> for MyOpcodes {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl<T: AsRef<str>> From<T> for Packet {
    fn from(value: T) -> Self {
        Self::new(MyOpcodes::StringData, value.as_ref())
    }
}

impl TryFrom<Packet> for String {
    type Error = socket::Error;
    fn try_from(value: Packet) -> socket::Result<Self> {
        if MyOpcodes::StringData == value.opcode() {
            Ok(String::from_utf8(value.into()).unwrap())
        } else {
            Err(socket::Error::BadOpcode)
        }
    }
}

fn client() {
    let client = socket::Client::new().unwrap();
    client.connect("127.0.0.1:7777").unwrap();
    loop {
        print!("> ");
        stdout().flush().unwrap();

        let mut user_input = String::new();
        stdin().read_line(&mut user_input).unwrap();

        client.send(user_input).unwrap();
        let pong: String = client.recv().unwrap();
        println!("< {pong}");
    }
}

fn server_connectionful() {
    let mut server = socket::Server::listen(7777).unwrap();
    loop {
        let client = server.accept().unwrap();
        thread::spawn(move || {
            loop {
                let ping: Packet = client.recv().unwrap();
                if OpCode::UserDefined != ping.opcode() {
                    println!("bad data, closing connection with client");
                    break;
                }

                let data = ping.clone().into();
                let data_str = String::from_utf8(data).unwrap();
                println!("= {data_str}");

                client.send(ping).unwrap();
            }
        });
    }
}

fn server_connectionless() {
    let server = socket::Server::listen(7777).unwrap();
    loop {
        let (data, address): (String,_) = server.recv().unwrap();
        println!("= {data}");
        server.send(data, address).unwrap();
    }
}

fn main() {
    //let window = Engine::default();
    //window.run();

    let err = "provide `client` or `server`";
    let args = env::args().collect::<Vec<_>>();
    let arg = args.get(1).expect(err);
    match arg.as_str() {
        "client" => client(),
        "server" => server_connectionless(),
        "server-con" => server_connectionful(),
        e => panic!("{err}, got `{e}`"),
    }
}
