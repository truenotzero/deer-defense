
use engine_2d::ecs::*;

use engine_2d::math::Vec2;
use engine_2d::render::sprite;

use engine_2d::ecs;

component! {
    struct Transform {
        #[slot(0)] scale: Vec2,
        #[slot(1)] rotation: Vec2,
        #[slot(2)] position: Vec2,
    }
}

component! {
    struct Sprite {
        #[slot(0)] sprite: sprite::Sprite,
        #[slot(1)] junk: isize,
    }
}

