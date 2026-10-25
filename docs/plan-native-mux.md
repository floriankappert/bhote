# Plan: bhote replaces herdr

Goal: start `bhote` from the console instead of `herdr`. bhote runs the panes itself; herdr is dropped at the end.
Branch: `rust/mux` (started right away, PR #33 stays open in parallel). Status: plan, nothing built.

## What herdr does for bhote today

| Area | herdr gives | bhote needs instead |
|---|---|---|
| Panes | PTYs, splits, tabs, workspaces, drawing, mouse, scrollback, keys, clipboard, kitty graphics | own pane server in the daemon (`bhoted mux`) and a client that draws |
| Sessions | server outlives the terminal, panes restored after restart | detach/attach over a socket, layout snapshot on disk |
| Agent state | working / blocked / idle, tokens (`pane list`, hooks) | state from the agents' hooks (already in `integrations/claude`) plus screen heuristics as a fallback |
| Other machines | herdr-kharka, `herdr.commands` | the same kharka records, with bhote's own commands (`agent.focus`, `agent.prompt`, `workspace.create`, …) |
| Panels | plugin `open_panel.sh` splits panes | the panels become built-in panes (left, right, chat), no plugin |

## Steps

1. **Pane server** (the big one). PTY per pane (`openpty`, `setsid`), a terminal-emulator state per pane (vte/`alacritty_terminal` crate), splits as a tree, resize, scrollback. Unix socket API with the verbs bhote already uses on herdr: `pane list/layout/split/swap/resize/rename/run/close`, `agent focus`, `workspace list/create/focus/close`, `tab …`.
2. **Client**: `bhote` (no arguments, or `bhote attach`) draws the layout in the terminal, forwards keys and mouse, keeps the prefix key (default `ctrl+b`, as in herdr). Detach and re-attach.
3. **Compatibility layer**: bhote talks to a `Mux` trait with two backends, `herdr` (today) and `native`. Panel, cards, workspaces and clicks move over one by one; every suite runs against both.
4. **Sessions**: snapshot of layout, cwd, labels and agent resume commands; restore after a restart (the agents are started again with their resume command).
5. **Other machines**: kharka commands for the native backend; the Mac runs the same server.
6. **Chat panel** (can start in parallel with step 1, it works on herdr too): see below.
7. **Switch**: native becomes the default, herdr stays selectable for one release (`MUX=herdr`), then it goes.

## Chat panel (decided)

A pane with an input field and the agent's history; Enter sends the text to the chosen agent (`agent.prompt`), the agent's last turns are read from its transcript (`agent.read`). The agent keeps running as Claude Code in its own PTY.

## Decisions

- Chat panel: input to the running agents.
- Agents (Claude Code, Codex, …) stay normal programs in a PTY of bhote.
- Keys and config: a new bhote keymap (own file, `~/.config/bhote/keys`), analogous to herdr (prefix `ctrl+b`, the same verbs); herdr's `config.toml` is not read.
- Start on `rust/mux` at once; macOS runs the same binary and server, tested from step 2 on.

## Starting point in the code

- `daemon/` is std-only plus `serde_json` and `ureq`; `term.rs` calls libc by hand (no crates for the terminal). The mux needs a few more dependencies (decide in M0): `libc` (PTY, `ioctl`, `poll`), a terminal-state crate (`vte` for parsing; `alacritty_terminal` or `vt100` for the screen), optionally `signal-hook`.
- The panels (`panel.rs`, `cards.rs`) already draw with escape codes into a pane. In the native client they become widgets drawn into a pane rectangle instead of a separate process.
- Everything that talks to herdr today goes through a few call sites (`herdr pane list`, `agent focus`, `workspace list/create/focus`, `pane split/swap/resize/rename/run`, `agent.prompt` over kharka). M2 puts them behind one trait.

## Milestones (each ends in something that runs and is tested)

**M0 – Decide and prepare (no feature code).** Pick the crates (try `alacritty_terminal` vs `vt100` on a recorded Claude Code session: colours, alt screen, wide characters, scroll regions). Fix the socket protocol (newline-delimited JSON requests like herdr's: `{"id","method","params"}`, plus a binary stream for screen deltas). Write `docs/mux-protocol.md`. Done when: one recorded session replays correctly through the chosen crate in a test.

**M1 – One pane.** `bhoted mux` server: socket in `$XDG_RUNTIME_DIR/bhote/mux.sock`, one PTY with the user's shell, terminal state, resize. `bhote attach` client: raw mode, draws the screen, forwards keys, `ctrl+b d` detaches, attach again shows the same screen. Done when: a pty test starts the server, types `echo hi`, detaches, re-attaches and sees the line; `vim` and `claude` run.

**M2 – Layout and verbs.** Split tree, tabs, workspaces; the herdr verbs on the socket (`pane list/layout/split/swap/resize/rename/run/close`, `workspace list/create/focus/close`, `tab …`) with the same JSON shape as herdr's, so the existing bash and Rust call sites work unchanged against `BHOTE_MUX=native`. A `Mux` trait in the daemon with the `herdr` and `native` backends. Done when: `tests/panel.py` and `tests/cli.sh` pass against both backends.

**M3 – Input.** The bhote keymap (`~/.config/bhote/keys`, analogous to herdr: prefix `ctrl+b`, split, focus, resize mode, tab and workspace keys, copy mode, scrollback), mouse (focus, drag a divider, selection, wheel), bracketed paste, clipboard (OSC 52), focus events. Done when: a key-by-key test table passes and the user can work in it for a day without touching herdr.

**M4 – Panels built in.** The left, right and chat panes are created by the server from the layout (no `open_panel.sh`); widths per side as today (`PANEL_LEFT_WIDTH`, the width groups). Done when: `PANEL_LAYOUT=split` works in the native mux without the plugin.

**M5 – Agents.** Agent state from the Claude/Codex hooks (already writing `evstate`), the screen as a fallback; `agent list/focus/prompt/read` on the socket; cards, clicks and the active highlight 1:1 as today. Done when: the agent suites (`agents`, `frame`, `panel`) pass on the native backend.

**M6 – Chat panel.** A pane with an input field (multi-line, history, `@` to pick the agent) and the chosen agent's last turns from its transcript; Enter sends with `agent.prompt`. Can be built on herdr earlier as a normal panel (it only needs `agent.prompt` and `agent.read`). Done when: a message typed there arrives in the agent's prompt and its answer shows in the history.

**M7 – Sessions.** Snapshot of layout, cwd, labels and the agents' resume commands; restore after a restart; the server survives the terminal. Done when: kill the server, start it again, and the layout and the agents (resumed) are back.

**M8 – Other machines.** The native backend answers the kharka commands (`agent.focus`, `agent.prompt`, `agent.read`, `workspace.create`, …); workspace `+ new` works on the Mac too. macOS: same binary, PTY and signals tested there (`openpty`, `TIOCSWINSZ` differ in constants).

**M9 – Switch.** `bhote` with no arguments starts the native client; `MUX=herdr` keeps the old way for one release; migration note and `bhote doctor` check; then herdr is removed from the docs, the plugin and the dotfiles config.

## Order and size

M0, M1 and M2 are the critical path (the rest builds on them). M6 is independent and can be done first if a quick win is wanted. M3 and M5 are the largest after M1. Each milestone is its own PR into `rust/mux`; `rust/mux` goes into `main` only at M9 (or earlier behind `BHOTE_MUX=native`, off by default).

## Tests

- Pty tests like `tests/panel.py` run the native server and client in a pseudo-terminal.
- A replay test feeds recorded escape-code streams into the terminal state and compares the screen.
- The existing suites run against both backends from M2 on (a `BHOTE_MUX` switch in `tests/run.sh`).

## Risks

- A terminal emulator that is correct enough for Claude Code and other TUIs (colours, alt screen, mouse modes, bracketed paste, kitty keyboard, resize) is the main risk; use a proven crate, do not write one.
- Every key and mouse path that herdr handles today has to be rebuilt; budget most of the time for step 2.
- No flag day: both backends run side by side until step 7.

## First deliverable

`bhoted mux` with one pane: start a shell in a PTY, attach, type, resize, detach, attach again. Then splits and the `pane list` / `pane layout` verbs, so the existing panel can be started in it.
