// Manual validation for e2-schema.
// Run: cargo run -p tmuxwright-rpc --example e2_schema

use tmuxwright_rpc::schema::{
    method, ActionKind, ActionParams, Capability, HandshakeParams, HandshakeResult, LocateParams,
    LocateResult, NodeRef, RegionWire, SelectorWire, PROTOCOL_VERSION,
};
use tmuxwright_rpc::{Id, Request, Response};

fn show<T: serde::Serialize>(tag: &str, v: &T) {
    println!("{tag}: {}", serde_json::to_string(v).unwrap());
}

fn main() {
    println!("protocol: {PROTOCOL_VERSION}");

    let hp = HandshakeParams {
        client: "tmuxwright".into(),
        client_version: "0.0.0".into(),
        protocol: PROTOCOL_VERSION.into(),
    };
    let req = Request::new(
        Id::Num(1),
        method::HANDSHAKE,
        Some(serde_json::to_value(&hp).unwrap()),
    );
    show("handshake req ", &req);

    let hr = HandshakeResult {
        name: "textual".into(),
        version: "0.1.0".into(),
        protocol: PROTOCOL_VERSION.into(),
        capabilities: vec![
            Capability::KeyInput,
            Capability::MouseInput,
            Capability::WidgetTree,
            Capability::Focus,
        ],
    };
    show(
        "handshake resp",
        &Response::ok(Id::Num(1), serde_json::to_value(&hr).unwrap()),
    );

    let loc = LocateParams {
        selector: SelectorWire::Role {
            role: "button".into(),
            name: Some("Save".into()),
        },
    };
    show("locate req    ", &loc);

    let locres = LocateResult {
        nodes: vec![NodeRef {
            node_id: "btn-save".into(),
            region: Some(RegionWire {
                x: 10,
                y: 5,
                w: 8,
                h: 1,
            }),
            role: Some("button".into()),
            name: Some("Save".into()),
        }],
    };
    show("locate resp   ", &locres);

    let act = ActionParams {
        node_id: "btn-save".into(),
        action: ActionKind::Click,
    };
    show("action click  ", &act);

    let act = ActionParams {
        node_id: "input-1".into(),
        action: ActionKind::Type {
            text: "hello".into(),
        },
    };
    show("action type   ", &act);

    let act = ActionParams {
        node_id: "root".into(),
        action: ActionKind::Press {
            chord: "ctrl+s".into(),
        },
    };
    show("action press  ", &act);

    // round-trip one to prove the schema deserializes symmetrically
    let j = serde_json::to_string(&hr).unwrap();
    let back: HandshakeResult = serde_json::from_str(&j).unwrap();
    assert_eq!(back, hr);
    println!("\nroundtrip ok");
}
