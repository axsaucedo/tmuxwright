// Manual validation for d1-primitives.
// Run: cargo run -p tmuxwright-core --example d1_primitives

use tmuxwright_core::{
    Action, ChordKey, Driver, DriverError, Key, Modifiers, MouseButton, Point, Snapshot,
};

struct RecordingDriver {
    log: Vec<String>,
    frames: Vec<Snapshot>,
    idx: usize,
}

impl Driver for RecordingDriver {
    fn dispatch(&mut self, action: &Action) -> Result<(), DriverError> {
        self.log.push(format!("{action:?}"));
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot, DriverError> {
        let s = self.frames[self.idx].clone();
        self.idx = (self.idx + 1).min(self.frames.len() - 1);
        Ok(s)
    }
}

fn main() {
    let mut d = RecordingDriver {
        log: Vec::new(),
        frames: vec![
            Snapshot::from_plain(20, 1, "prompt> "),
            Snapshot::from_plain(20, 1, "prompt> hi"),
            Snapshot::from_plain(20, 1, "hello!              "),
        ],
        idx: 0,
    };

    let script = vec![
        Action::Type("hi".into()),
        Action::Press(Key::Enter),
        Action::Chord {
            mods: Modifiers::CTRL,
            key: ChordKey::Char('l'),
        },
        Action::Click {
            at: Point { x: 3, y: 0 },
            button: MouseButton::Left,
        },
        Action::Resize {
            width: 80,
            height: 24,
        },
    ];

    for a in &script {
        d.dispatch(a).unwrap();
        let s = d.snapshot().unwrap();
        println!(
            "after {a:?}\n  hash={}  raw={:?}",
            s.hash.short(),
            s.raw.trim_end()
        );
    }

    println!("\nlog:");
    for l in &d.log {
        println!("  {l}");
    }

    let a = Snapshot::from_plain(10, 1, "same");
    let b = Snapshot::from_plain(10, 1, "same");
    assert_eq!(a.hash, b.hash);
    println!("\nhash determinism confirmed");
}
