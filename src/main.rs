#![feature(more_qualified_paths)]

use std::collections::HashMap;
use std::env;
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use common::EntityDestroy;
use common::EntityKind;
use common::SpriteName;
use common::TIMEOUT;
use engine_2d::render;

extern crate engine_2d;
use engine_2d::math::Vec2;
use engine_2d::render::shader::IShaderBuilder;
use engine_2d::render::shader::PartType;
use engine_2d::render::shader::Shader;
use engine_2d::render::shader::ShaderBuilder;
use engine_2d::render::shader::ShaderPart;
use engine_2d::render::window as glfw;
use engine_2d::render::window::Action;
use engine_2d::render::window::Key;
use engine_2d::render::Context;
use engine_2d::shader;
use entities::EntityManager;
use entities::KeyEvent;
use socket::Client;
use socket::Packet;
use timer::Cooldown;
use timer::Timer;

use crate::common::EntitySpawn;
use crate::common::EntityUpdate;
use crate::common::OpCode;

mod common;
mod entities;
mod event;
mod server;
mod socket;
mod timer;

fn make_shader(ctx: Context<'_>) -> Shader<'_> {
    ShaderBuilder::new(ctx)
        .add_part(shader! {
            #type Vertex
            "#version 450 core

            uniform mat3 uSprite;

            layout (location = 0)
            in vec2 aPos;

            layout (location = 1)
            in vec2 aUV;

            out vec2 texUV;

            void main() {
                vec3 pos = vec3(aPos, 1.0);
                texUV = aUV;
                gl_Position = vec4(uSprite * pos, 1.0);
            }"
        })
        .unwrap()
        .add_part(shader! {
            #type Fragment
            "#version 450 core
            
            uniform sampler2D uTexture;

            in vec2 texUV;

            out vec4 FragColor;

            void main() {
                FragColor = texture(uTexture, texUV);
            }"
        })
        .unwrap()
        .verify()
        .unwrap()
}

struct Window {
    glfw: glfw::Glfw,
    handle: glfw::PWindow,
    event_queue: glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
}

impl Default for Window {
    fn default() -> Self {
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
        glfw::Context::make_current(&mut handle.render_context());

        Self {
            glfw,
            handle,
            event_queue,
        }
    }
}

fn recv_loop(socket: Arc<Client>, tx: Sender<Packet>) {
    loop {
        if let Ok(msg) = socket.recv() {
            tx.send(msg).unwrap();
        } else {
            break;
        }
    }
}

// 'a: 'b means a outlives 'b
// g is the lifetime of gl objects
// c is the lifetime of the gl context
// w is the lifetime of the window
pub struct Engine<'c> {
    wnd: Window,
    ctx: render::Context<'c>,
    sock: Arc<socket::Client>,
    rx_packet: Receiver<Packet>,
    server_to_local_id: HashMap<i32, i32>,
    player_id: i32,

    ping_timer: Timer,
    player_pos_timer: Timer,
    timeout_timer: Timer,
    shot_cooldown: Cooldown,
}

impl<'c> Engine<'c> {
    fn new(mut wnd: Window, sock: Arc<socket::Client>) -> Self {
        let ctx = render::init(|proc| wnd.handle.get_proc_address(proc));
        let sock_ = sock.clone();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || recv_loop(sock_, tx));

