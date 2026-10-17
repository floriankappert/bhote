# Rust rewrite: `bhoted`

Goal: the work that costs time in bash (agents of every machine, topic store, monitors, follow-ups, frame building)
runs in one Rust daemon per machine. Measured today (bash): collector round 0.16 s every 3 s, one frame 0.09 s, the panels
and the collector together about 9 % of a core. Aim: the daemon under 1 %, a frame under 5 ms.

## Rules

- **Strangler, not big bang.** Every step ships on its own; bash keeps working after each one. Without the daemon (no
  binary, an unsupported system) bhote runs as today (bash 3.2, no dependencies: [CONTRIBUTING.md](../CONTRIBUTING.md)).
- **Same data.** `*.topic` files stay the format (also for kharka and for backups; [data-and-sync](data-and-sync.md)); the
  daemon reads and writes them with the same rules (first value of a key counts, temp file + rename).
- **Parity tests.** A step is done when the bash suites pass with the daemon on and off, and a Rust test compares its
  output with the bash function's on the same data (the prototype's index is compared that way).
- Standard library first; a crate only where it is the difference (inotify, JSON, sockets).

## Interface: files, not a socket

bash cannot talk to a Unix socket without starting a process (`nc`, `socat`), and a process per read is exactly what made
bhote slow. So the daemon and the bash clients meet where they already do: in files. The daemon writes snapshots (the
topic index, the agent and monitor lists) that panels read with builtins; changes a user makes still go through the CLI
or the bash functions, which invalidate the snapshot (`topic_set` removes `topics.idx`, the next reader gets a current
one). From step 4 on the daemon writes topics itself, in-process.

## Steps

| # | Daemon takes over | Bash keeps | Done when |
|---|---|---|---|
| 0 | topic index (prototype in `daemon/`, same file as `topics_index_build`) | everything | index identical on 414 real topics |
| 1 | the topic index, kept current by inotify (Linux; macOS: a look every 100 ms) | writers (`topic_set`, CLI) | `tests/store.sh` passes with the daemon on and off; measured below |
| 2a | this machine's agents: `pane list` + `workspace list` into `agents.local`, `.meta.local`, `.now.local`, `agents.cwd`, `focus.pane` (Linux; macOS keeps bash until kqueue) | panel reads the files as before | byte-identical files, `tests/agents.sh` |
| 2b | the other machines through kharka (`AGENTS_VIA=kharka`): `herdr/<node>` and `_nodes/` into `agents.remote`, `.meta.remote`, `.now.remote`, `agents.nodes`, `machines.offline` (the ssh way stays bash) | agent names, cards, the ssh path | byte-identical files (live and `tests/agents.sh`) |
| 3 | monitors: CircleCI (API, `ureq`), GitHub Actions (`gh`), local test runs and status files into `deploys.list` / `tests.list` | panel reads the lists as before | same lists as `monitor_fetch` (live and `tests/monitor.sh`) |
| 4a | ten follow-ups of the collector (cards, dedup, review, finished, gone, question, branches, numbers, reminders), through a Sink: `BHOTED_FOLLOW=off\|shadow\|on` | the other follow-ups (tests, deploys, auto-assign, projects), the stores' sync, Slack, updates, kharka watch | decisions identical in `tests/follow.sh` (logged and written), shadow soak with `tools/shadow-report.sh` |
| 4b | the rest of the collector | CLI | collector no longer runs in bash |
| 5a | blocks of the main view, one at a time, as `bhoted block <name>` (the panel calls it once per block): monitors, cards, lists; then done, agents, head, bottom | key and mouse handling, click maps, the other views | `tests/frame.sh`: the same bytes as the bash function, with and without colour, three widths |
| 6a | the panel itself (`bhoted panel`, `BHOTED_PANEL=on`): terminal, keys, mouse, hover, the main view, its actions, a line to type into; the topic's page (menu) and the chooser of free agents are the panel's own (68 golden cases); settings, wizard and the suggestions are still the script's views, run as a child on the same terminal | the collector (as a child of the panel), the views listed | `tests/panel.py`: a pty, the real binary, clicks and keys |
| 5b | the whole main view and its maps in one call (`bhoted block frame`), and the screen made of it (`bhoted paint`) | key and mouse handling, `ui_load`, the other views (menu, settings, pick, wizard) | `tests/frame.sh`: the same frame, screen and click maps, step by step |

