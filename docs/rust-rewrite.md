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
| 2b | the other machines (ssh, kharka), agent names, cards | panel | `tests/agents.sh` in both modes |
| 3 | monitors: CircleCI, GitHub Actions, local test runs | panel reads a snapshot | `tests/monitor.sh` in both modes |
| 4 | follow-ups, auto-assign, reminders, Slack pins, update check, kharka watch | CLI | collector no longer runs in bash |
| 5 | the frame: the panel draws from the snapshot in Rust (CLI and wizard stay bash or follow) | wizard | frame under 5 ms |

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
