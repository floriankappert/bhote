//! The native mux of bhote (plan: docs/plan-native-mux.md): pane server, client, protocol.
#[cfg(test)] mod replay;

use std::num::NonZeroU16;
/// A terminal state of the given size (at least 1x1) with scrollback lines.
pub fn parser(rows: u16, cols: u16, scrollback: usize) -> vt100::Parser {
    vt100::Parser::new(NonZeroU16::new(rows.max(1)).unwrap(), NonZeroU16::new(cols.max(1)).unwrap(), scrollback)
}

pub mod proto;
pub mod pty;
pub mod render;
pub mod pane;
pub mod server;
pub mod client;
pub mod layout;
pub mod world;
pub mod api;
pub mod herdrcli;
pub mod input;
pub mod keymap;
pub mod actions;
