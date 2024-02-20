#![feature(more_qualified_paths)]

use std::cell::RefCell;
use std::env;
use std::net::Ipv4Addr;
use std::path::Path;
use std::thread;

use engine_2d::ecs;
use engine_2d::ecs::Entity;
use engine_2d::ecs::EntityManager;
use engine_2d::render;

extern crate engine_2d;
use engine_2d::render::shader::IShaderBuilder;
use engine_2d::render::shader::PartType;
use engine_2d::render::shader::Shader;
use engine_2d::render::shader::ShaderBuilder;
use engine_2d::render::shader::ShaderPart;
use engine_2d::render::sprite::ISprite;
use engine_2d::render::sprite::Sprite;
use engine_2d::render::texture::ITexture;
use engine_2d::render::texture::Texture;
use engine_2d::render::window as glfw;
use engine_2d::render::window::Action;
use engine_2d::render::window::Context;
use engine_2d::render::window::Key;
use engine_2d::shader;
use entity::archetypes::mob;
use entity::systems;
use socket::Client;

mod client;
mod common;
mod entity;
mod event;
mod server;
mod socket;

fn make_shader() -> Shader {
    ShaderBuilder::default()
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

struct Engine {
    // windowing
    glfw: glfw::Glfw,
    handle: glfw::PWindow,
    event_queue: glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
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
        }
    }
}

impl Engine {
    fn run(mut self) {
        let mut entity_man = EntityManager::default();
        let player = entity_man.spawn();
        let player_texture = Texture::from_file(Path::new("deer.png")).unwrap();
        let mut player_sprite = Sprite::default();
        player_sprite.set_texture(player_texture);
        mob::new(&player, (0.0, 0.0).into(), 0.1, 0.01, player_sprite);

        systems::register_event_adapters();

        let shader = make_shader();
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
            self.render(&shader);
            self.handle.swap_buffers();
        }
    }

    fn tick(&mut self) {
        systems::input(&self.handle);
        systems::movement();
    }

    fn render(&mut self, shader: &Shader) {
        systems::draw(&shader);
    }
}

fn server(port: u16) {
    let socket = socket::Server::listen(port).unwrap();
    loop {
        let (p, address): (socket::Packet, _) = socket.recv().unwrap();
        let op = p.opcode::<u8>();
        let data = Vec::from(p);
    }
}

thread_local! {
    pub static CLIENT: socket::Client = Client::new().unwrap();
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() > 1 && args[1] == "server" {
        let default_port = 7777;
        thread::spawn(move || server(default_port));
    }

    CLIENT.with(|c| c.connect((Ipv4Addr::LOCALHOST, 7777)).unwrap());
    let game = Engine::default();
    game.run();
}
