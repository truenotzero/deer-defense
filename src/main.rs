use engine_2d::ecs;
use engine_2d::render;


extern crate engine_2d;
use engine_2d::render::window::Action;
use engine_2d::render::window::Context;
use engine_2d::render::window::Key;
use engine_2d::render::window as glfw;

mod comp;
mod client;
mod server;
mod common;
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
        let (mut handle, event_queue) = glfw.create_window(default_width, default_height, default_title, glfw::WindowMode::Windowed).unwrap();

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
                    glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => self.handle.set_should_close(true),
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

    fn tick(&mut self) {

    }

    fn render(&mut self) {

    }
}

fn main() {
    let window = Engine::default();
    window.run();
}
