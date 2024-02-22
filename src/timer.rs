use std::time::Duration;


pub struct Timer {
    accumulator: Duration,
    threshold: Duration,
}

impl Timer {
    pub fn new(threshold: Duration) -> Self {
        Self {
            accumulator: Duration::ZERO,
            threshold,
        }
    }

    pub fn reset(&mut self) {
        self.accumulator = Duration::from_secs(0);
    }

    pub fn tick(&mut self, dt: Duration) -> bool {
        self.accumulator += dt;
        let ret = self.accumulator >= self.threshold;

        if ret {
            self.accumulator -= self.threshold;
        }

        ret
    }
}

pub struct Cooldown {
    accumulator: Duration,
    duration: Duration,
}
impl Cooldown {
    pub fn new(duration: Duration) -> Self {
        Self {
            accumulator: Duration::ZERO,
            duration,
        }
    }

    /// reset the cooldown
    pub fn reset(&mut self) {
        self.accumulator = Duration::from_secs(0);
    }

    /// start the cooldown
    pub fn enable(&mut self) {
        self.accumulator = self.duration;
    }

    /// advances the cooldown
    /// returns false if the cooldown is still active
    ///         true if the cooldown is over
    pub fn tick(&mut self, dt: Duration) -> bool {
        self.accumulator = self.accumulator.saturating_sub(dt);
        self.accumulator == Duration::ZERO
    }
}

