# The native mux

`bhote` typed in a terminal starts bhote's own pane server and attaches to it (herdr is no longer needed; `MUX=herdr`, or
`BHOTE_MUX=herdr`, starts herdr as before for one more release). Needs the Rust daemon: `cargo build --release` in `daemon/`.

- `bhote` (no arguments, in a plain terminal) / `bhote attach` — start the server if none runs, attach. The server outlives the terminal.
- `ctrl+b d` — detach; `bhote` again shows the same screen. After a restart of the machine or a killed server, the session is back
  (layout, names such as `w1:p2`, labels, folders; agents with a session are started again with their resume command, `claude --resume <id>`).
  The session is `~/.local/state/bhote/mux-session.json` (`BHOTE_MUX_STATE`).
- A panel pane is marked with `BHOTE_PANEL=1` (the mux and the herdr plugin set it); a plain `bhote` in a terminal always starts the mux, even when the terminal inherited `HERDR_*` variables. `bhote doctor` — what the mux needs, and what is left of herdr.

Inside a pane, `herdr ...` is a link to the mux with herdr's words and JSON (`herdr pane list`, `pane split`, `agent prompt`, `workspace create`, …),
so the scripts of bhote, the Claude Code hooks of herdr's integration (`pane.report_agent_session`) and `herdr-kharka` work unchanged.
The socket is `$XDG_RUNTIME_DIR/bhote/mux.sock` ([protocol](mux-protocol.md)).

## Keys

Prefix `ctrl+b`. The file `~/.config/bhote/keys` (`BHOTE_KEYS`) changes them; `bhote` shows the keys with `prefix ?`.

```
prefix = ctrl+space                       # herdr's old key
split_vertical = prefix+v alt+enter       # action = key key ...
run prefix+g lazygit                      # a shell command
popup ctrl+alt+t bhote find               # a floating pane over the tab
detach = none                             # take an action's keys away
```

Defaults: `d` detach · `v` / `h` split (right / below) · `x` close pane · `z` zoom · `;` last pane · arrows focus · `ctrl+arrows` resize mode ·
`c` new tab · `1..9` tab · `n` `p` tab · `,` rename tab · `k` close tab · `C` new workspace · `N` `P` `K` workspaces · `O` rename pane · `[` copy mode ·
`t` jump between the panel and the agent · `b` open the panels · `?` help · `q` reload the keys. Without the prefix: `alt+1..9`, `alt+left/right` (tabs),
`alt+up/down` (workspaces), `ctrl+alt+arrows` (focus), `ctrl+alt+shift+arrows` (resize).

Mouse: a click focuses a pane or switches a tab or workspace on the bar; dragging a divider resizes; the wheel scrolls the pane's history
(an alternate-screen program gets arrow keys; a program that asked for the mouse gets the mouse); dragging selects text and copies it (OSC 52).
Copy mode (`prefix [`): `hjkl`, `w b 0 $ g G`, `ctrl+u/d/b/f`, `v` / `V` select, `y` copy, `q` leave.

## Panels and chat

Every tab gets the panels of bhote as panes (what the herdr plugin did): the right one, and with `PANEL_LAYOUT=split` the left one
(`PANEL_LEFT_WIDTH`, `PANEL_MIN_WIDTH`, `PANEL_MAX_WIDTH`), unless `AUTOSTART=off`. A panel you close stays closed in that tab until `prefix b`.
`CHAT_PANEL=bottom|right` (`CHAT_HEIGHT`) adds the chat pane (`bhote chat`): the history of an agent from its Claude Code transcript, an input field
(`alt+enter` new line, `up` earlier messages), `@` picks the agent, `enter` sends it to the agent's prompt.

## Agents

The agent in front of a pane is found from its process (`claude`, `codex`, `gemini`, …); its state is what it reported (`pane report-agent`), else what the
screen shows (`esc to interrupt` = working, a question = blocked). Work that ends while nobody looks is `done` until the pane is shown. Every change runs
`bhote event`, as the herdr plugin did.

## Not done / not tested

- macOS: the same code (`libc` constants, `ps` instead of `/proc`), but only built and tested on Linux so far.
- `herdr --machine <label> ...` is not available in the native mux; other machines are reached through kharka (`herdr-kharka --herdr-socket <mux socket>`).
- The herdr plugin and herdr's config are still in the repository for `MUX=herdr`; they go after one release.
