use std::cell::RefCell;

use engine_2d::ecs::*;

use engine_2d::math::Vec2;
use engine_2d::render::sprite;

use engine_2d::ecs;

use crate::event;

component! {
    struct Transform {
        #[slot(0)] scale: Vec2,
        #[slot(1)] rotation: f32,
        #[slot(2)] position: Vec2,
    }
}

component! {
    struct Move {
        #[slot(0)] s: f32, // move speed
        #[slot(1)] d: Vec2, // move direction
    }
}

component! {
    struct Sprite {
        #[slot(0)] sprite: sprite::Sprite,
        #[slot(1)] junk: isize,
    }
}

component! {
    struct MoveEvent {
        #[slot(0)] on_move_event: Option<fn(&event::Data)>,
        #[slot(1)] junk: isize,
    }
}

#[derive(Default)]
struct Components {
    transform: TransformManager,
    movement: MoveManager,
    sprite: SpriteManager,
}


thread_local! {
    static COMPONENTS: RefCell<Components> = RefCell::new(Components::default());
}

pub mod archetypes {

    /// A mob has a Transform, DynamicPos and a Sprite
    pub mod mob {
        use engine_2d::ecs::ComponentManager;
        use engine_2d::ecs::Entity;
        use engine_2d::math::Vec2;
        use engine_2d::render::sprite;

        use crate::entity::Move;
        use crate::entity::Sprite;
        use crate::entity::Transform;
        use crate::entity::COMPONENTS;

        /// Adds Transform, DynamicPos and Sprite to the entity
        pub fn new(e: &Entity, spawn_pos: Vec2, scale: f32, speed: f32, sprite: sprite::Sprite) {
            let mut transform = Transform::default();
            transform.scale = (scale, scale).into();
            transform.position = spawn_pos;

            let mut movement = Move::default();
            movement.s = speed;

            COMPONENTS.with_borrow_mut(|c| {
                c.transform.add(e, Some(transform));
                c.movement.add(e, Some(movement));
                c.sprite.add(e, Some(Sprite { sprite, junk: 0 }));
            })
        }

        pub fn set_pos(e: &Entity, pos: Vec2) -> Option<()> {
            COMPONENTS.with_borrow_mut(|c| {
                *c.transform.get_mut(e)?.position = pos;
                Some(())
            })
        }
    }

    /// A player is controlled by player input
    /// Add mob before adding this
    pub mod player {
        use engine_2d::ecs::Entity;

        use crate::event;

        fn on_move_event(data: &event::Data) {

        }

        pub fn new(e: &Entity) {
            // add keyboard
            // add mouse
        }
    }
}

pub mod systems {
    use engine_2d::ecs::itertools::izip;
    use engine_2d::ecs::ComponentManager;
    use engine_2d::math::Mat3;
    use engine_2d::math::Vec2;
    use engine_2d::render::shader::Shader;
    use engine_2d::render::sprite::ISprite;
    use engine_2d::render::window::Key;
    use engine_2d::render::window::PWindow;

    use crate::event;

    use super::COMPONENTS;

    pub fn draw(shader: &Shader) {
        COMPONENTS.with_borrow(|c| {
            for (t, s) in izip!(c.transform.iter(), c.sprite.iter()) {
                let sprite_matrix = Mat3::translate(*t.position)
                    * Mat3::rotate(*t.rotation)
                    * Mat3::scale(*t.scale);
                s.sprite.draw(shader, sprite_matrix);
            }
        })
    }

    pub fn movement() {
        COMPONENTS.with_borrow_mut(|c| {
            for (t, m) in izip!(c.transform.iter_mut(), c.movement.iter()) {
                *t.position += *m.s * *m.d;
            }
        })
    }

    pub fn input(wnd: &PWindow) {
        // gather raw input data
        // keyboard
        // either 0 for release, 1 for press
        let w = wnd.get_key(Key::W) as i32 as f32;
        let a = wnd.get_key(Key::A) as i32 as f32;
        let s = wnd.get_key(Key::S) as i32 as f32;
        let d = wnd.get_key(Key::D) as i32 as f32;

        // mouse
        // TODO:

        // push events
        // movement
        let d = Vec2::new(d - a, w - s).normalize();
        event::submit(event::Type::Move, event::Data::Move(d));

    }

    pub fn register_event_adapters() {
        use event::Type as T;
        event::subscribe(T::Move, self::on_move);
    }

    fn on_move(data: &event::Data) {
        let &event::Data::Move(d) = data;
        COMPONENTS.with_borrow_mut(|c| {
            for m in c.movement.iter_mut() {
                *m.d = d;
            }
        })
    }
}
