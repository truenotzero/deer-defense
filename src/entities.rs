use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::ops::Not;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use engine_2d::math::Mat3;
use engine_2d::math::Vec2;
use engine_2d::render::shader::Shader;
use engine_2d::render::sprite::ISprite;
use engine_2d::render::sprite::Sprite;
use engine_2d::render::texture::ITexture;
use engine_2d::render::texture::Texture;
use engine_2d::render::Context;
use rand::thread_rng;
use rand::Rng;

use crate::common::EntityKind;
use crate::common::EntitySpawn;
use crate::socket;
use crate::SpriteName;

pub const WORLD_SIZE: isize = 64;

fn world() -> Mat3 {
    // the world is going to be 128 * 128 tiles
    // centered at 0,0
    let scale = 1.0 / (WORLD_SIZE as f32);
    Mat3::scale(Vec2::new(scale, scale))
}

pub trait Entity {
    fn pos(&self) -> Vec2;
    fn kind(&self) -> EntityKind;
    fn scale(&self) -> f32;
    fn speed(&self) ->  f32;
    fn dir(&self) -> Vec2;

    fn set_pos(&mut self, pos: Vec2);
    fn set_direction(&mut self, dir: Vec2);

    fn kill(&mut self);
    fn is_alive(&self) -> bool;

    fn tick(&mut self, dt: f32) -> bool;
    fn render(&self, shader: &Shader);
}

pub struct BaseEntity<'a> {
    alive: bool,
    pos: Vec2,
    scale: f32,
    speed: f32,
    rotation: f32,
    direction: Vec2,
    sprite: Option<Rc<Sprite<'a>>>,
    kind: EntityKind,
}

impl<'a> BaseEntity<'a> {
    pub fn new(
        pos: Vec2,
        scale: f32,
        speed: f32,
        rotation: f32,
        direction: Vec2,
        sprite: Option<Rc<Sprite<'a>>>,
        kind: EntityKind,
    ) -> Self {
        Self {
            alive: true,
            pos,
            scale,
            speed,
            rotation,
            direction,
            sprite,
            kind,
        }
    }
}

impl<'a> Entity for BaseEntity<'a> {
    fn pos(&self) -> Vec2 {
        self.pos
    }

    fn kind(&self) -> EntityKind {
        self.kind
    }

    fn scale(&self) -> f32 {
        self.scale
    }

    fn speed(&self) ->  f32 {
        self.speed
    }

    fn dir(&self) -> Vec2 {
        self.direction
    }

    fn set_pos(&mut self, pos: Vec2) {
        self.pos = pos;
    }

    fn set_direction(&mut self, dir: Vec2) {
        self.direction = dir;
    }

    fn kill(&mut self) {
        self.alive = false;
    }

    fn is_alive(&self) -> bool {
        self.alive
    }

    fn tick(&mut self, dt: f32) -> bool {
        let dpos = self.speed * self.direction.normalize();
        self.pos += dt * dpos;

        let bound = (WORLD_SIZE as f32) * 1.5;
        -bound <= self.pos.x && self.pos.x <= bound && -bound <= self.pos.y && self.pos.y <= bound
    }

    fn render(&self, shader: &Shader) {
        if let Some(sprite) = self.sprite.clone() {
            let sprite_matrix = Mat3::translate(Vec2::new(self.pos.x, self.pos.y))
                * Mat3::rotate(self.rotation)
                * Mat3::scale(Vec2::new(self.scale, self.scale));
            sprite.draw(shader, world() * sprite_matrix);
        }
    }
}

pub type KeyEvent = (bool, bool, bool, bool);

pub struct Player<'a> {
    base: BaseEntity<'a>,
    rx: Receiver<KeyEvent>,
    ptx: Sender<Vec2>,
}

impl<'a> Player<'a> {
    pub fn new(base: BaseEntity<'a>, rx: Receiver<KeyEvent>, ptx: Sender<Vec2>) -> Self {
        Self { base, rx, ptx }
    }
}

impl<'a> Entity for Player<'a> {
    fn pos(&self) -> Vec2 {
        self.base.pos
    }

    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }

    fn scale(&self) -> f32 {
        self.base.scale
    }

    fn speed(&self) ->  f32 {
        self.base.speed
    }

    fn dir(&self) -> Vec2 {
        self.base.direction
    }

    fn set_pos(&mut self, pos: Vec2) {
        self.base.set_pos(pos);
    }

    fn set_direction(&mut self, dir: Vec2) {
        self.base.set_direction(dir)
    }
    fn kill(&mut self) {
        self.base.kill()
    }

    fn is_alive(&self) -> bool {
        self.base.is_alive()
    }

    fn tick(&mut self, dt: f32) -> bool {
        let (w, a, s, d) = self.rx.recv().unwrap();

        let up = (w as i32 as f32) * Vec2::new(0.0, 1.0);
        let left = (a as i32 as f32) * Vec2::new(-1.0, 0.0);
        let down = (s as i32 as f32) * Vec2::new(0.0, -1.0);
        let right = (d as i32 as f32) * Vec2::new(1.0, 0.0);

        self.base.direction = up + left + down + right;
        self.base.tick(dt);

        self.ptx.send(self.base.pos).unwrap();
        true
    }

    fn render(&self, shader: &Shader) {
        self.base.render(shader);
    }
}

