// Server
// Hosts a game and allows clients to conenct
// The most basic functions the server should supply
// The flow diagram goes inbound(1,2,3) then outbound(3,2,1)
/* | INBOUND                     | OUTBOUND
-------------------------------------------
 1 | packets -> logic            | logic -> packets
 2 | logic -> simulate world     |
*/

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use engine_2d::math::Vec2;
use engine_2d::time::Timer;
use rand::thread_rng;
use rand::Rng;

use crate::common::EntityDestroy;
use crate::common::EntityKind;
use crate::common::EntitySpawn;
use crate::common::EntityUpdate;
use crate::common::OpCode;
use crate::common::SpriteName;
use crate::common::TIMEOUT;
use crate::entities;
use crate::entities::WORLD_SIZE;
use crate::socket;
use crate::socket::Error;
use crate::socket::NoData;
use crate::socket::Packet;
use crate::socket::Server;

fn broadcast(
    packet: Packet,
    socket: &Server,
    but: Option<SocketAddr>,
    clients: impl Iterator<Item = SocketAddr>,
) {
    let but = but.unwrap_or((Ipv4Addr::UNSPECIFIED, 0).into());
    clients.filter(|a| a != &but).for_each(|a| {
        socket.send(packet.clone(), a).unwrap();
    })
}

fn read_packet_and_update_world(
    socket: &Server,
    rx: &Receiver<(Packet, SocketAddr)>,
    clients: &mut HashMap<SocketAddr, Timer>,
    ents: &mut entities::EntityManager,
    player_ids: &mut HashMap<SocketAddr, i32>,
) {
    if let Ok((p, address)) = rx.try_recv() {
        // println!("server-process");
        match clients.get_mut(&address) {
            Some(timer) => timer.reset(),
            None => {
                println!("new client joined! {}", address);
                let now = Timer::new(TIMEOUT);
                clients.insert(address, now);
                // new client / timed out client reconnect
                // broadcast all entities rn

                for (id, e) in ents.iter() {
                    let p = EntitySpawn {
                        id,
                        pos: e.pos(),
                        dir: e.dir(),
                        scale: e.scale(),
                        kind: e.kind(),
                        speed: e.speed(),
                    };

                    // println!("Server: EntitySpawn {:?}", p);
                    socket.send(p, address).unwrap();
                }
            }
        }

        if socket::OpCode::Pong == p.opcode() {
            clients.get_mut(&address).unwrap().reset();
            // println!("server - pong ({})", address);
        } else {
            match p.opcode() {
                OpCode::EntitySpawn => {
                    let mut e = EntitySpawn::try_from(p).unwrap();
                    let id = ents.spawn(
                        e.pos,
                        e.scale,
                        e.speed,
                        0.0,
                        e.dir,
                        SpriteName::None,
                        e.kind,
                    );
                    e.id = id;

                    if e.kind == EntityKind::Player {
                        player_ids.insert(address, id);
                        player_ids[&address];
                    }

                    broadcast(e.into(), &socket, Some(address), clients.keys().copied());
                }
                OpCode::EntityUpdate => {
                    let mut e = EntityUpdate::try_from(p).unwrap();
                    if e.id == 0 {
                        // player update
                        // fetch the player id
                        e.id = player_ids[&address];
                    }
                    ents.set_position(e.id, e.pos);

                    broadcast(e.into(), &socket, Some(address), clients.keys().copied());
                }
                OpCode::EntityDestroy => {
                    println!("server: entity destroy");
                    let mut e = EntityDestroy::try_from(p).unwrap();
                    if e.id == 0 {
                        // player update
                        // fetch the player id
                        e.id = player_ids[&address];
                    }
                    ents.destroy(e.id);

                    broadcast(e.into(), &socket, Some(address), clients.keys().copied());
                }
            }
        }
    }
}

