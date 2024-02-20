use std::cell::RefCell;
use std::collections::HashMap;

use engine_2d::math::Vec2;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub enum Type {
    Move,
}

pub enum Data {
    Move(Vec2),
}

#[derive(Default)]
pub struct EventManager {
    subscribers: HashMap<Type, Vec<fn(&Data)>>,
}

impl<'a> EventManager {
    pub fn subscribe(&mut self, e: Type, f: fn(&Data)) {
        if let None = self.subscribers.get(&e) {
            self.subscribers.insert(e, Vec::new());
        }

        self.subscribers.get_mut(&e).unwrap().push(f);
    }

    pub fn submit(&self, e: Type, data: &Data) -> Option<()> {
        for s in self.subscribers.get(&e)? {
            s(data);
        }
        Some(())
    }
}

pub fn subscribe(e: Type, f: fn(&Data)) {
    EVENT_MANAGER.with_borrow_mut(|m| m.subscribe(e, f))
}


pub fn submit(e: Type, data: Data) -> Option<()> {
    EVENT_MANAGER.with_borrow(|m| m.submit(e, &data))
}

thread_local! {
    static EVENT_MANAGER: RefCell<EventManager> = RefCell::new(EventManager::default());
}