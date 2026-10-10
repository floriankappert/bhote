# Changelog

All notable changes to bhote. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and bhote
uses [semantic versioning](https://semver.org/). Each release's section is also its GitHub release text.

## [Unreleased]

### Added
- `AGENT_CARDS` (settings: *Cards for agents without a topic*, default `on`): a working agent that has no topic gets a card
  on *Now* by itself, titled with what it works on; it goes when the agent stops or gets a topic of its own.

### Added
- `STORE=kharka` (settings › *Data location*, setup wizard step *Topics*): the topics merge with the local
  [kharka](https://github.com/jakobbeyer/kharka) daemon, which syncs every machine through its hub, also after one was
  offline. Each topic is an entry `bhote.topics/<id>` (records: `bhote.projects/`, `bhote.machines/`,
  `bhote.agentnums/`); the merge, its checks and the tombstones are the ones of the replica machine. The collector
  follows `kharka watch` and is woken at once by a change on another machine; the merge every minute is a safety net.
  A push is conditional (`--if-hlc`), and when kharka changed in between, the merge runs again at once. The summary
  shows the hub (`hub offline`, writes pending) and kharka's own complaint when a merge fails; `KHARKA` names the binary.

## [0.7.0] - 2026-10-09

### Added
- Review card: `bhote review <ref> --summary <text> --look` puts a summary sub-card and a jump button (*Review ansehen*)
  under a topic in review; the card is gone when the topic moves on. `--ask` collapses to the same jump button, never to
  a button that triggers an action. `bhote show --json` has `review.summary` and `review.actions`.
- On hover the card's coloured left bar turns coral.
- `AGENT_TOPICS` (settings: *Agents may create topics*, default `on`): `off` stops agents from creating topics with
  `bhote add`; the "Question from" topics stay with `AUTO_REVIEW`.
- A Dracula theme: `THEME=dracula` (settings › *Colours*), with the exact colours of the Dracula palette.

### Changed
- The search popup no longer lists free agents; it shows only the newest 25 done topics and "+N more done tasks"
  until something is typed, then the search reaches all.
- `bhote skills install` and the setup link the skills into a git checkout instead of copying them, so they no longer go
  stale after a `git pull`; earlier copies become links. A package (brew, AUR) still copies.

### Fixed
- The ssh masters that herdr leaves behind for every `herdr --machine` call (one per call, in `/tmp/herdr-ssh-*`) no
  longer pile up: with `REMOTE_AGENTS` on and herdr 0.9.1 (`ControlPersist=yes`) they reached the process limit within a
  night and every fork failed; herdr 0.9.3 ends them after 10 minutes but keeps their folders. The collector ends the
  ones whose herdr is gone and removes the folders, at start and then every minute.

## [0.6.0] - 2026-10-09

### Added
- Topics in the panel are cards: an own background (lighter when selected, a step lighter under the pointer), the status
  colour as a bar on the left edge, half-block edges above and below. Each topic is one row taller. The panel now asks the
  terminal for mouse motion (for the hover) and redraws only when the card under the pointer changes.
- Contributor files: CONTRIBUTING, code of conduct, security policy, support, issue and pull request templates,
  CODEOWNERS, `.editorconfig`, `.shellcheckrc`, an architecture guide (`docs/architecture.md`).
- CI: the test suites on macOS (bash 3.2) and Linux, shellcheck, and a release check for `v*` tags
  (`tools/release-check.sh`). `tests/run.sh` runs every suite.
- README: a picture of the panel (`tools/screenshot.sh`), requirements, quick start, troubleshooting, uninstall.

### Changed
- Neutral defaults: `PROJECT_COLORS` is empty and `TEST_STATUS_DIR` is `~/.cache/bhote/test-status`. To keep the old
  values, set them: `bhote config set TEST_STATUS_DIR ~/.cache/ims-test-status` and
  `bhote config set PROJECT_COLORS marketing=pink,ims=teal,bilendo=yellow,bhote=green,micro=mauve`.
- The live test status file format is documented in [docs/monitors.md](docs/monitors.md).
- README: the full CLI and configuration tables live in `docs/` only.

### Removed
- The CircleCI token is no longer read from `~/.config/zsh/secrets.zsh`; use `bhote config set CIRCLECI_TOKEN …` or
  `$CIRCLECI_TOKEN`.

### Fixed
- Two changes to the same topic at once (panel, collector, an agent's CLI call) no longer lose one of them.
- Auto-assign goes on with the other projects when one project has no free agent.
- `bhote project pin <project> <agent>` pins the agent's repository (else its workspace), not the reference itself.
- No error text in the panel and no stale spinner or click targets when there are no agents.
- `bhote add -p <unknown project>` creates nothing.
- The search popup no longer asks herdr again when the agent lists are fresh but unchanged.
- Two merges with the data location can no longer run at once in one panel.
- `tests/cli.sh` no longer sends a real herdr notification.

## [0.5.8] - 2026-10-08
- The hotkey is written with the prefix and the key apart: Ctrl+B|T (Ctrl+Option+T on a Mac, Ctrl+Alt+T elsewhere).
- A click or Enter on an agent (or its topic) on another machine focuses it there and says which keys switch to that
  machine (herdr cannot be told to switch machines).

## [0.5.7] - 2026-10-08
- New: `bhote reload` restarts all bhote panels of a machine safely (it ends the old panel and only then starts the new
  one, and never types into a panel that still runs). Use it after an update; the `bhote-install` skill now says so.
- Fixes a way to start an unwanted Steal & Transfer: text typed into a running panel (by restarting panels from outside)
  is key input.

## [0.5.6] - 2026-10-08
- Review topics only for a real question: an agent that is blocked or ends its turn with a question gets
  "Question from <agent>"; a plain hand-back makes none.
- An agent does not revive what you parked or closed (`bhote now` / `bhote review` refuse later and done topics).
- Agent numbers: every agent has a number of its own on its machine for good (MAC4, OMR1), shown in the agent list,
  behind done topics, in `bhote agents` and usable as a reference.
- The agent list shows the topic an agent works on (seven characters); done topics show the machine code of the agent
  that did it; a running deployment says in brackets what goes live.
- Steal & Transfer is safer: a target of another project is refused, the panel asks before it starts, a pending
  hand-over can be cancelled.
- Keys are written as they are typed (Ctrl+B+T, Ctrl+Option+T on a Mac) after the keyboard machine's system
  (`HERDR_KEYS_MACHINE`).
- Jakob Beyer is co-author.

## [0.5.5] - 2026-10-08
- Auto-assign (`AUTO_ASSIGN`, off by default) chooses by project: a `now` topic goes to the next free agent of its
  project. A topic without a project gets it from a project name in its title or description (whole words, the longest
  name wins); without a project nothing starts.

## [0.5.4] - 2026-10-08
- An agent that hands its turn back without a topic gets a review topic; it goes when the agent works again. A topic in
  work goes to review and returns to now (`AUTO_REVIEW`, on by default).
- A done topic taken up again gets "Reopened <date>." at the start of its description.
- Topic page: a done topic has its own actions (reopen, up next, park, project, rename, describe, delete); the action
  numbers line up from 10 on.

## [0.5.3] - 2026-10-08
- Robustness: herdr calls that check or prompt an agent time out (also on another machine), and a herdr that does not
  answer is 'unknown' instead of 'gone'.

## [0.5.2] - 2026-10-08
- Agent list: sorted inside each project (working in Claude orange, needs you in red, idle in grey), coloured project
  captions (`PROJECT_COLORS`), air between sections.
- Done, Deployments and Tests as grey captions, seven rows and "+N more", progress bars for running entries.
- New: "New Features Available · w Start Wizard" (the wizard offers only the new steps); update notice in Settings and
  the head (`bhote update`, `UPDATE_CHECK`).
- New: `HERDR_KEYS_MACHINE`, `CLAUDE_CONFIG_DIR` for the Slack watch and the wizard.
- New: skill `bhote-install` and `bhote skills install`.
- Hardening: the Slack watch runs Claude without tools and without your allow rules; test status files are sanitised;
  the sync merges only differing topics; remote collection and gh calls have timeouts; sync lock in the runtime dir.
- Omarchy bar widget: shows only the number; the small dog and the `ANIMATION` setting are gone.

## [0.5.0] - 2026-10-08
- Omarchy bar widget (`bhote.bar`): the number of things for you, the list on hover, a click brings the bhote panel.
- Machine codes (MAC, OMR) in front of agent names when more than one machine is listed.

## [0.4.0] - 2026-10-08
- Projects: topics and agents belong to a project (repository, worktrees, `-p`), the agent list is grouped by project,
  `TOPICS_SCOPE=project`, `bhote project …`, `bhote take`.
- GitHub: the git branch a topic's agent works on.
- Enter on a topic jumps to its agent in herdr (also in another tab); `→` opens the topic's page.
- Settings: a Connections page; wizard navigation (change, skip, back, abort).
- Machines that do not answer stay listed as seen last, muted.

## [0.3.0] - 2026-10-08
- Slack pins: a waiting topic can carry a Slack channel, DM or thread; with `SLACK_WATCH` a message from someone else
  puts it under review.
- Done topics in their own area above the agents; bored agents when nothing is open.

## [0.2.0] - 2026-10-08
- Setup wizard, settings API (`bhote config`), status `review` and `next`, `AGENT_CAN_CLOSE`.
- Sync of the topics through herdr (`SYNC_VIA=herdr`), pushed agent states, remote agents.
- herdr integration: delegation through `herdr agent prompt`, the agent-status hook, `bhote current` for SessionStart.
- Steal & Transfer, themes, search popup (`bhote find`), jump key, auto-assign, topic page with numbered actions.
- API docs and JSON schemas.

## [0.1.0] - 2026-10-07
- First release: topics and agents next to herdr.

[Unreleased]: https://github.com/floriankappert/bhote/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/floriankappert/bhote/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/floriankappert/bhote/compare/v0.5.8...v0.6.0
[0.5.8]: https://github.com/floriankappert/bhote/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/floriankappert/bhote/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/floriankappert/bhote/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/floriankappert/bhote/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/floriankappert/bhote/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/floriankappert/bhote/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/floriankappert/bhote/compare/v0.5.0...v0.5.2
[0.5.0]: https://github.com/floriankappert/bhote/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/floriankappert/bhote/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/floriankappert/bhote/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/floriankappert/bhote/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/floriankappert/bhote/releases/tag/v0.1.0
