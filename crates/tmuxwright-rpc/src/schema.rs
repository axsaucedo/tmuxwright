//! Capability schema for the Tmuxwright adapter RPC.
//!
//! Pure data types describing what adapters declare in the handshake
//! and the request/result payloads for the core method set. The engine
//! (workstream F) will wrap these with a client; adapter authors
//! (workstream H) will implement the server side.
//!
//! Protocol version is `PROTOCOL_VERSION`. Backward-incompatible
//! changes bump this; additive changes keep it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wire protocol version. Adapters must echo the same major in
/// `HandshakeResult::protocol` or the engine will refuse to continue.
pub const PROTOCOL_VERSION: &str = "1";

/// Canonical method names. Defined here so client and server agree
/// on the exact strings.
pub mod method {
    pub const HANDSHAKE: &str = "tmw.handshake";
    pub const SHUTDOWN: &str = "tmw.shutdown";

    pub const SNAPSHOT_SEMANTIC: &str = "tmw.snapshot.semantic";
    pub const LOCATE: &str = "tmw.locate";
    pub const ACTION_DISPATCH: &str = "tmw.action";
    pub const STATE_FOCUS: &str = "tmw.state.focus";
    pub const STATE_VALUE: &str = "tmw.state.value";
}

/// Stable wire names for adapter capabilities. Mirrors the engine
/// `Capability` enum but is duplicated here so this crate has no
/// dependency on `tmuxwright-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    KeyInput,
    MouseInput,
    WidgetTree,
    Focus,
    SemanticSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeParams {
    pub client: String,
    pub client_version: String,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeResult {
    pub name: String,
    pub version: String,
    pub protocol: String,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SelectorWire {
    Text { value: String, nth: Option<u32> },
    Region { x: u16, y: u16, w: u16, h: u16 },
    Role { role: String, name: Option<String> },
    TestId { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocateParams {
    pub selector: SelectorWire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocateResult {
    pub nodes: Vec<NodeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRef {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<RegionWire>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionWire {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    Click,
    Focus,
    Press { chord: String },
    Type { text: String },
    Scroll { dx: i32, dy: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionParams {
    pub node_id: String,
    pub action: ActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticSnapshotResult {
    /// Framework-native tree. Deliberately opaque `Value`: each
    /// adapter defines its own shape and the engine surfaces it to
    /// user code via the SDK without interpreting it.
    pub tree: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateValueParams {
    pub node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateValueResult {
    pub value: Value,
}

/// Checks that `adapter_protocol` is a version the engine understands.
/// Protocol is a single integer major as a string ("1", "2", ...).
#[must_use]
pub fn is_compatible(adapter_protocol: &str) -> bool {
    adapter_protocol == PROTOCOL_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_are_stable() {
        assert_eq!(method::HANDSHAKE, "tmw.handshake");
        assert_eq!(method::LOCATE, "tmw.locate");
        assert_eq!(method::ACTION_DISPATCH, "tmw.action");
        assert_eq!(method::SNAPSHOT_SEMANTIC, "tmw.snapshot.semantic");
    }

    #[test]
    fn capability_wire_names_are_snake_case() {
        let j = serde_json::to_string(&vec![
            Capability::KeyInput,
            Capability::WidgetTree,
            Capability::SemanticSnapshot,
        ])
        .unwrap();
        assert_eq!(j, r#"["key_input","widget_tree","semantic_snapshot"]"#);
    }

    #[test]
    fn handshake_roundtrip() {
        let h = HandshakeResult {
            name: "textual".into(),
            version: "0.1.0".into(),
            protocol: PROTOCOL_VERSION.into(),
            capabilities: vec![Capability::KeyInput, Capability::WidgetTree],
        };
        let j = serde_json::to_string(&h).unwrap();
        assert!(j.contains(r#""protocol":"1""#));
        let back: HandshakeResult = serde_json::from_str(&j).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn selector_discriminator_is_kind() {
        let s = SelectorWire::Role {
            role: "button".into(),
            name: Some("Save".into()),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains(r#""kind":"role""#));
        assert!(j.contains(r#""role":"button""#));
        let back: SelectorWire = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn action_kind_carries_payload_shape_per_variant() {
        let a = ActionKind::Press {
            chord: "ctrl+s".into(),
        };
        let j = serde_json::to_string(&a).unwrap();
        assert!(j.contains(r#""kind":"press""#));
        assert!(j.contains(r#""chord":"ctrl+s""#));
        let back: ActionKind = serde_json::from_str(&j).unwrap();
        assert_eq!(back, a);

        let c = ActionKind::Click;
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(j, r#"{"kind":"click"}"#);
    }

    #[test]
    fn locate_result_with_minimal_node() {
        let r = LocateResult {
            nodes: vec![NodeRef {
                node_id: "n1".into(),
                region: None,
                role: None,
                name: None,
            }],
        };
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"nodes":[{"node_id":"n1"}]}"#);
    }

    #[test]
    fn compat_check_accepts_current_and_rejects_other() {
        assert!(is_compatible("1"));
        assert!(!is_compatible("2"));
        assert!(!is_compatible(""));
    }
}