Each step names its measurement (CPU of panels + collector, frame time) before and after in the commit message.
Distribution (Homebrew, PKGBUILD, CI for Linux and macOS arm64/x86) comes with step 1, when the daemon first matters to users.

## Step 1, measured

`bhoted run` (setting `BHOTED=auto`, started by the lead collector, ends with it): 0.1 % CPU idle, 2.7 MB, an index
rebuild 4 ms (awk: 20 ms), a new file is in the index within a moment. The panel's "is the index current?" check no
longer looks at 414 files: about 4 ms before, 1.6 ms per call now (mostly the setting read). The gain of this step alone
is small, as predicted; it is the foundation: the daemon exists, is started, found and tested.

## Step 2a, measured

The daemon runs the local collection (poke file wakes it at once, `LOCAL_EVERY` and `TESTS_BUSY` read from the settings
file each round) and writes the five files byte for byte as `collect_local` does (checked on the live herdr and by
`tests/agents.sh` against the fake herdr, both `TESTS_BUSY` values). The lead collector skips `collect_local` while the
daemon has the duty (`bhoted.pid.agents`). A round was ~29 ms in bash (`collect_local`, ~8 processes: jq five times, mv,
date); in the daemon it is two `herdr` calls and the parse in-process. The idle daemon takes 0.9 % CPU, most of that the
two `herdr` processes it must start every 3 s (a direct socket to herdr would remove them).

## Step 3, measured

`bhoted` writes `deploys.list` and `tests.list` (`DEPLOY_MONITOR`/`TEST_MONITOR`, `*_EVERY`, the per-CI switches and the
CircleCI token read from the settings; `bhoted.pid.monitors` tells the collector to leave it alone). The lists are
identical to `monitor_fetch`'s on the live projects (CircleCI, GitHub, the 43 status files) and in `tests/monitor.sh` (the
same fake `gh` and `curl`; `BHOTED_HTTP=curl` makes the daemon use the fake curl in tests). One fetch round, bash vs Rust:
deployments 7.1 s -> 1.3 s, tests 6 to 14 s -> 1.3 s. Most of the time was waiting for the network one call after the
other; the daemon asks for the independent calls side by side and asks git once per folder, not once per status file
(that alone was 3 s). The rounds run in the background of the daemon, never in a panel.

## Step 2b, measured

With `AGENTS_VIA=kharka` the daemon reads the other machines' `herdr/<node>` entries (every `REMOTE_EVERY` s, and at
once when kharka's watch creates `remote.due`), and writes the five files byte for byte as `collect_remote_kharka` does
(live, and in `tests/agents.sh` with a fake kharka: a fresh machine, a stale one, an offline node, a machine switched off,
control characters, a running test; and the case that kharka does not answer: the agents stay, muted). The collector skips
its own look while the daemon has the duty (`bhoted.pid.remote`). Agent names and cards stay in bash for step 4/5.

## Step 4a: shadow first