        Self {
            wnd,
            ctx,
            sock,
            server_to_local_id: HashMap::new(),
            rx_packet: rx,
            player_id: 0,
            ping_timer: Timer::new(Duration::from_secs(1)),
            player_pos_timer: Timer::new(Duration::from_millis(50)),
            timeout_timer: Timer::new(TIMEOUT),
            shot_cooldown: Cooldown::new(Duration::from_millis(250)),
        }
    }

    fn run(&mut self) {
        let mut ents = EntityManager::default();
        ents.load_sprite(self.ctx, SpriteName::Tile, Path::new("tile.png"));
        ents.load_sprite(self.ctx, SpriteName::Deer, Path::new("deer.png"));
        ents.load_sprite(self.ctx, SpriteName::Forest, Path::new("pine.png"));
        ents.load_sprite(self.ctx, SpriteName::Spit, Path::new("spit.png"));
        ents.load_sprite(self.ctx, SpriteName::Hunter, Path::new("hunter.png"));

        let (tx, rx) = mpsc::channel();
        let (ptx, prx) = mpsc::channel();

        ents.create_forest();
        self.player_id = ents.spawn_player(rx, ptx, &self.sock);

        let shader = make_shader(self.ctx);
        let mut last = Instant::now();
        while !self.wnd.handle.should_close() {
            // update logic
            self.wnd.glfw.poll_events();
            for (_, event) in glfw::flush_messages(&self.wnd.event_queue) {
                match event {
                    glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                        self.wnd.handle.set_should_close(true);
                    }
                    _ => (),
                }
            }

            let now = Instant::now();
            let dt = now - last;
            self.tick(&mut ents, &tx, &prx, dt);
            last = now;

            // draw logic
            render::clear();
            self.render(&ents, &shader);
            glfw::Context::swap_buffers(&mut self.wnd.handle.render_context());
        }
    }

    fn get_key(&self, key: Key) -> bool {
        self.wnd.handle.get_key(key) == Action::Press
    }

    fn tick(
        &mut self,
        ents: &mut EntityManager,
        tx: &Sender<KeyEvent>,
        prx: &Receiver<Vec2>,
        dt: Duration,
    ) {
        let dtf = dt.as_secs_f32();

        if self.ping_timer.tick(dt) {
            let packet = Packet::new(socket::OpCode::Ping, socket::NoData);
            self.sock.send(packet).unwrap();
            // println!("client - ping")
        }

        // if self.timeout_timer.tick(dt) {
        //     panic!("Server timed out");
        // }

        if let Ok(p) = self.rx_packet.try_recv() {
            if socket::OpCode::Pong == p.opcode() {
                self.timeout_timer.reset();
                // println!("client - pong")
            } else {
                match p.opcode() {
                    OpCode::EntitySpawn => {
                        let e = EntitySpawn::try_from(p).unwrap();

                        let sprite = match e.kind {
                            common::EntityKind::Tile => SpriteName::Tile,
                            common::EntityKind::Forest => SpriteName::Forest,
                            common::EntityKind::Player => SpriteName::Deer,
                            common::EntityKind::PlayerProjectile => SpriteName::Spit,
                            common::EntityKind::Enemy => SpriteName::Hunter,
                        };

                        let lid = ents.spawn(e.pos, e.scale, e.speed, 0.0, e.dir, sprite, e.kind);
                        self.server_to_local_id.insert(e.id, lid);
                        // println!("Spawning entity ({:?}) sid=[{}], lid=[{}]", e.kind, e.id, lid);
                    }
                    OpCode::EntityUpdate => {
                        let e = EntityUpdate::try_from(p).unwrap();
                        let lid = self.server_to_local_id[&e.id];
                        // ents.set_position(lid, e.pos);
                        let d = e.pos - ents.get(lid).pos();
                        ents.get_mut(lid).set_direction(d);
                    }
                    OpCode::EntityDestroy => {
                        let e = EntityDestroy::try_from(p).unwrap();
                        // println!("client: entity destroy sid=[{}]", e.id);
                        let lid = self.server_to_local_id[&e.id];
                        ents.destroy(lid);
                    } // _ => (),
                }
            }
        }

        let w = self.get_key(Key::W);
        let a = self.get_key(Key::A);
        let s = self.get_key(Key::S);
        let d = self.get_key(Key::D);
        let space = self.get_key(Key::Space);
        tx.send((w, a, s, d)).unwrap();

        ents.tick(dtf);

        let send_player_pos = self.player_pos_timer.tick(dt);

        let player_pos = prx.recv().unwrap();
        if send_player_pos {
            let p = EntityUpdate {
                id: 0,
                pos: player_pos,
            };
            self.sock.send(p).unwrap();
        }

        if self.shot_cooldown.tick(dt) && space {
            let up = Vec2::new(0.0, 1.0);
            let scale = 6.0;
            let speed = 30.0;
            ents.spawn(
                player_pos,
                scale,
                speed,
                0.0,
                up,
                SpriteName::Spit,
                EntityKind::PlayerProjectile,
            );
            let projectile_spawn = EntitySpawn {
                id: 0,
                kind: EntityKind::PlayerProjectile,
                pos: player_pos,
                scale,
                speed,
                dir: up,
            };
            self.sock.send(projectile_spawn).unwrap();
            self.shot_cooldown.enable();
        }
    }

    fn render(&self, ents: &EntityManager, shader: &Shader) {
        ents.render(shader);
    }
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let mut client_ip = Ipv4Addr::LOCALHOST;
    let default_port = 7777;
    if args.len() > 1 {
        match args[1].as_str() {
            "server" => {
                thread::spawn(move || server::run(default_port));
            }
            ip => client_ip = Ipv4Addr::from_str(ip).expect("Expected IP address"),
        }
    }

    let window = Window::default();
    let sock = Arc::new(socket::Client::new().unwrap());
    sock.connect((client_ip, default_port)).unwrap();
    let mut game = Engine::new(window, sock);

    game.run();
}
