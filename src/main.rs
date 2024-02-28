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
use engine_2d::shader;
use engine_2d::time::Cooldown;
use engine_2d::time::Timer;
use engine_2d::window::DrawContext;
use engine_2d::window::Engine;
use engine_2d::window::GameLoop;
use engine_2d::window::WindowManager;
use entities::EntityManager;
use entities::KeyEvent;
use socket::Client;
use socket::Packet;

use crate::common::EntitySpawn;
use crate::common::EntityUpdate;
use crate::common::OpCode;

mod common;
mod entities;
mod server;
mod socket;

fn make_shader<'c>(ctx: &'c DrawContext) -> Shader<'c> {
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
pub struct Game<'e, 's: 'e> {
    ping_timer: Timer,
    player_pos_timer: Timer,
    timeout_timer: Timer,
    shot_cooldown: Cooldown,

    prx: Receiver<Vec2>,
    ktx: Sender<(bool, bool, bool, bool)>,

    shader: Shader<'s>,
    ents: EntityManager<'e, 's>,

    sock: Arc<socket::Client>,
    rx_packet: Receiver<Packet>,
    server_to_local_id: HashMap<i32, i32>,
    player_id: i32,
}

impl<'e, 's: 'e, 'c: 's> GameLoop<'c> for Game<'e, 's> {
    fn setup(ctx: &'c DrawContext, wm: &mut WindowManager) -> Self {
        let sock = Arc::new(socket::Client::new().unwrap());
        sock.connect((Ipv4Addr::LOCALHOST, 7777)).unwrap();

        let sock_ = sock.clone();
        let (tx, rx) = mpsc::channel();
        let (ptx, prx) = mpsc::channel();
        let (ktx, krx) = mpsc::channel();
        thread::spawn(move || recv_loop(sock_, tx));

        let mut ents = EntityManager::default();
        ents.load_sprite(ctx, SpriteName::Tile, Path::new("tile.png"));
        ents.load_sprite(ctx, SpriteName::Deer, Path::new("deer.png"));
        ents.load_sprite(ctx, SpriteName::Forest, Path::new("pine.png"));
        ents.load_sprite(ctx, SpriteName::Spit, Path::new("spit.png"));
        ents.load_sprite(ctx, SpriteName::Hunter, Path::new("hunter.png"));
        ents.create_forest();
        let player_id = ents.spawn_player(krx, ptx, &sock);

        let shader = make_shader(&ctx);
        Self {
            ktx,
            prx,
            shader,
            ents,
            sock,
            server_to_local_id: HashMap::new(),
            rx_packet: rx,
            player_id,
            ping_timer: Timer::new(Duration::from_secs(1)),
            player_pos_timer: Timer::new(Duration::from_millis(50)),
            timeout_timer: Timer::new(TIMEOUT),
            shot_cooldown: Cooldown::new(Duration::from_millis(250)),
        }
    }

    fn tick(&mut self, dt: Duration, wm: &mut WindowManager) {
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

                        let lid = self
                            .ents
                            .spawn(e.pos, e.scale, e.speed, 0.0, e.dir, sprite, e.kind);
                        self.server_to_local_id.insert(e.id, lid);
                        // println!("Spawning entity ({:?}) sid=[{}], lid=[{}]", e.kind, e.id, lid);
                    }
                    OpCode::EntityUpdate => {
                        let e = EntityUpdate::try_from(p).unwrap();
                        let lid = self.server_to_local_id[&e.id];
                        // ents.set_position(lid, e.pos);
                        let d = e.pos - self.ents.get(lid).pos();
                        self.ents.get_mut(lid).set_direction(d);
                    }
                    OpCode::EntityDestroy => {
                        let e = EntityDestroy::try_from(p).unwrap();
                        // println!("client: entity destroy sid=[{}]", e.id);
                        let lid = self.server_to_local_id[&e.id];
                        self.ents.destroy(lid);
                    } // _ => (),
                }
            }
        }
        /* TODO:
                let w = self.get_key(Key::W);
                let a = self.get_key(Key::A);
                let s = self.get_key(Key::S);
                let d = self.get_key(Key::D);
                let space = self.get_key(Key::Space);
                self.ktx.send((w, a, s, d)).unwrap();
        */
        let space = false;

        self.ents.tick(dtf);

        let send_player_pos = self.player_pos_timer.tick(dt);

        let player_pos = self.prx.recv().unwrap();
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
            self.ents.spawn(
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

    fn draw(&mut self, ctx: &'c DrawContext, wm: &mut WindowManager) {
        render::clear();
        self.ents.render(&self.shader);
    }
}

/*
impl Game {
    fn get_key(&self, key: Key) -> bool {
        self.wnd.handle.get_key(key) == Action::Press
    }
}
*/

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let mut client_ip = Ipv4Addr::LOCALHOST;
    let default_port = 7777;
    let force_server = true;
    if force_server {
        thread::spawn(move || server::run(default_port));
    } else if args.len() > 1 {
        match args[1].as_str() {
            "server" => {
                thread::spawn(move || server::run(default_port));
            }
            ip => client_ip = Ipv4Addr::from_str(ip).expect("Expected IP address"),
        }
    }

    let window = WindowManager::new(1200, 1200, "Deer Defense");
    let mut engine = Engine::new(window);
    engine.run::<Game>();
}
