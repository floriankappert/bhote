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

## Steps

| # | Daemon takes over | Bash keeps | Done when |
|---|---|---|---|
| 0 | topic index (prototype in `daemon/`, same file as `topics_index_build`) | everything | index identical on 414 real topics |
| 1 | the topic store: parse, `topic_set`/`topic_new`, locks, events (inotify) | panel, CLI call it over a socket | `tests/cli.sh`, `store.sh` pass in both modes |
| 2 | agents of every machine (herdr, kharka), agent names, cards | panel reads a snapshot | `tests/agents.sh` in both modes |
| 3 | monitors: CircleCI, GitHub Actions, local test runs | panel reads a snapshot | `tests/monitor.sh` in both modes |
| 4 | follow-ups, auto-assign, reminders, Slack pins, update check, kharka watch | CLI | collector no longer runs in bash |
| 5 | the frame: the panel draws from the snapshot in Rust (CLI and wizard stay bash or follow) | wizard | frame under 5 ms |

Each step names its measurement (CPU of panels + collector, frame time) before and after in the commit message.
Distribution (Homebrew, PKGBUILD, CI for Linux and macOS arm64/x86) comes with step 1, when the daemon first matters to users.
