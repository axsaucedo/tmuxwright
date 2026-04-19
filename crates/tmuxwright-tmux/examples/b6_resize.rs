// Manual validation for b6-resize.
// Run: cargo run -p tmuxwright-tmux --example b6_resize

use tmuxwright_tmux::{detect, pane_geometry, Session, SessionOptions};

fn main() {
    let tmux = detect().expect("tmux");
    let s = Session::create(
        tmux,
        &SessionOptions {
            width: 80,
            height: 24,
            command: vec!["cat".into()],
        },
    )
    .expect("session");

    let g0 = pane_geometry(&s).expect("geom0");
    println!("initial: {g0:?}");

    s.resize(120, 40).expect("resize to 120x40");
    let g1 = pane_geometry(&s).expect("geom1");
    println!("after 120x40: {g1:?}");
    // Status line consumes one row; pane height is window height - 1.
    assert_eq!(g1.width, 120);
    assert_eq!(g1.height, 39);

    s.resize(60, 20).expect("shrink");
    let g2 = pane_geometry(&s).expect("geom2");
    println!("after 60x20: {g2:?}");
    assert_eq!(g2.width, 60);
    assert_eq!(g2.height, 19);

    match s.resize(1, 1) {
        Err(e) => println!("1x1 correctly refused: {e}"),
        Ok(()) => panic!("1x1 resize should have failed"),
    }

    println!("\ndone");
}
