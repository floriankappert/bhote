# Mux protocol (`bhoted mux`)

Decision (M0): terminal state is the `vt100` crate (0.18). It passed the replay tests in `daemon/src/mux/replay.rs` on `tests/fixtures/mux/session1.raw` (colours, truecolor, wide characters, scroll region, alt screen, any chunking, diff round trip) and has scrollback, mouse/paste modes and `contents_diff`. `alacritty_terminal` has no serialisation of the screen, so a client would need its own differ. Libc calls (`openpty`, `ioctl`, `poll`) go through the `libc` crate; no signal crate (a self-pipe, as `term.rs`).

## Socket

`$XDG_RUNTIME_DIR/bhote/mux.sock` (fallback `/tmp/bhote-mux-<uid>/mux.sock`, mode 0700 dir). `BHOTE_MUX_SOCK` overrides. The server starts on demand (`bhote attach` forks `bhoted mux` when nothing listens) and outlives every client.

## Control connection

Newline-delimited JSON, one request per line, one response per line, like herdr:

    -> {"id":"1","method":"pane.list","params":{}}
    <- {"id":"1","result":{"panes":[...]}}
    <- {"id":"1","error":{"code":"not_found","message":"no pane p9"}}

Methods mirror herdr's names and result shapes (M2): `ping`, `pane.list/layout/split/swap/resize/rename/run/close/send_text/read`, `agent.list/focus/prompt/read`, `workspace.list/create/focus/close`, `tab.*`, `server.stop`.

## Attach connection

`{"method":"attach","params":{"cols":120,"rows":40}}` is answered with `{"result":{"attached":true}}`; from then on the connection carries frames:

    frame = type:u8, len:u32 (big endian), payload[len]

client to server: `I` input bytes exactly as read from the terminal (the server parses keys, mouse and the prefix key), `R` resize (cols:u16, rows:u16 big endian), `D` detach.
server to client: `O` bytes to write to the terminal (the first one is a full redraw, then deltas), `Q` the server ends the attach (payload: reason text, e.g. `detached`, `exited`).

The server composes the whole screen (panes, borders, panels) in one `vt100` state per client and sends `contents_diff`; the client is thin (raw mode, copy bytes, resize). Several clients may attach; each has its own size, the layout uses the smallest.

## Sessions (M7)

`$XDG_STATE_HOME/bhote/mux-session.json`: tree, cwd, labels, resume command per pane; written on change (debounced), read on a cold start.