The follow-ups change topics that kharka copies to every machine, so they go over in three stages (`BHOTED_FOLLOW`):
`off` (bash), `shadow` (bash acts; bash and daemon both log every decision, `shadow/bash.log` and `shadow/rust.log`; the
daemon's own state files live in `shadow/`, so it never moves the real ones), `on` (the daemon acts, bash skips them).
`tests/follow.sh` runs every function on the same fixture (bash and Rust) and compares the decisions, then the topic files
that come out, with the real writes; a test that fails on purpose (a changed threshold) was seen to fail. `tools/shadow-report.sh`
compares the two logs of a real run. A round of all ten takes 26 ms in the daemon; the topics are read once and again only
after a write.

## Step 5a: blocks of the frame

`bhoted block monitors|cards|lists` draws a block with the same bytes as `monitors_block` and `topics_block cards|lists`
(204 golden cases in `tests/frame.sh`: every card kind, selection, hover, open groups, project scope, narrow and wide, colour
and none; a changed rule in the code was seen to fail them). The colours come as the script's own variables
(`colors_dump`, written when the theme is applied), the screen's own state (selection, hover, open groups, the dancing star's
character) as `view_dump` writes it; everything else is read from the same files. Differences that are on purpose: the daemon
reads the topic files itself, so a frame of navigation shows a topic that was made a moment ago one frame sooner than the
arrays of bash would; the order of topics follows the locale's collation of the file names, like the shell's glob.
Measured: monitors 40 ms -> 2.8 ms, a frame after a reload 126 -> 78 ms, a frame of navigation 99 -> 44 ms.

## Step 5b: the whole main view

`bhoted block frame` is `bhote_view`: it gets the head and the bottom (agents, keys) as the script made them, draws the cards,
lines, done area and monitors itself, cuts and windows them to the rows (`toff`), and writes the maps for the clicks (`tmap`,
`spinrows`, `divrow`). `bhoted paint` is `bhote_paint`'s second half: the markers out, the line ends, `pmap`. Together 368
golden cases step by step (a walk of the selection through the list at 11 sizes, with colour and without, with open groups,
with the project scope, empty), plus the live state of this machine byte for byte. A redraw: 130 ms at the start of the work,
now 29 ms (`ui_load` 12, frame 9, screen 8); a frame of navigation 99 -> about 17 ms. `ui_load` got 15 ms faster by not
computing each topic's project unless the project scope is on.
What stays in bash on purpose: the keys and the mouse (they need the arrays), `ui_load`, and the views that are not the main
view. A Rust panel would have to take those over as well; that is a different, bigger step (the whole of step 6, if wanted).

## Step 6a: the panel in Rust

`BHOTED_PANEL=on` (off by default) makes `bhote` replace itself with `bhoted panel`. The Rust panel owns the terminal (raw mode, the
alternate screen, SGR mouse with motion, the resize signal, Ctrl-C and a closed terminal clean up, also after a panic), reads
keys and mouse reports into events, draws the main view in-process (no process per block, no files for the maps), and does what
a click or a key does: select, expand "+N more", the section toggles and project checkboxes, the review card's buttons, jump
to an agent, check off (x), dismiss (c), edit title or description (e, d), new topic (n), with a small line editor on the last
row. What still needs the script is run through `bhote __act <name>` (jump to an agent or a topic's agent, answer a review: the
herdr and kharka calls) or as a view: the topic's menu (double click, right button, →), settings (,), the wizard (w and the
first start), the suggestions (a) and the choosers (s, t) run as `bhote` in a child on the same terminal (`BHOTE_DELEGATE`), the
panel takes over again when it ends. The collector runs as a child of the panel (`BHOTE_COLLECTOR_ONLY`).
Not there yet: the welcome screen with the dog, and the views above (each is a candidate for the next steps, with the rest of the collector).
Measured: the panel idles at 0.2 % CPU and 5 MB; a hover or a click redraws in a few milliseconds.

## Step 6b: the topic's page and the chooser

`bhote_menu`, `menu_build`, `bhote_pick` and `pick_build` are in `menuui.rs` (the same bytes as the script, markers for the clicks and the
selected line included: 68 cases in `tests/frame.sh`), with every action of `menu_do` (the status changes, wait, project, rename,
describe, delete, jump, start, transfer, resume, deployment wait, Slack pin) and the keys of the page (arrows, numbers, Enter, Esc) and
a click on a line. What talks to herdr, kharka or Slack runs as `bhote __act …`. A topic with a Slack pin or answer on it still gets
the script's page (its text goes through `slack_marks`). **Settings, the wizard and the suggestions stay the script's views on purpose:**
they are used rarely, nothing in them is time critical, and a port would be some 2,000 lines for no gain a user would see.