#[derive(Default)]
pub struct EntityManager<'e, 's: 'e> {
    sprites: HashMap<SpriteName, Rc<Sprite<'s>>>,
    entities: Vec<(i32, Box<dyn Entity + 'e>)>,
    entity_counter: i32,
}

impl<'e, 's: 'e> EntityManager<'e, 's> {
    pub fn iter(&self) -> impl Iterator<Item=(i32, &dyn Entity)> {
        self.entities.iter().filter(|e| e.1.is_alive()).map(|e| (e.0, e.1.as_ref()))
    }

    pub fn load_sprite<'c: 's>(&mut self, ctx: Context<'c>, name: SpriteName, path: &Path) {
        self.sprites.insert(
            name,
            Rc::new(Sprite::new(ctx, Texture::from_file(ctx, path).unwrap())),
        );
    }

    fn emplace_entity(&mut self, entity: Box<dyn Entity + 'e>) -> i32 {
        let id = self.entity_counter;
        self.entity_counter += 1;

        self.entities.push((id, entity));
        id
    }

    pub fn create_forest(&mut self) {
        // place tiles
        let offset = Vec2::new(1.0, -1.0);
        let scale = 1.0;
        let speed = 0.0;
        let rot = 90.0;
        let dir = Vec2::default();
        let sprite = SpriteName::Tile;

        let mut rng = thread_rng();

        // let rand = rand
        let w = WORLD_SIZE;
        for y in (-w..=w).step_by(2) {
            for x in (-w..=w).step_by(2) {
                let pos = Vec2::new(x as _, y as _);
                let r = rng.gen_range(0..=4);
                let rotation = (r as f32) * rot;
                self.spawn(pos + offset, scale, speed, rotation, dir, sprite, EntityKind::Tile);
            }
        }
        // place trees
    }

    pub fn get(&self, id: i32) -> &dyn Entity {
        let slot = self.entities.iter().find(|(eid, _)| *eid == id).unwrap().0;
        self.entities[slot as usize].1.as_ref()
    }

    pub fn get_mut(&mut self, id: i32) -> &mut dyn Entity {
        let slot = self.entities.iter().find(|(eid, _)| *eid == id).unwrap().0;
        self.entities[slot as usize].1.as_mut()
    }

    pub fn destroy(&mut self, id: i32) {
        // let slot = self.entities.iter().find(|(eid, _)| *eid == id).unwrap().0;
        // self.entities.remove(slot as _);

        self.entities.iter_mut().find(|e| e.0 == id).unwrap().1.kill();
    }

    pub fn spawn(
        &mut self,
        pos: Vec2,
        scale: f32,
        speed: f32,
        rotation: f32,
        dir: Vec2,
        sprite: SpriteName,
        kind: EntityKind,
    ) -> i32 {
        let sprite = self.sprites.get(&sprite).cloned();
        let ent = BaseEntity::new(pos, scale, speed, rotation, dir, sprite, kind);
        self.emplace_entity(Box::new(ent))
    }

    pub fn spawn_enemy(&mut self) -> usize {
        // network this
        unimplemented!()
    }

    pub fn spawn_player<'a: 'e>(&mut self, rx: Receiver<KeyEvent>, ptx: Sender<Vec2>,  sock: &socket::Client) -> i32 {
        let sprite = self.sprites[&SpriteName::Deer].clone();
        let pos = Vec2::new(1.0, 2.0);
        let scale = 4.0;
        let speed = 12.0;
        let dir = Vec2::default();
        let base = BaseEntity::new(pos, scale, speed, 0.0, dir, Some(sprite), EntityKind::Player);
        let ent = Player::new(base, rx, ptx);

        let packet = EntitySpawn {
            id: 0,
            kind: EntityKind::Player,
            pos: Vec2::new(pos.x, pos.y),
            scale,
            speed,
            dir,
        };
        sock.send(packet).unwrap();
        
        self.emplace_entity(Box::new(ent))
    }

    pub fn spawn_projectile(&mut self) -> usize {
        // network this
        unimplemented!()
    }

    pub fn set_position(&mut self, id: i32, pos: Vec2) {
        if let Some((_, e)) = self.entities.iter_mut().find(|(eid, _)| *eid == id) {
            e.set_pos(pos);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        // let removal_list = self.entities
        // .iter_mut()
        // .enumerate()
        // .filter_map(|(idx, (_, e))| {
        //     e.tick(dt).not().then_some(idx)
        // })
        // .collect::<Vec<_>>();
        // for slot in removal_list {
        //     // println!("cleaning spit");
        //     // self.entities.remove(slot);
        //     // TODO:
        //     // fix z ordering so removing entities will work properly
        // }

        // tick all alive entities
        self.entities.iter_mut().filter(|e| e.1.is_alive()).for_each(|e| { e.1.tick(dt); });
    }

    pub fn render(&self, shader: &Shader) {
        self.entities.iter().filter(|e| e.1.is_alive()).for_each(|(_, e)| e.render(shader));
    }
}
