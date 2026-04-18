// Manual validation for c3-stability.
// Exercises the detector with a fake clock and a sequence of hashes.
//
// Run: cargo run -p tmuxwright-term --example c3_stability

use std::cell::Cell;
use std::time::{Duration, Instant};

use tmuxwright_term::{Clock, ScreenHash, Stability, StabilityConfig, Status};

struct FakeClock {
    now: Cell<Instant>,
}
impl FakeClock {
    fn new() -> Self {
        Self {
            now: Cell::new(Instant::now()),
        }
    }
    fn advance(&self, d: Duration) {
        self.now.set(self.now.get() + d);
    }
}
impl Clock for &FakeClock {
    fn now(&self) -> Instant {
        self.now.get()
    }
}

fn h(b: u8) -> ScreenHash {
    ScreenHash([b; 32])
}

fn main() {
    let clock = FakeClock::new();
    let mut s = Stability::new(
        &clock,
        StabilityConfig {
            quiet_for: Duration::from_millis(100),
            timeout: Duration::from_millis(800),
        },
    );

    println!("t=0   observe(h=1) -> {:?}", s.observe(h(1)));
    clock.advance(Duration::from_millis(50));
    println!("t=50  observe(h=1) -> {:?}", s.observe(h(1)));
    clock.advance(Duration::from_millis(60));
    println!("t=110 observe(h=1) -> {:?}", s.observe(h(1)));
    clock.advance(Duration::from_millis(30));
    println!("t=140 observe(h=2) -> {:?}  (changed)", s.observe(h(2)));
    clock.advance(Duration::from_millis(110));
    let st = s.observe(h(2));
    println!("t=250 observe(h=2) -> {st:?}");
    assert_eq!(st, Status::Stable);

    // Drive to timeout by flapping the hash forever.
    let mut saw_timeout = false;
    for i in 3..100u8 {
        clock.advance(Duration::from_millis(30));
        if s.observe(h(i)) == Status::Timeout {
            saw_timeout = true;
            println!("timeout observed after flapping at step {i}");
            break;
        }
    }
    assert!(saw_timeout, "expected timeout from flapping hashes");

    println!("\ndone");
}
