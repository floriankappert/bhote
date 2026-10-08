# Architecture

A map of the `bhote` script for contributors: which processes run, which files they share, and where things live in the
script. For the rules the code follows (bash 3.2, nothing leaves the machine by default) see
[CONTRIBUTING.md](../CONTRIBUTING.md).

## Processes

The same script runs in four roles:

| Role | Started by | Does |
|---|---|---|
| **Panel** | `bhote` in a pane (the herdr plugin types it into the pane it opens) | Reads keys and mouse reports, draws a frame whenever what is shown would change (`bhote_main`, `bhote_paint`). Starts the collector in the background. |
| **Collector** | every panel, in the background; only the one holding `collector.lock` works | Asks herdr for the agents (this machine every `LOCAL_EVERY` s, the others every `REMOTE_EVERY` s), merges the topics with the data location, runs auto-assign, the branch sync, the monitors, the Slack watch and the update check. Each round re-reads the settings. Ends when its panel ends. |
| **Plugin hooks** | herdr, through `herdr-plugin/*.sh` | `open_panel.sh` opens the panel next to the agent pane; `on_event.sh` runs `bhote event` when an agent changes state (review topics, notifications, a poke for the panels); `dump.sh` hands the topics to another machine; `changed.sh` takes a pushed agent state; `focus.sh` jumps between agent and panel; `search.sh` runs `bhote find` in the popup. |
| **CLI** | you, scripts, agents (`bhote add …`) | Works on the same files with the same functions and merges with the data location right after a change. |

Several panels can run on one machine (one per tab); they all read what the one collector writes. A panel never waits
for herdr or ssh: everything slow happens in the collector, and the panel only reads files.

## Files

| Where | What | Who writes |
|---|---|---|
| `~/.config/bhote/config` (`BHOTE_CONFIG`) | `KEY=value` settings, never sourced | panel, CLI, wizard (`cfg_set`, under `config.lock`) |
| `~/.local/share/bhote/topics/*.topic` (`BHOTE_DATA`) | one file per topic, plus project, machine and agent-number records ([format](data-and-sync.md)) | panel, CLI, collector, `bhote event` |
| `~/.local/share/bhote/testruns/` | runs of `bhote test run` | CLI |
| the shared folder (`BHOTE_SHARED`, default `$XDG_RUNTIME_DIR` or `$TMPDIR` + `/bhote-$UID`, mode 0700) | the agent lists (`agents.local`, `agents.remote`, `agents.meta.*`), `collector.lock`, the revision that tells panels to redraw, the monitor lists, `machines.offline` | collector (the panels read) |
| a private scratch folder per process (`RUN_DIR`, removed on exit) | the frame's click maps (`divrow`, `agmap`, `agrows`) | the panel |

Writes go to a temp file that is moved into place, so a reader never sees half a file. Agent lists use the unit separator
`US` (`\037`) between fields, so empty fields stay fields.

## Inside the script

The script is ordered top to bottom; every part starts with a `# ── name` line (`grep -n '^# ── ' bhote` lists them):

| Section | Contains |
|---|---|
| settings | the `SETTINGS` table (key, default, type, description) and `cfg_var`/`cfg_get`/`cfg_set`, number guards (`num_or`, `num_set`) |
| topics | the topic store: `topics_load` (the `T_*` arrays), `topic_get`/`topic_set`, the replica and `store_sync` |
| themes | the colour roles for `THEME=terminal` and `THEME=catppuccin`, `COLOR_*` overrides |
| the agents of every machine | `collect_one`, `collect_local`/`collect_remote`, the collector loop, auto-assign, `bhote event` effects |
| the topic list | the main view: head, topics, done area, agents (`topics_block`, `done_block`, `agents_block`, `bhote_view`) |
| the settings screen | the rows of every settings page and their keys |
| the animation | the welcome-screen dog and the busy agents' star (a ticker that only decides the picture; the main loop draws) |
| suggestions, the picker | `a` (topics from what agents work on), `s` (choose a free agent) |
| projects, machine codes, agent numbers | project records and repository keys, `MAC`/`OMR` codes, `MAC4` numbers |
| talking to agents | the live state of a pane and prompts to it, through herdr (`--machine` for another machine) |
| Steal & Transfer | hand-over prompts and their state ([how it works](steal-and-transfer.md)) |
| Slack pins | the Slack watch through Claude Code |
| the deployment and the test monitor | GitHub Actions, CircleCI, local runs and live status files ([monitors](monitors.md)) |
| updates | the newest tag, the update notice |
| run | `bhote_main`: the input loop and the views (`main`, `menu`, `settings`, `pick`, `suggest`, `hotkeys`, `splash`) |
| the setup wizard | its steps, `FEATURE_LEVEL`/`step_level` |
| the command line | `bhote_cli` and one `cli_*` function per command; `--json` output follows [the schemas](schema/) |
| bhote find | the search popup |

At the very end the script dispatches: no arguments start the panel, a known word runs that command, and
`BHOTE_SOURCE_ONLY=1` stops before either (the tests source the functions this way).

## Drawing a frame

`bhote_paint` loads the state once (`ui_load`), builds the frame in a subshell (`bhote_draw <view>`), and writes it with
one `printf`: cursor home, every line cleared to its end, the rest of the screen cleared. Nothing is cleared first, so
nothing flickers. Because the frame is built in a subshell, anything the main loop needs from it (where the divider is,
which agent sits on which row) is left in `RUN_DIR` as a small file. Invisible markers in the text (`\001` for the
selected line, `\002` for a field being edited) tell `bhote_paint` where to scroll and where to open the editor; they are
stripped before the frame is written.

## Tests

`tests/*.sh` source the script with `BHOTE_SOURCE_ONLY=1` or run it with `BHOTE_ONCE=1`, against throw-away folders and a
fake `herdr` (and fake `ssh`/`curl` where needed) on the `PATH`. See [CONTRIBUTING.md](../CONTRIBUTING.md#tests).
