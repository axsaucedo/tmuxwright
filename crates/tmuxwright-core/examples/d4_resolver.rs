// Manual validation for d4-resolver.
// Run: cargo run -p tmuxwright-core --example d4_resolver

use tmuxwright_core::{
    resolve, Capability, FallbackPolicy, Handshake, Negotiated, NullSemanticBackend, Selector,
    SemanticBackend,
};
use tmuxwright_core::{EngineError, Snapshot};
use tmuxwright_term::{Match, Region};

struct FakeAdapter {
    role_hits: std::collections::HashMap<String, Match>,
}
impl SemanticBackend for FakeAdapter {
    fn query(&mut self, selector: &Selector) -> Result<Option<Match>, EngineError> {
        if let Selector::Role { role, .. } = selector {
            return Ok(self.role_hits.get(role).copied());
        }
        Ok(None)
    }
}

fn main() {
    let snap = Snapshot::from_plain(30, 3, "name: alice\r\n[OK]    [Cancel]\r\n");
    let grid = &snap.grid;

    // Terminal-only: text resolves against grid.
    let n0 = Negotiated::terminal_only();
    let mut b0 = NullSemanticBackend;
    let r = resolve(
        &Selector::Text {
            needle: "alice".into(),
            case_insensitive: false,
            nth: 0,
        },
        grid,
        &n0,
        &mut b0,
    )
    .unwrap();
    println!("terminal text(alice) -> {:?} via {:?}", r.hit.region, r.via);

    // Terminal-only: role selector is a hard miss.
    let err = resolve(
        &Selector::Role {
            role: "button".into(),
            name: Some("OK".into()),
        },
        grid,
        &n0,
        &mut b0,
    )
    .unwrap_err();
    println!("terminal role(button) -> {} / {err}", err.kind());

    // With adapter: role resolves via adapter.
    let adapter = Handshake {
        name: "textual".into(),
        version: "0.1.0".into(),
        protocol: "1".into(),
        capabilities: vec![Capability::WidgetTree, Capability::KeyInput],
    };
    let n1 = Negotiated::with_adapter(adapter, FallbackPolicy::PreferAdapter);
    let mut b1 = FakeAdapter {
        role_hits: [(
            "button".to_string(),
            Match {
                region: Region {
                    x: 0,
                    y: 1,
                    width: 4,
                    height: 1,
                },
            },
        )]
        .into_iter()
        .collect(),
    };
    let r = resolve(
        &Selector::Role {
            role: "button".into(),
            name: Some("OK".into()),
        },
        grid,
        &n1,
        &mut b1,
    )
    .unwrap();
    println!("adapter role(button) -> {:?} via {:?}", r.hit.region, r.via);

    // Text with adapter that has no hit falls back to grid.
    let r = resolve(
        &Selector::Text {
            needle: "Cancel".into(),
            case_insensitive: false,
            nth: 0,
        },
        grid,
        &n1,
        &mut b1,
    )
    .unwrap();
    println!(
        "adapter+text(Cancel) fallback -> {:?} via {:?}",
        r.hit.region, r.via
    );
}
