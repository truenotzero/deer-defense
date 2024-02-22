use std::time::Duration;

use engine_2d::math::Vec2;

use crate::socket;
use crate::socket::Error;
use crate::socket::Packet;
use crate::socket::Result;

pub const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum SpriteName {
    None,
    Tile,
    Forest,
    Deer,
    Spit,
    Hunter,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum OpCode {
    EntitySpawn = socket::OpCode::UserDefined as _,
    EntityUpdate,
    EntityDestroy,
}

impl From<u8> for OpCode {
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl From<OpCode> for u8 {
    fn from(value: OpCode) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntityKind {
    Tile,
    Forest,
    Player,
    PlayerProjectile,
    Enemy,
}

#[derive(Debug, Clone, Copy)]
pub struct EntitySpawn {
    pub id: i32,
    pub kind: EntityKind,
    pub pos: Vec2,
    pub scale: f32,
    pub speed: f32,
    pub dir: Vec2,
}

impl TryFrom<Packet> for EntitySpawn {
    type Error = Error;
    fn try_from(value: Packet) -> Result<Self> {
        if OpCode::EntitySpawn != value.opcode() {
            Err(Error::BadOpcode)
        } else {
            let data = value.data();
            let id = i32::from_be_bytes(data[0..4].try_into().unwrap());
            let kind = unsafe { std::mem::transmute(data[4]) };
            let x = f32::from_be_bytes(data[5..9].try_into().unwrap());
            let y = f32::from_be_bytes(data[9..13].try_into().unwrap());
            let scale = f32::from_be_bytes(data[13..17].try_into().unwrap());
            let speed = f32::from_be_bytes(data[17..21].try_into().unwrap());
            let dx = f32::from_be_bytes(data[21..25].try_into().unwrap());
            let dy = f32::from_be_bytes(data[25..29].try_into().unwrap());
            Ok(Self {
                id,
                kind,
                pos: Vec2::new(x, y),
                scale,
                speed,
                dir: Vec2::new(dx, dy),
            })
        }
    }
}

impl From<EntitySpawn> for Packet {
    fn from(value: EntitySpawn) -> Self {
        let mut data = Vec::new();
        data.extend_from_slice(&value.id.to_be_bytes());
        data.extend_from_slice(&(value.kind as u8).to_be_bytes());
        data.extend_from_slice(&value.pos.x.to_be_bytes());
        data.extend_from_slice(&value.pos.y.to_be_bytes());
        data.extend_from_slice(&value.scale.to_be_bytes());
        data.extend_from_slice(&value.speed.to_be_bytes());
        data.extend_from_slice(&value.dir.x.to_be_bytes());
        data.extend_from_slice(&value.dir.y.to_be_bytes());
        Self {
            opcode: OpCode::EntitySpawn as u8 as _,
            data,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EntityUpdate {
    pub id: i32,
    pub pos: Vec2,
}

impl TryFrom<Packet> for EntityUpdate {
    type Error = Error;
    fn try_from(value: Packet) -> Result<Self> {
        if OpCode::EntityUpdate != value.opcode() {
            Err(Error::BadOpcode)
        } else {
            let data = value.data();
            let id = i32::from_be_bytes(data[0..4].try_into().unwrap());
            let x = f32::from_be_bytes(data[4..8].try_into().unwrap());
            let y = f32::from_be_bytes(data[8..12].try_into().unwrap());
            Ok(Self {
                id,
                pos: Vec2::new(x, y),
            })
        }
    }
}

impl From<EntityUpdate> for Packet {
    fn from(value: EntityUpdate) -> Self {
        let mut data = Vec::new();
        data.extend_from_slice(&value.id.to_be_bytes());
        data.extend_from_slice(&value.pos.x.to_be_bytes());
        data.extend_from_slice(&value.pos.y.to_be_bytes());
        Self {
            opcode: OpCode::EntityUpdate as u8 as _,
            data,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EntityDestroy {
    pub id: i32,
}

impl TryFrom<Packet> for EntityDestroy {
    type Error = Error;
    fn try_from(value: Packet) -> Result<Self> {
        if OpCode::EntityDestroy != value.opcode() {
            Err(Error::BadOpcode)
        } else {
            let data = value.data();
            let id = i32::from_be_bytes(data[0..4].try_into().unwrap());
            Ok(Self { id })
        }
    }
}

impl From<EntityDestroy> for Packet {
    fn from(value: EntityDestroy) -> Self {
        let mut data = Vec::new();
        data.extend_from_slice(&value.id.to_be_bytes());
        Self {
            opcode: OpCode::EntityDestroy as u8 as _,
            data,
        }
    }
}
