// Manual validation for b5-preserve.
//
// End-to-end failure-recovery flow:
//   1. Create a session.
//   2. Verify is_alive() is true.
//   3. Call preserve() — simulates a test failure handler.
//   4. Drop the handle.
//   5. Confirm the server is still reachable (we couldn't reconnect
//      in a detached process here, but has-session on the same socket
//      is the same check tmux uses).
//   6. Print the reconnect hint and reconnect_command.
//   7. Kill the leaked server manually.
//
// Run: cargo run -p tmuxwright-tmux --example b5_preserve

use std::process::Command;

use tmuxwright_tmux::{detect, Session, SessionOptions};

fn main() {
    let tmux = detect().expect("tmux");
    let opts = SessionOptions {
        width: 80,
        height: 24,
        command: vec!["cat".into()],
    };
    let (socket, session_name, hint) = {
        let mut s = Session::create(tmux.clone(), &opts).expect("create");
        println!("is_alive (before preserve): {}", s.is_alive());
        s.preserve();
        let h = s.reconnect_hint();
        println!("reconnect_hint: {h:?}");
        println!("reconnect_command: {}", s.reconnect_command());
        (s.socket().to_string(), s.name().to_string(), h)
        // s drops here; preserve=true so server should survive
    };

    // Probe the raw socket directly to prove the server outlived Drop.
    let alive = Command::new(tmux.path())
        .args(["-L", &socket, "has-session", "-t", &session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    println!("server alive after drop: {alive}");
    assert!(alive, "preserve() should keep the server running past Drop");

    // Manual cleanup — the developer would normally do this after
    // inspecting the session via the reconnect command.
    let _ = Command::new(tmux.path())
        .args(["-L", &hint.socket, "kill-server"])
        .output();

    let alive_after_kill = Command::new(tmux.path())
        .args(["-L", &socket, "has-session", "-t", &session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    println!("server alive after manual kill: {alive_after_kill}");
    assert!(!alive_after_kill, "kill-server must remove the session");

    println!("\ndone");
}
