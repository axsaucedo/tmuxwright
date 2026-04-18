//! Tmuxwright adapter RPC.
//!
//! Defines the JSON-RPC 2.0 message shapes, capability schema, framing,
//! and the stdio + Unix-domain-socket transports that framework
//! adapters (Textual, Bubble Tea, Ratatui) use to talk to the engine.
//!
//! Implementation lands incrementally per `plan.md` workstream E.
