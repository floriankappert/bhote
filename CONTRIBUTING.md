# Contributing to bhote

Thanks for helping. Bug reports, ideas and pull requests are all welcome. For anything larger than a fix, open an issue
first, so that we can agree on the shape before you write it.

## Development setup

bhote is one bash script (`bhote`) plus a few small pieces around it:

| Path | What |
|---|---|
| `bhote` | the panel, the CLI, the collector, the wizard (see [docs/architecture.md](docs/architecture.md)) |
| `herdr-plugin/` | the herdr plugin `bhote.panel` (POSIX `sh`, it runs under herdr's shell) |
| `omarchy-plugin/` | the Omarchy bar widget `bhote.bar` |
| `integrations/claude/skills/` | the Claude Code skills `bhote` and `bhote-install` |
| `packaging/` | the Homebrew formula and the Arch PKGBUILD |
| `tests/` | the test suites (plain bash) |
| `tools/` | `dog.py` (the welcome screen dog as a pixel grid), `screenshot.sh` + `ansi2svg.py` (the README picture), `release-check.sh` |

Run it from a checkout:

```sh
git clone https://github.com/floriankappert/bhote && cd bhote
./install.sh          # links ~/.local/bin/bhote and the herdr plugin to this checkout
bhote reload          # restart the running panels after every change you want to see
```

Your panels now run whatever is checked out, a feature branch included. Run `bhote reload` after switching branches.

## Tests

```sh
bash tests/run.sh                 # every suite
bash tests/run.sh smoke cli       # some of them
bash tests/agents.sh              # one suite, with every line of output
```

The suites need `jq`, `python3` and `git` (with a user name and email). They work in throw-away folders with a fake
`herdr` on the `PATH`, and never touch your config, your topics or the herdr you work in. CI runs them on macOS with the
system bash 3.2 and on Linux with bash 5.

Useful hooks for your own experiments:

| Variable | Effect |
|---|---|
| `BHOTE_CONFIG`, `BHOTE_DATA`, `BHOTE_SHARED` | config file, data folder and shared runtime folder (point them at a temp dir) |
| `BHOTE_ONCE=1 BHOTE_VIEW=main` | print one frame of a view (`splash`, `main`, `settings`, `menu`, …) and exit |
| `BHOTE_COLS`, `BHOTE_ROWS` | the size of that frame |
| `BHOTE_FORCE_COLOR=1`, `NO_COLOR=1` | colours on even into a pipe / off |
| `BHOTE_SOURCE_ONLY=1` | `. ./bhote` loads the functions without starting anything (how `tests/agents.sh` calls them one by one) |

Lint with [shellcheck](https://www.shellcheck.net/): CI fails on errors (`shellcheck -S error bhote install.sh tests/*.sh
tools/*.sh herdr-plugin/*.sh omarchy-plugin/*.sh`). New code should not add warnings either (`shellcheck -S warning`);
`.shellcheckrc` lists the few checks that are off on purpose.

## The rules the script lives by

- **bash 3.2.** macOS ships it as `/bin/bash`, and bhote must run there. No associative arrays (`declare -A`), no
  `mapfile`/`readarray`, no `${var,,}`/`${var^^}`, no `declare -n`, no `EPOCHREALTIME`, no `read -t 0` as the only path,
  no `;&`/`;;&` in `case`, no negative array indexes. `printf -v` is fine. `herdr-plugin/*.sh` is POSIX `sh`.
- **Nothing leaves the machine by default.** Every setting that opens a connection (ssh, another machine, Slack, a CI
  API) defaults to off and is switched on by the user in the settings or the wizard.
- **Input is data, never code.** The config is read line by line, never sourced. Every value that ends up in `$(( ))`
  goes through `num_or`/`num_set` first (bash runs `a[$(cmd)]` inside arithmetic). Control characters are stripped from
  everything that is shown on screen or sent to an agent pane.
- **One writer at a time.** Files that several processes write (config, topics, lists in the shared folder) are written
  to a temp file and moved into place, under a lock where two writers can meet.
- **The frame is drawn often.** Avoid forks (`$(…)`, pipes, external commands) in the drawing path; prefer builtins and
  `printf -v`.

## Adding a setting

1. A row in the `SETTINGS` table at the top of `bhote`: `KEY|default|type|description` (`bool`, `int:min:max`,
   `enum:a:b`, `text`, `list`, `secret`). `bhote config` and the settings screen follow it.
2. A row in [docs/configuration.md](docs/configuration.md), and in the README if users will look for it there.
3. If the wizard should offer it: a wizard step, `FEATURE_LEVEL` counted up, and the step's level in `step_level`.
   Machines that set up at a lower level then show "New Features Available".

## Style

- Comments are short English sentences that say *why*, at the end of the line or above a block; sections start with
  `# ── name`. Four spaces, no tabs (see `.editorconfig`).
- User-facing text is plain English, short, and the same words everywhere (panel, CLI, docs).
- Commit messages: one English line in the style of the history, `Area: what changes` (e.g. `Topic page: a done topic
  gets its own actions`). Several authors: add `Co-authored-by: Name <email>` trailers so that GitHub credits each.

## Pull requests

`main` is protected: every change goes through a pull request with one approving review, and the CI checks must pass.
The pull request template has the checklist. Keep a pull request to one topic; update `CHANGELOG.md` under
*Unreleased* when users will notice the change.

## Releasing (maintainers)

1. Bump the version in `bhote` (`VERSION=${BHOTE_VERSION:-X.Y.Z}`), `herdr-plugin/herdr-plugin.toml`,
   `omarchy-plugin/manifest.json`, `packaging/arch/PKGBUILD` (`pkgver`, `pkgrel=1`) and `packaging/homebrew/bhote.rb`
   (`tag: "vX.Y.Z"`).
2. Move *Unreleased* in `CHANGELOG.md` to `## [X.Y.Z] - date` and add the compare link.
3. `bash tools/release-check.sh X.Y.Z`, then commit `bhote X.Y.Z` (through a pull request).
4. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`. The *Release check* workflow checks the versions again.
5. `gh release create vX.Y.Z --notes "<the CHANGELOG section>"`.
6. Copy `packaging/homebrew/bhote.rb` to `Formula/bhote.rb` in
   [floriankappert/homebrew-bhote](https://github.com/floriankappert/homebrew-bhote) and commit `bhote X.Y.Z` there.
7. Check: `brew update && brew upgrade bhote && bhote version`, then `bhote reload`.
