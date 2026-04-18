// Manual validation for c4-text-locator.
// Run: cargo run -p tmuxwright-term --example c4_locator

use tmuxwright_term::{Parser, RegionLocator, TextLocator};

fn main() {
    let mut p = Parser::new(30, 5);
    p.feed(b"login prompt:\r\n  Username: alice\r\n  Password: ****\r\n  [ OK ]   [Cancel]\r\n");
    let g = p.into_grid();

    println!("== text locator: 'Username' ==");
    let m = TextLocator::new("Username").first(&g).expect("match");
    println!("  region: {:?}  center: {:?}", m.region, m.center());
    assert_eq!(m.region.y, 1);

    println!("\n== text locator: case-insensitive 'PASSWORD' ==");
    let m = TextLocator::new("PASSWORD")
        .case_insensitive()
        .first(&g)
        .expect("match");
    println!("  region: {:?}", m.region);
    assert_eq!(m.region.y, 2);

    println!("\n== text locator: all 'OK' + nth(0) ==");
    let all = TextLocator::new("OK").all(&g);
    println!("  all matches: {all:?}");
    let first = TextLocator::new("OK").nth(0).first(&g).expect("m");
    println!("  click target: {:?}", first.center());

    println!("\n== region locator: rows 0-2 cols 2..18 ==");
    let text = RegionLocator::new(2, 0, 16, 3).text(&g);
    println!("{text}");

    println!("\n== region locator: off-grid returns None ==");
    println!("  {:?}", RegionLocator::new(100, 0, 1, 1).resolve(&g));

    println!("\ndone");
}
