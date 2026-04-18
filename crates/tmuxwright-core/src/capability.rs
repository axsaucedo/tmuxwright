//! Adapter capability negotiation.
//!
//! An adapter (Textual / Bubble Tea / Ratatui, landing in workstream H)
//! announces what it can do via a [`Handshake`], and the engine decides
//! per-operation whether to route through the adapter or fall back to
//! the terminal-mode driver.
//!
//! The actual RPC transport lives in `tmuxwright-rpc` (workstream E);
//! this module is pure data + policy so it can be unit-tested without
//! any process boundary.

use std::fmt;

/// Individual things an adapter may support.
///
/// The set is intentionally small in v1. More capabilities will be
/// added as the adapters grow (e.g. `TypeahadBatching`, `Animation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Adapter accepts keyboard input as a semantic action (not as
    /// encoded terminal bytes).
    KeyInput,
    /// Adapter accepts mouse input as a semantic action.
    MouseInput,
    /// Adapter can return a semantic widget/DOM tree.
    WidgetTree,
    /// Adapter can report the currently focused widget.
    Focus,
    /// Adapter can produce its own plain-text rendering of the UI
    /// (distinct from the tmux pane capture).
    SemanticSnapshot,
}

impl Capability {
    /// All known capabilities (stable order for iteration in tests /
    /// trace output).
    #[must_use]
    pub fn all() -> &'static [Capability] {
        &[
            Capability::KeyInput,
            Capability::MouseInput,
            Capability::WidgetTree,
            Capability::Focus,
            Capability::SemanticSnapshot,
        ]
    }

    /// Stable lowercase name used on the wire and in traces.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::KeyInput => "key_input",
            Capability::MouseInput => "mouse_input",
            Capability::WidgetTree => "widget_tree",
            Capability::Focus => "focus",
            Capability::SemanticSnapshot => "semantic_snapshot",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An adapter's response to the initial handshake call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    /// Adapter implementation name, e.g. "textual", "bubbletea",
    /// "ratatui".
    pub name: String,
    /// Semver of the adapter.
    pub version: String,
    /// Semver of the RPC protocol it speaks.
    pub protocol: String,
    /// Capabilities this adapter supports.
    pub capabilities: Vec<Capability>,
}

impl Handshake {
    #[must_use]
    pub fn supports(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
}

/// What the engine decided for a requested capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Adapter will handle this operation.
    Adapter,
    /// Fall back to the terminal-mode driver.
    Terminal,
    /// Neither backend can satisfy this capability; the engine must
    /// raise an error on use.
    Unavailable,
}

/// Fallback policy when an adapter is missing a capability. The
/// terminal-mode driver is always present, but not every capability
/// has a sensible terminal-mode analogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackPolicy {
    /// Try the adapter; if unsupported, fall back to terminal when
    /// possible, else Unavailable.
    PreferAdapter,
    /// Always use terminal mode regardless of adapter support.
    ForceTerminal,
    /// Only use the adapter; if unsupported, return Unavailable.
    AdapterOnly,
}

/// Capabilities whose semantics can also be expressed in
/// terminal-mode. Used by `PreferAdapter` fallback to decide whether
/// missing adapter support can degrade gracefully.
fn terminal_can_do(cap: Capability) -> bool {
    matches!(cap, Capability::KeyInput | Capability::MouseInput)
}

/// The engine's negotiated view of an adapter session.
#[derive(Debug, Clone)]
pub struct Negotiated {
    pub handshake: Option<Handshake>,
    pub policy: FallbackPolicy,
}

impl Negotiated {
    #[must_use]
    pub fn terminal_only() -> Self {
        Self {
            handshake: None,
            policy: FallbackPolicy::ForceTerminal,
        }
    }

    #[must_use]
    pub fn with_adapter(handshake: Handshake, policy: FallbackPolicy) -> Self {
        Self {
            handshake: Some(handshake),
            policy,
        }
    }

