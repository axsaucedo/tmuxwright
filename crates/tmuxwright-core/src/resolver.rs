//! Locator resolver.
//!
//! A [`Selector`] is the user-facing description of "where on the UI"
//! (text, absolute region, or a semantic role/name). The resolver
//! decides — per selector kind, using the [`Negotiated`] capabilities
//! from D3 — whether to resolve against the parsed [`Grid`] or hand
//! off to a [`SemanticBackend`] (D3/E wiring).
//!
//! This module does *not* know how to talk to a real adapter; it only
//! knows how to ask `SemanticBackend::query`. Tests inject a mock
//! backend; workstream E will plug the RPC client in behind the same
//! trait.

use tmuxwright_term::{Grid, Match, RegionLocator, TextLocator};

use crate::capability::{Capability, Negotiated, Route};
use crate::error::EngineError;

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Substring match on a row of terminal text.
    Text {
        needle: String,
        case_insensitive: bool,
        nth: usize,
    },
    /// Absolute rectangle in grid coordinates.
    Region {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    /// Semantic role + accessible name. Requires an adapter.
    Role { role: String, name: Option<String> },
}

impl Selector {
    /// Which capability does the adapter need to serve this selector?
    #[must_use]
    pub fn required_capability(&self) -> Capability {
        match self {
            // Text and Region are *also* servable by an adapter that
            // exposes a semantic snapshot, but resolving them against
            // the terminal grid is always correct, so we don't require
            // anything from the adapter. Signal WidgetTree only for
            // semantic selectors.
            Selector::Text { .. } | Selector::Region { .. } => Capability::SemanticSnapshot,
            Selector::Role { .. } => Capability::WidgetTree,
        }
    }

    /// Short stable tag for logs and trace entries.
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Selector::Text { .. } => "text",
            Selector::Region { .. } => "region",
            Selector::Role { .. } => "role",
        }
    }
}

/// Successful resolution outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    /// Region on the grid to operate on.
    pub hit: Match,
    /// Whether the resolution came from the adapter or terminal.
    pub via: Via,
}

/// Where the resolution came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Adapter,
    Terminal,
}

/// Pluggable semantic backend the resolver can query. Production
/// impls wrap the adapter RPC client; tests inject a mock.
pub trait SemanticBackend {
    /// Resolve a selector through the adapter. Returns `None` if the
    /// selector does not match any element.
    ///
    /// # Errors
    /// Returns an `EngineError::Adapter` when the adapter call fails
    /// (transport error, schema mismatch, etc.).
    fn query(&mut self, selector: &Selector) -> Result<Option<Match>, EngineError>;
}

/// A no-op semantic backend used when no adapter is connected. It
/// never returns a hit. Resolver routing in terminal-only mode never
/// calls it, but having a concrete type keeps the API monomorphic.
#[derive(Debug, Default)]
pub struct NullSemanticBackend;

impl SemanticBackend for NullSemanticBackend {
    fn query(&mut self, _selector: &Selector) -> Result<Option<Match>, EngineError> {
        Ok(None)
    }
}

/// Resolve a selector against the current grid + negotiated adapter.
///
/// Routing rules:
///
/// - `Role` always needs the adapter. If routing says Unavailable or
///   Terminal, we fail with `LocatorMiss` (terminal mode can't answer).
/// - `Text` and `Region` prefer the adapter only when routing says so
///   *and* the backend returns a hit. Otherwise we resolve against
///   the grid — which always works.
///
/// # Errors
/// Returns [`EngineError::LocatorMiss`] when the selector matches no
/// element, and propagates [`EngineError::Adapter`] from the backend.
pub fn resolve<B: SemanticBackend>(
    selector: &Selector,
    grid: &Grid,
    negotiated: &Negotiated,
    backend: &mut B,
) -> Result<Resolved, EngineError> {
    let route = negotiated.route(selector.required_capability());

    if matches!(selector, Selector::Role { .. }) {
        return match route {
            Route::Adapter => query_adapter(selector, backend).map(|hit| Resolved {
                hit,
                via: Via::Adapter,
            }),
            Route::Terminal | Route::Unavailable => Err(miss(selector, 0)),
        };
    }

    // Text / Region: try adapter first if routed there; fall back to
    // terminal-mode resolution which is always correct.
    if route == Route::Adapter {
        if let Some(hit) = backend.query(selector)? {
            return Ok(Resolved {
                hit,
                via: Via::Adapter,
            });
        }
    }
    resolve_terminal(selector, grid).map(|hit| Resolved {
        hit,
        via: Via::Terminal,
    })
}

fn query_adapter<B: SemanticBackend>(
    selector: &Selector,
    backend: &mut B,
) -> Result<Match, EngineError> {
    backend.query(selector)?.ok_or_else(|| miss(selector, 0))
}

fn resolve_terminal(selector: &Selector, grid: &Grid) -> Result<Match, EngineError> {
    match selector {
        Selector::Text {
            needle,
            case_insensitive,
            nth,
        } => {
            let mut loc = TextLocator::new(needle.clone()).nth(*nth);
            if *case_insensitive {
                loc = loc.case_insensitive();
            }
            loc.first(grid).ok_or_else(|| miss(selector, 0))
        }
        Selector::Region {
            x,
            y,
            width,
            height,
        } => RegionLocator::new(*x, *y, *width, *height)
            .resolve(grid)
            .ok_or_else(|| miss(selector, 0)),
        Selector::Role { .. } => Err(miss(selector, 0)),
    }
}

fn miss(selector: &Selector, found: usize) -> EngineError {
    EngineError::LocatorMiss {
        selector: format!("{}={selector:?}", selector.tag()),
        found,
        preservation: None,
    }
}
