pub enum Usage {
    KeepAlive,
    Slow,
    Fast,
}

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

    pub fn increment(&mut self, usage: Usage) {
        match usage {
            Usage::KeepAlive => self.keep_alive += 1,
            Usage::Slow => self.slow += 1,
            Usage::Fast => self.fast += 1,
        }
    }

    pub fn decrement(&mut self, usage: Usage) {
        match usage {
            Usage::KeepAlive => self.keep_alive -= 1,
            Usage::Slow => self.slow -= 1,
            Usage::Fast => self.fast -= 1,
        }
    }

    pub fn best_usage(&self) -> Option<Usage> {
        if self.fast > 0 {
            Some(Usage::Fast)
        } else if self.slow > 0 {
            Some(Usage::Slow)
        } else if self.keep_alive > 0 {
            Some(Usage::KeepAlive)
        } else {
            None
        }
    }
}
