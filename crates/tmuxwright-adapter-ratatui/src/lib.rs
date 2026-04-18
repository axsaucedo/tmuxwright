//! Ratatui adapter for Tmuxwright.
//!
//! In-process helper that integrates with Ratatui's `TestBackend` for
//! integration-oriented rendering tests and provides explicit semantic
//! region registration so the unified Tmuxwright API can resolve
//! locators by role/name when the application opts in.
//!
//! Implementation lands incrementally per `plan.md` workstream H3.