    /// Decide which backend should serve a capability request.
    #[must_use]
    pub fn route(&self, cap: Capability) -> Route {
        let adapter_has = self.handshake.as_ref().is_some_and(|h| h.supports(cap));
        match self.policy {
            FallbackPolicy::ForceTerminal => {
                if terminal_can_do(cap) {
                    Route::Terminal
                } else {
                    Route::Unavailable
                }
            }
            FallbackPolicy::AdapterOnly => {
                if adapter_has {
                    Route::Adapter
                } else {
                    Route::Unavailable
                }
            }
            FallbackPolicy::PreferAdapter => {
                if adapter_has {
                    Route::Adapter
                } else if terminal_can_do(cap) {
                    Route::Terminal
                } else {
                    Route::Unavailable
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hs(caps: &[Capability]) -> Handshake {
        Handshake {
            name: "textual".into(),
            version: "0.1.0".into(),
            protocol: "1".into(),
            capabilities: caps.to_vec(),
        }
    }

    #[test]
    fn capability_names_are_stable() {
        assert_eq!(Capability::KeyInput.as_str(), "key_input");
        assert_eq!(Capability::MouseInput.as_str(), "mouse_input");
        assert_eq!(Capability::WidgetTree.as_str(), "widget_tree");
        assert_eq!(Capability::Focus.as_str(), "focus");
        assert_eq!(Capability::SemanticSnapshot.as_str(), "semantic_snapshot");
        assert_eq!(Capability::all().len(), 5);
    }

    #[test]
    fn handshake_supports_reflects_list() {
        let h = hs(&[Capability::KeyInput, Capability::WidgetTree]);
        assert!(h.supports(Capability::KeyInput));
        assert!(h.supports(Capability::WidgetTree));
        assert!(!h.supports(Capability::Focus));
    }

    #[test]
    fn terminal_only_routes_input_to_terminal_and_semantics_unavailable() {
        let n = Negotiated::terminal_only();
        assert_eq!(n.route(Capability::KeyInput), Route::Terminal);
        assert_eq!(n.route(Capability::MouseInput), Route::Terminal);
        assert_eq!(n.route(Capability::WidgetTree), Route::Unavailable);
        assert_eq!(n.route(Capability::Focus), Route::Unavailable);
        assert_eq!(n.route(Capability::SemanticSnapshot), Route::Unavailable);
    }

    #[test]
    fn prefer_adapter_uses_adapter_when_supported() {
        let h = hs(&[Capability::WidgetTree, Capability::KeyInput]);
        let n = Negotiated::with_adapter(h, FallbackPolicy::PreferAdapter);
        assert_eq!(n.route(Capability::WidgetTree), Route::Adapter);
        assert_eq!(n.route(Capability::KeyInput), Route::Adapter);
    }

    #[test]
    fn prefer_adapter_falls_back_to_terminal_for_missing_inputs() {
        let h = hs(&[Capability::WidgetTree]);
        let n = Negotiated::with_adapter(h, FallbackPolicy::PreferAdapter);
        assert_eq!(n.route(Capability::KeyInput), Route::Terminal);
        assert_eq!(n.route(Capability::MouseInput), Route::Terminal);
    }

    #[test]
    fn prefer_adapter_unavailable_for_missing_semantics() {
        let h = hs(&[Capability::KeyInput]);
        let n = Negotiated::with_adapter(h, FallbackPolicy::PreferAdapter);
        assert_eq!(n.route(Capability::WidgetTree), Route::Unavailable);
        assert_eq!(n.route(Capability::Focus), Route::Unavailable);
    }

    #[test]
    fn adapter_only_refuses_to_fall_back() {
        let h = hs(&[Capability::WidgetTree]);
        let n = Negotiated::with_adapter(h, FallbackPolicy::AdapterOnly);
        assert_eq!(n.route(Capability::WidgetTree), Route::Adapter);
        assert_eq!(n.route(Capability::KeyInput), Route::Unavailable);
    }

    #[test]
    fn force_terminal_ignores_adapter_capabilities() {
        let h = hs(&[
            Capability::KeyInput,
            Capability::MouseInput,
            Capability::WidgetTree,
        ]);
        let n = Negotiated::with_adapter(h, FallbackPolicy::ForceTerminal);
        assert_eq!(n.route(Capability::KeyInput), Route::Terminal);
        assert_eq!(n.route(Capability::WidgetTree), Route::Unavailable);
    }
}
