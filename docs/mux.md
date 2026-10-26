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

The side panels are **global**: one left panel, one right panel and (with `CHAT_PANEL=on`) the chat strip at the bottom, the same for every workspace and tab. Switching
a workspace changes what the main area shows and nothing in the panels (they keep running, their scroll and selection stay). They are not panes of a tab (`herdr pane list`
does not list them; `pane list --all` does; `pane layout` has them under `sidebars`).

What a panel shows is a list of **sections**: `workspaces` (of all machines, grouped by machine), `agents` (the cards), `overview` (the agents' summary and the keys at the bottom),
`topics`, `done`, `deployments`, `tests`, `worktrees` (the git worktrees of the repositories the agents work in, with who works in each), `head`.
`PANEL_LEFT` (default `workspaces,agents,overview`) and `PANEL_RIGHT` (default `topics,done,deployments,tests`) choose them, `off` takes a panel away,
`PANEL_LAYOUT=single` makes one panel on the right with everything. Widths: `PANEL_LEFT_WIDTH` (40), the right one `PANEL_MIN_WIDTH`..`PANEL_MAX_WIDTH`; they are fixed (a bigger
window gives the room to the main area), you can drag a divider. In a narrow window the right panel goes first, then the left one gets narrower, then it goes, too: the main
area keeps at least 30 columns; a panel that does not fit is not drawn, but keeps running. `prefix b` hides and shows the panels, `prefix t` goes into a panel and back,
`prefix B` makes a panel that ended again.

**Your own panels:** `PANEL_<NAME>_CMD=<program>` hangs any program under the bhote panel of a column (`PANEL_<NAME>=left|right`, default right; `PANEL_<NAME>_HEIGHT` rows, default 12), e.g.
`PANEL_GIT_CMD=lazygit` and `PANEL_GIT_HEIGHT=16`: it is a real terminal (keys, mouse), the same for every workspace; in a low window the last ones go first.

`CHAT_PANEL=on` (`CHAT_HEIGHT`) adds the chat strip (`bhote chat`): the history of an agent from its Claude Code transcript, an input field
(`alt+enter` new line, `up` earlier messages), `@` picks the agent, `enter` sends it to the agent's prompt.

## Agents

The agent in front of a pane is found from its process (`claude`, `codex`, `gemini`, …); its state is what it reported (`pane report-agent`), else what the
screen shows (`esc to interrupt` = working, a question = blocked). Work that ends while nobody looks is `done` until the pane is shown. Every change runs
`bhote event`, as the herdr plugin did.

## Other machines (kharka)

With `STORE=kharka` or `AGENTS_VIA=kharka` the server runs `herdr-kharka` against its own socket (the sidecar: started with the server, started again when it ends,
ended with it; `KHARKA_SIDECAR=off` stops that, `KHARKA_LABEL` names the machine). The other machines then read this machine's workspaces and agents from kharka
(`herdr/<node>`) and can focus and prompt its agents; this machine's panel shows theirs, whether they run herdr or the native mux. **Stop the old `herdr-kharka`
service on a machine that runs the native mux** (it points at herdr's socket and writes the same entry): Linux `systemctl --user disable --now herdr-kharka`,
macOS `launchctl bootout gui/$(id -u)/dev.kharka.herdr`. `bhote doctor` says which one runs. `bhote mux stop|status|restart` controls the server.

A click on an agent of another machine opens it in a tab of its own, named `<machine> <pane>`: the tab runs `ssh -t <node> …` with `bhote view <pane>`
(the other machine runs the native mux; `bhote view` shows that one pane, every key goes to it, it does not resize it) or, when it runs herdr,
`herdr agent attach <pane>`. The ssh destination is the kharka node name; `REMOTE_SSH` changes the command (`{node}`). A second click shows the same tab.
Close the tab (`prefix k`) to leave. The other machine needs `bhote` (or herdr) in the PATH of a login shell, and ssh from here to there must work.

## What is tested, and what is not

- CI (`.github/workflows/tests.yml`, also for `rust/**`) runs every suite on **macOS (bash 3.2) and Linux (bash 5)**, plus clippy and the dependency advisories. The mux suites (`tests/mux*.sh`)
  start real servers on throw-away sockets, drive them through a pseudo terminal and cover keys, mouse, panels, sessions, agents, the hook, kharka (with a real kharka daemon),
  other machines (a fake ssh that runs the command here), no herdr on the PATH, and speed (key echo about 2 ms, 300000 lines of output in 0.5 s, a client that does not read does not hold the server up).
- `bash tools/check-macos.sh` type-checks the daemon for macOS on any machine.
- Not covered: two real machines (ssh between them is what you have to set up: `ssh <node> true` must work without a password both ways; `bhote doctor` checks it),
  kitty graphics (not passed on), OSC 8 hyperlinks (not passed on), `herdr` plugins other than bhote's.

`herdr --machine <label> <command>` works in the native mux: the command runs over ssh on that machine (its kharka node) and its answer comes back.
