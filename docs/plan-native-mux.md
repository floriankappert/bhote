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

## Risks

- A terminal emulator that is correct enough for Claude Code and other TUIs (colours, alt screen, mouse modes, bracketed paste, kitty keyboard, resize) is the main risk; use a proven crate, do not write one.
- Every key and mouse path that herdr handles today has to be rebuilt; budget most of the time for step 2.
- No flag day: both backends run side by side until step 7.

## First deliverable

`bhoted mux` with one pane: start a shell in a PTY, attach, type, resize, detach, attach again. Then splits and the `pane list` / `pane layout` verbs, so the existing panel can be started in it.
