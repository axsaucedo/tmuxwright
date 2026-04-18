// Manual validation for d5-errors.
// Run: cargo run -p tmuxwright-core --example d5_errors

#![allow(clippy::result_large_err)]

use std::time::Duration;

use tmuxwright_core::{Action, EngineError, EngineResult, Key, Preservation, WaitCondition};

fn simulated_wait() -> EngineResult<()> {
    Err(EngineError::WaitTimeout {
        condition: WaitCondition::Text {
            needle: "READY".into(),
            case_insensitive: false,
        },
        waited: Duration::from_secs(5),
        preservation: Some(Preservation::new("tmuxwright-abc123", "run-42")),
    })
}

fn simulated_dispatch() -> EngineResult<()> {
    let source: Box<dyn std::error::Error + Send + Sync> = "send-keys exited with status 1".into();
    Err(EngineError::Dispatch {
        action: Action::Press(Key::Enter),
        source,
        preservation: Some(Preservation::new("tmuxwright-abc123", "run-42")),
    })
}

fn main() {
    for (label, result) in [
        ("wait", simulated_wait()),
        ("dispatch", simulated_dispatch()),
    ] {
        match result {
            Ok(()) => unreachable!(),
            Err(e) => {
                println!("-- {label} --");
                println!("  kind    = {}", e.kind());
                println!("  display = {e}");
                if let Some(p) = e.preservation() {
                    println!("  reconnect = {}", p.reconnect_cmd);
                }
                if let Some(src) = std::error::Error::source(&e) {
                    println!("  source = {src}");
                }
            }
        }
    }

    let attached = EngineError::AssertFailed {
        description: "expected 'hello'".into(),
        preservation: None,
    }
    .with_preservation(Preservation::new("sock", "sess"));
    println!("\nwith_preservation: {attached}");
}