fn tick(
    ents: &mut entities::EntityManager,
    clients: &mut HashMap<SocketAddr, Timer>,
    player_ids: &mut HashMap<SocketAddr, i32>,
    socket: &Server,
    dt: Duration,
) {
    let mut purge_list = Vec::new();
    for (address, timer) in clients.iter_mut() {
        if timer.tick(dt) {
            purge_list.push(*address);
        }
    }

    for address in purge_list {
        println!("purging client: {}", address);
        for (k, v) in player_ids.iter() {
            println!("player_ids: [{}]=>[{}]", k, v);
        }
        clients.remove(&address);
        if let Some(id) = player_ids.remove(&address) {
            broadcast(
                EntityDestroy { id }.into(),
                &socket,
                None,
                clients.keys().copied(),
            );
            println!("Purging client [ent={}]- {}", id, address);
        }
    }

    ents.tick(dt.as_secs_f32());

    let mut hunter_purge_list = Vec::new();
    for (id, h) in ents.iter().filter(|e| e.1.kind() == EntityKind::Enemy) {
        let d = h.pos().len2();
        if d < 1.0 {
            hunter_purge_list.push(id);
        }
    }

    for id in hunter_purge_list {
        ents.destroy(id);
        broadcast(
            EntityDestroy { id }.into(),
            socket,
            None,
            clients.keys().copied(),
        );
    }
}

fn recv_loop(socket: Arc<Server>, tx: Sender<(Packet, SocketAddr)>) {
    loop {
        match socket.recv() {
            Ok(msg) => {
                // println!("server-get");
                tx.send(msg).unwrap();
            }
            Err(Error::IoError(e)) if e.kind() == ErrorKind::ConnectionReset => (),
            Err(e) => panic!("server - recv_loop error: {:?}", e),
        }
    }
}

fn make_forest(ents: &mut entities::EntityManager) {
    let num_trees = 5;
    let mut rng = thread_rng();

    for _ in 0..num_trees {
        let spread = 12.0;
        let x = rng.gen_range((-spread)..=spread);
        let y = rng.gen_range((-spread)..=spread);
        ents.spawn(
            Vec2::new(x, y),
            8.0,
            0.0,
            0.0,
            Vec2::default(),
            SpriteName::None,
            EntityKind::Forest,
        );
    }
}

fn spawn_hunter(
    ents: &mut entities::EntityManager,
    socket: &Server,
    clients: impl Iterator<Item = SocketAddr>,
) {
    let mut rng = thread_rng();
    let bound = WORLD_SIZE as f32;
    let x = rng.gen_range(-bound..bound);
    let y = rng.gen_range(-bound..bound);
    let pos = Vec2::new(x, y);
    let scale = 5.25;
    let speed = 24.0;
    let dir = Vec2::default() - pos;
    let rotation = dir.angle();
    let kind = EntityKind::Enemy;

    let id = ents.spawn(pos, scale, speed, rotation, dir, SpriteName::None, kind);
    let packet = EntitySpawn {
        id,
        kind,
        pos,
        scale,
        speed,
        dir,
    };

    broadcast(packet.into(), socket, None, clients);
}

pub fn run(port: u16) {
    let mut ents = entities::EntityManager::default();
    let mut player_ids = HashMap::new();
    let mut clients = HashMap::new();
    let socket = Arc::new(Server::listen(port).unwrap());
    let send_socket = socket.clone();

    let (tx, rx) = mpsc::channel();

    make_forest(&mut ents);

    thread::spawn(move || recv_loop(send_socket, tx));

    let mut last = Instant::now();
    let mut ping_timer = Timer::new(Duration::from_secs(1));
    let mut hunter_timer = Timer::new(Duration::from_millis(500));
    loop {
        read_packet_and_update_world(&socket, &rx, &mut clients, &mut ents, &mut player_ids);

        let now = Instant::now();
        let dt = now - last;
        tick(&mut ents, &mut clients, &mut player_ids, &socket, dt);
        last = now;

        if ping_timer.tick(dt) {
            let ping = Packet::new(socket::OpCode::Ping, NoData);
            // println!("server - ping");
            broadcast(ping, &socket, None, clients.keys().copied());
        }
        if hunter_timer.tick(dt) {
            spawn_hunter(&mut ents, &socket, clients.keys().copied());
        }
    }
}
