// Manual validation for d3-capability.
// Run: cargo run -p tmuxwright-core --example d3_capability

use tmuxwright_core::{Capability, FallbackPolicy, Handshake, Negotiated, Route};

fn print_routes(label: &str, n: &Negotiated) {
    println!("== {label} ==");
    for cap in Capability::all() {
        println!("  {cap:<20} -> {:?}", n.route(*cap));
    }
}

fn main() {
    let textual = Handshake {
        name: "textual".into(),
        version: "0.1.0".into(),
        protocol: "1".into(),
        capabilities: vec![
            Capability::KeyInput,
            Capability::MouseInput,
            Capability::WidgetTree,
            Capability::Focus,
            Capability::SemanticSnapshot,
        ],
    };
    let ratatui_thin = Handshake {
        name: "ratatui".into(),
        version: "0.1.0".into(),
        protocol: "1".into(),
        capabilities: vec![Capability::SemanticSnapshot],
    };

    print_routes("terminal-only (no adapter)", &Negotiated::terminal_only());
    print_routes(
        "textual + prefer_adapter",
        &Negotiated::with_adapter(textual.clone(), FallbackPolicy::PreferAdapter),
    );
    print_routes(
        "ratatui-thin + prefer_adapter (inputs fall back to terminal)",
        &Negotiated::with_adapter(ratatui_thin.clone(), FallbackPolicy::PreferAdapter),
    );
    print_routes(
        "ratatui-thin + adapter_only (inputs unavailable)",
        &Negotiated::with_adapter(ratatui_thin, FallbackPolicy::AdapterOnly),
    );
    print_routes(
        "textual + force_terminal (semantics unavailable by policy)",
        &Negotiated::with_adapter(textual, FallbackPolicy::ForceTerminal),
    );

    // Sanity: adapter-only without adapter is never routable.
    let never = Negotiated {
        handshake: None,
        policy: FallbackPolicy::AdapterOnly,
    };
    assert_eq!(never.route(Capability::KeyInput), Route::Unavailable);
    println!("\nadapter_only + no handshake = all Unavailable: ok");
}
