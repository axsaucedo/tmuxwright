// Manual validation for b2-session-mgr.
//
// Validates: creating an isolated tmux session on a unique socket,
// reading back its properties, issuing a tmux subcommand against it
// (list-panes), and cleaning it up on Drop.
//
// Run: cargo run -p tmuxwright-tmux --example b2_session

use tmuxwright_tmux::{detect, Session, SessionOptions};

fn main() {
    let tmux = detect().expect("tmux must be installed to run this example");
    println!("using tmux {} at {}", tmux.version(), tmux.path().display());

    let opts = SessionOptions {
        width: 80,
        height: 24,
        command: vec!["cat".into()],
    };
    let session = Session::create(tmux, &opts).expect("create session");
    println!(
        "created socket={} session={}",
        session.socket(),
        session.name()
    );
    println!("primary pane target: {}", session.primary_pane_target());
    println!("reconnect: {}", session.reconnect_command());

    let out = session
        .tmux_cmd(&[
            "list-panes",
            "-t",
            session.name(),
            "-F",
            "#{pane_id} #{pane_width}x#{pane_height}",
        ])
        .expect("list-panes");
    print!(
        "list-panes output:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );

    println!("dropping session (should kill server)...");
    drop(session);
    println!("done");
}
