#[derive(Debug)]
pub enum UsageStrategy {
    KeepAlive,
    Slow,
    Fast,
}

#[derive(Debug)]
pub struct UsageCounter {
    keep_alive: u32,
    slow: u32,
    fast: u32,
}

impl UsageCounter {
    pub fn new() -> Self {
        UsageCounter {
            keep_alive: 0,
            slow: 0,
            fast: 0,
        }
    }

    pub fn increment(&mut self, usage: UsageStrategy) {
        match usage {
            UsageStrategy::KeepAlive => self.keep_alive += 1,
            UsageStrategy::Slow => self.slow += 1,
            UsageStrategy::Fast => self.fast += 1,
        }
    }

    pub fn decrement(&mut self, usage: UsageStrategy) {
        match usage {
            UsageStrategy::KeepAlive => self.keep_alive -= 1,
            UsageStrategy::Slow => self.slow -= 1,
            UsageStrategy::Fast => self.fast -= 1,
        }
    }

    pub fn best_usage(&self) -> Option<UsageStrategy> {
        if self.fast > 0 {
            Some(UsageStrategy::Fast)
        } else if self.slow > 0 {
            Some(UsageStrategy::Slow)
        } else if self.keep_alive > 0 {
            Some(UsageStrategy::KeepAlive)
        } else {
            None
        }
    }
}
