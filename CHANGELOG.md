# Changelog

All notable changes to bhote. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and bhote
uses [semantic versioning](https://semver.org/). Each release's section is also its GitHub release text.

## [Unreleased]

### Changed
- The small views of Deployments and Tests (counts only): `failed` has a red ground and light letters, a space on both sides (` ✗ 2 failed `), and when something runs the `✶` dances like a working agent's.
- Test and deployment bars are small now: five boxes and the percentage (`client ■■■□□ 19%`, the suite's name in front, cut to eight characters), on the right of the second and third row of the card (beside the task, the live text or the branch), under the status. A card no longer needs a row of its own for them (only a card without an agent still has one).
- The ground of the active card is darker (a little lighter than the panel, no longer a light grey). The agents of another machine have their machine name in blue (this machine's in herdr's machine colour, pink), also on idle cards. Idle cards are less muted: the text in the dim grey, not the darkest one.
- Claude Code mod: the band above the prompt no longer shows the status of the agent's topic. It shows a **friendly reminder** instead: now and then, at random (at the earliest every ten minutes, for 45 seconds), in orange with a dog, a topic that waits, is next or is parked, with its description under it. New setting `FRIENDLY_REMINDER` (settings › *Show Friendly Reminder*, on); new command `bhote reminder`. The pane and `/topic` stay. After `bhote mod install` Claude Code needs `/reload-plugins`.

### Fixed
- An agent is always there: when the last topic of an agent was set to done, its card was gone and nothing showed the agent (and herdr's `done` state, a finished agent, never had a card). Every agent without a card of its own now has one (ungrouped: idle, done, working, needs you; grouped: idle and done in the group IDLE, the others in the list at the bottom as before). An agent that works or needs you shows in its colour, an idle one dimmed; the list at the bottom keeps only what is not on a card (offline machines) and no longer counts idle or done agents as inactive.
- A click on a card shows its agent at once (3 ms until herdr is asked, was about 200 ms): the Rust panel asks herdr itself for an agent of this machine (`herdr agent focus`, no script started), does not repaint for the click, and no longer waits half a second for a possible double click. A double click still works: the last click is kept in the shared folder, so the panel of the tab the first click went to takes the second. The panels notice a changed focus (`focus.pane`) within 150 ms, a click writes it at once: the highlight moves with the agent shown instead of after the next round of the collector (up to 3 s).
- A card that says `finished` says when, always: a now card whose agent has stopped had no age (`finished` alone, and `finished · x = done` on the chosen one). The age is there now (`just now · finished`), and `x = done` is gone.
- The highlighted card is the active agent's, one to one with herdr: the agent that herdr shows in the main area of the panel's tab (the focused pane when it is an agent of the tab, else the tab's first agent) has the highlighted ground on all its cards, an idle agent too (its idle card, not dimmed then); when you pick another agent in herdr the highlight moves with it (within a second or two). Before, the highlight was a card you had chosen, and could be another agent's.
- A topic titled `Waiting for New Work` (what an agent writes when it has nothing to do) is never a card, whatever status it was given (review, finished, now): it is put away and the agent shows as idle, in the group IDLE. The same words as the *task* of an agent (herdr's task) are no task either: the card of a topic shows the topic's title then, and an idle card has no task row.
- A recently finished card that repeats a card of the same agent (same title: at work, in review, waiting, or a newer finished one) is put away at once: the same work no longer shows twice (the remains of the card flood showed as a second `IMS (main)` card).
- The deployment monitor shows every deployment of a CircleCI pipeline, not only the ones whose job is in the stored names: IMS showed Backend only, because the detection had stored workflow entries (`release-and-deploy-frontend`, a name from the `workflows:` section, not a job of the runs) and a run was skipped when none of the names was one of its jobs. Now a workflow whose jobs match none of the names falls back to its first job with "deploy" in the name (`deploy-frontend` is Frontend). The detection no longer stores names for CircleCI (`bhote monitor detect` writes the new, empty names; the old ones are harmless now). A workflow that fails before its deploy job (the deploy job did not run) is a `failed` row, not nothing, and a failed or waiting workflow without any deploy job, in a pipeline that has no deploy row, shows once with the workflow's name (the setup workflow does not).
- When the tab of a panel comes back into view, a card chosen before is forgotten, so the highlight is the active agent's again (it stayed on the card of another agent you had clicked).

## [0.9.2] - 2026-10-19

### Added
- Setting `CARD_GROUPING` (settings › *Agent cards: grouping/sorting*): `ungrouped` (the new default) lists all agent cards in one list without group headings, in the order of the agents in herdr's agent panel (idle agents without a card too; cards without an agent after them); `status` keeps the groups (NOW, TESTING, DEPLOYING, RECENTLY FINISHED, IDLE) with their headings.
- A card shows the CI test jobs (CircleCI, GitHub Actions) that run for its agent's branch as bars, like local test runs (two at most; CircleCI jobs report 0 of 1 until they are over, so the bar fills when they finish).
- Panels no longer drift apart when you switch workspaces: herdr works the layout out again from the split ratios then, so a panel could come up narrower or wider than the others and, as the one in front, set that width for all. A panel whose size changed while its tab was hidden or just shown (and a new panel) now takes the width the panels have; only a drag of the panel in front sets a new one.
- The widths also go to the other machines: with kharka (`STORE=kharka` or `AGENTS_VIA=kharka`) a panel made wider or narrower sets the panels on every machine to that width (`bhote.width/<node>`). A panel started by hand (no label, no title) is found too: every panel registers itself in the shared folder.
- A topic whose agent has stopped (recently finished) but whose branch is being deployed moves to the DEPLOYING group (was: WAITING FOR DEPLOYMENT, and only for topics still at work); when the deployment is over it goes to review.
- All panels are as wide as the one you work in: when you make a panel wider or narrower, the panels of the other tabs and workspaces follow after a second (inside `PANEL_MIN_WIDTH` and `PANEL_MAX_WIDTH`), so a click lands on the same column everywhere. Only the panel of the tab in front leads.
- Rust daemon `bhoted` (step 1 of docs/rust-rewrite.md): keeps the topic index current for every panel (inotify on Linux), started by the lead collector, setting `BHOTED=auto|off`; on Linux it also collects this machine's agents (same files as before), looks at the other machines' agents through kharka (`AGENTS_VIA=kharka`) and fetches the deployment and test lists (same lists as before, four to ten times faster) and, with `BHOTED_FOLLOW=shadow|on`, ten follow-ups of the collector (shadow only compares; `tools/shadow-report.sh`), and draws the main view of the panel (`BHOTED_FRAME`: cards, lines, done area, monitors, the windows and the click maps, and the screen made of it; the same bytes as before, a redraw 130 ms -> 29 ms, a frame of navigation 99 ms -> about 17 ms); without the binary bhote works as before. The collector's and the dog's pauses no longer start a process (`nap`).

### Added (continued)
- Hardening of the daemon and the Rust panel: a fuzz test (`tests/fuzz.py`), guarded parts that survive a panic, time limits for every child process, restart of the collector and the daemon, a check of the shared folder, 0600 settings, size caps, `cargo audit` and clippy in the CI.
- The daemon merges the topics with kharka (`STORE=kharka`) and does the follow-ups `project_sync`, `tests_sync` and `deploy_sync` (`BHOTED_FOLLOW`); the topic's page and the chooser of free agents are drawn by the Rust panel.
- `BHOTED_PANEL=on`: the panel runs in the Rust daemon binary (`bhoted panel`): keys, mouse and the main view without bash. The topic's menu, settings, the wizard and the choosers still run as the script's views on the same terminal; the welcome screen with the dog is not in it yet. Off by default.

### Changed
- The active card (the highlighted one when you have not chosen another) is the card of the agent in the panel's own tab, the one you are working with, not the first card of the list.
- No hover on the cards and lines any more: the mouse reporting no longer asks for motion without a button (it flooded the panel and, on a fast scroll, lost the mouse-off, so a card stayed lit); clicks, the wheel and drags work as before.
- With more than one machine the cards of the other machines stand above those of this machine (ungrouped).
- A new group of cards, **IDLE**, between RECENTLY FINISHED and the Next lines: every idle agent that has no card of its own (nothing in review, now, testing, deploying or recently finished), two rows each (the agent, its last task); a click shows the agent in herdr. The list of agents at the bottom keeps only the others (working, needs you, offline); an idle agent is no longer counted there as inactive.
- On a card the age is on the left of the state (`5m ago · finished`) and a space follows the state; a review card says `review` (was `ready for review`).
- All cards of the panel (review, now, testing, deploying, recently finished) are drawn the same way and take far fewer rows: three (icon, machine · agent and the state with its age on the right; `∟` what it works on; `⎇` the branch), four when the agent is working (what it does now) or bars run. The frame rows above and below a card are gone; two cards are separated by one blank row. The description of a topic shows only under the card you have selected. Review and deploying cards show their agent like the others. The bars (deployment, tests) are indented like the text rows. Cards have no ground of their own: only the chosen one (and the one under the pointer) has.
- The question under a review card (the summary and the button) is gone, with the click on that button: the key `1` (`y`) still focuses the agent, the summary is in `bhote show` and in the Claude mod's pane.
- A redraw of the panel is faster in bash as well: `ui_load` no longer works out each topic's project unless the project scope is on (15 ms), the monitor block is kept for the frames of navigation, the daemon's blocks are found once per load.
- An agent is named by herdr's workspace and pane number, `w3:a1` for the pane `w3:p1` (`MAC/w3:a1` on another machine), in the panel, behind done topics, in `bhote agents`, in `bhote transfer` and in the mod's pane; the old code (`MAC4`, a machine code and a number) stays a ref and is the `alias` in `bhote agents --json`.
- Deployments say what went live behind their name, also when they went through (the branch of a merged pull request with its number, instead of `Merge pull request`), and test runs say which branch was tested (CI: its branch; local runs: the branch of the worktree). The result of a test run stays on the right.
- The panel is wider by default: 56 columns when the plugin opens it (was 44), the pane stays between 50 and 100 columns (`PANEL_MIN_WIDTH` 50, was 36; `PANEL_MAX_WIDTH` 100, was 60), and the list uses up to 100 columns. Open panels that are narrower are widened to the minimum; set `BHOTE_PANEL_WIDTH` (the plugin) or the two settings to go back.
- The list is for the mouse: a click on a card shows its agent, a double click opens its page (no `›` button, no ⋮ menu), a click on a line (next, waiting, later, done) opens its page. The arrow keys, `⏎`, `m` and the right-button menu are gone from the list.
- Done and Archived are one section: its last line says `✓ 42 done · ▤ 30 archived` (like `✶ 0 active · ○ 13 inactive` for the agents). The Archived section is gone; the small view of Done shows the same line.
- The lines (Next, Waiting, Later) sit at the bottom, docked above Done; the cards (review, now, testing, deploying, finished) are on top. The agent divider and the rule above the keys are dotted like the captions; the jump key at the bottom right is gone.
- The drag handle on the divider above the agents is gone; the ↕ button (counts only) stays. `AGENT_ROWS` still sets the area's height, in the settings file.

### Fixed
- A flood of cards under DEPLOYING (one card per old topic of the agent, and a new topic every few seconds while it deployed) is gone: a deployment is bound to one topic per agent (its newest), and an agent whose topic waits for a deployment gets no second automatic card (that card was bound again, and so on).
- A double click on a card (review too) opens its page: the click before it no longer shows the agent at once (that took the focus away from the panel, and the second click went to another pane); the agent is shown half a second later, unless a second click follows. (The Rust panel; the shell panel still shows the agent at once.)
- A topic of an agent that has finished is bound to a deployment of its branch only when it finished no more than ten minutes before that deployment began: an old topic of the agent (here a Commerzbank topic, reopened days ago) is not what is deployed.
- A topic that says it waits for a deployment is a card in DEPLOYING only while there is a deployment it follows (running, begun, or over and not yet seen). Without one (nothing deploys for its project or its branch) it is a line in the normal Waiting list, and the agent is not shown as deploying. The collector still looks for the deployment and binds the topic when it starts.
- A topic card under NOW whose agent has stopped (herdr idle or done) says `finished` in green and now has a green bar too (it was orange, like a working agent, and looked like a fourth one at work). The group is unchanged.
- A panel whose pane is too narrow (the panel of a workspace you just switched to, e.g. by a click on an agent card) is widened at once, after about 0.1 s instead of 1 to 2 s: the Rust panel no longer waits for its once-a-second tick or a one-second sleep, and a resize outside `PANEL_MIN_WIDTH` and `PANEL_MAX_WIDTH` is not waited out.
- One agent, one card on now/testing: when an agent had several topics there (a card for each piece of work, one session), the list showed two cards for it. Now the newest stays and the older ones are recently finished.
- An agent with several recently finished cards no longer reopens all of them when it works again: only the newest comes back on now (two cards for one agent in NOW).
- A topic that waits for a deployment and has a branch (its agent's) follows the deployment of that branch, no longer the newest one of the project; a binding made to another branch's deployment is ignored.
- macOS: the progress bars in the Deployments and Tests lists showed `��` where the shell counts bytes (no UTF-8 locale): the bar is no longer cut with `${bar:0:n}`.
- A panel whose terminal is gone (pane closed, process still alive) no longer spins at full CPU: input reads tell the end of input (`in_fill`/`in_get` return 2) from a timeout (1), and the panel cleans up and exits on it. New suite `tests/input.sh`.
- A working agent whose topics are all waiting gets a card on now (it works on something that has no topic); it goes when a topic of the agent is on now, testing, review or next.
- A "Question from …" review card now goes by itself when the question is settled without the agent working again (e.g. `/clear`, an empty prompt): the agent is idle and its pane no longer ends with a question. A pane on another machine: after `FINISHED_MINUTES`.

### Added
- Setting `CLAUDE_MOD` (default on; settings › *Claude Code mod*): off, the Claude Code mod draws neither the band nor the pane and asks bhote for nothing (`bhote current --json --mod` answers `{"disabled":true}`).
- `bhote jump <machine> <pane> | <agent>` shows an agent in herdr (a pane's item in the Claude Code mod presses it): this machine's agent is focused, another machine's through kharka or herdr. In the mod's pane every item has two lines (title; `∟` address and time) and is pressable; the card has a line above it, `This agent w3:a1`.
- `bhote kharka watch [install|uninstall|status]`: a watcher for `herdr-kharka`. When a machine's entry `herdr/<node>` in kharka is not written for 90 s (`KHARKA_WATCH_STALE`) while the daemon is online, it restarts `herdr-kharka` (the process can live on while it writes nothing, as on the Mac: its agents showed as `off-…`), and the daemon when it is offline twice in a row; at most once in three minutes, each time with its reasons in `kharka-watch.log`. `install` sets up a timer that looks every minute (launchd on macOS, a systemd user timer on Linux).
- The Claude Code mod can show the topic in a pane beside the chat (`/topic pane`): description, project, agent and branch, waiting and Slack pin, the pull request of the branch with its checks, the test and deployment bars, the review card with its buttons, the other topics of the panel's cards (review, now, testing, deploying, recently finished), and buttons for Review, Done, Next and Later. `bhote current --json --full` has a new `context` (`pr`, `siblings`, `lists`) for it.
- Projects can be switched off in the Deployments and Tests blocks, in the settings (Connections, under each monitor: one switch for every project with a CI): its entries and the local test runs of its worktrees are not shown. The captions have no checkboxes. Settings `DEPLOY_PROJECTS_OFF` and `TEST_PROJECTS_OFF`.
- Deployments follow CircleCI's logic: a deployment is a workflow run with a deploy job (one entry per workflow, labelled by the job: `IMS · Backend`), its progress bar counts the jobs of the workflow that are over (not `notify-*` jobs, not jobs that did not run), a dynamic config (`setup: true` + `continue.yml`) is detected through its continuation, and the jobs of a finished workflow are cached. An agent whose branch has the merged pull request waits for the deployment of `main` that carries it (`(#123)` in the commit title). `deploy_names` takes `job=Label` for CircleCI.
- **Claude Code mod `bhote-topic`** ([docs](docs/claude-mod.md)): a band above the Claude Code prompt with the agent's topic (title, description, status badge in the status colour, age that moves on) and `/topic review|done|now|next|later|testing|dismiss|wait <Name>|hide|show` without a Claude turn. `bhote mod [install|remove|status]` puts it into every Claude profile (`integrations/claude` is the marketplace `bhote`); the setup wizard and `/bhote-install` offer it. Needs Claude Code 2.1.287+. The band shows a progress bar for each test suite running in the agent's worktree and for the deployment its topic waits for; `bhote current --json` has a new `progress` field (`tests`, `deploy`) for it.
- Done, Deployments and Tests can be switched to a small view like the agents: the ↕ at the end of the caption (or a click on it) shows only `14 done`, or `x running · y failed` (failures among the last 10; a failure that a newer run of the same project and name made good counts as `✓ resolved`, and says so in the list, too). Settings `DONE_COMPACT`, `DEPLOYS_COMPACT`, `TESTS_COMPACT`.
- The testing card shows a progress bar for each running suite (two at most, the suite's name in front), like the deployment card, from the live status files of the agent's worktree.
- The panel shows whether it is the pane you are in: a green dot at the right of the head, and the head dimmed while another pane has
  the focus (read from herdr once a second when idle).
- A menu for a topic over the list, with the actions of its page: key `m`, the right mouse button, or the ⋮ button on the selected or
  hovered card. ↑↓/1-9/⏎ or a click run an action, esc closes.
- Done topics, and the lines of next, waiting and parked topics, are a step lighter under the pointer, like the cards.
- The last done topics above the agents are four by default, no longer ten (`DONE_MAX`).
- The agent overview can show counts only: the `↕` button at the right end of the divider above the agents switches between the list and
  `✶ 3 active · ○ 4 inactive` (`AGENTS_COMPACT`; `◉ n need you` appears when an agent waits for an answer). The `↕` lights up (orange, bold) under the pointer.
- **Recently finished**: a topic on *Now* whose agent stops without a question (nothing to check) moves to its own group *Recently finished*
  (status `finished`) instead of review. With a question, or an agent waiting for an answer, it still goes to review. When the agent works
  again the card is back on *Now*; after `FINISHED_MINUTES` (30, 0 = never) it is dismissed by itself (the card bhote makes for an agent
  without a topic is simply gone); key `c` (dismiss) or `x` (done), or the topic menu, ends it by hand. The card of an agent without a
  topic follows the same way (it used to vanish when the agent stopped).
- Dismiss a review card: key `c`, topic menu › *Dismiss: close the card*, `bhote dismiss <ref>`. The card leaves the list (search and CLI still
  find the topic); when its agent works again the topic is back on *Now* as the agent's work card, with the agent already working it goes
  straight there.
- A card of an agent at work shows what the agent does right now, like herdr's agent item: its third line, in orange (the star dances only
  in the first row). It is read from herdr's `now` token of every machine (also over kharka) and shows only while the agent works.
- Waiting for a deployment: `bhote wait <ref> --deploy[=Label]` / `bhote add <title> --deploy` (topic menu › *Waiting for the deployment*) puts a
  waiting topic into its own group *Waiting for deployment*, a card with the progress bar and percentage of the deployment it waits for (from
  the deployment monitor). It goes to review by itself when the deployment is over (ok: check production; failed or canceled: look at it), with a
  notification. A topic of an agent whose branch a running deployment comes from joins the group by itself. The bhote skill tells agents to use it.
- `AGENTS_VIA=kharka`: the agents of the other machines come from kharka instead of herdr's saved machines, so no machine
  needs ssh to another. [herdr-kharka](https://github.com/jakobbeyer/herdr-kharka) writes each machine's agents as
  `herdr/<node>` (listed under its `--label`); the collector watches them and looks at once when one changes. The live state of
  an agent there, a prompt and the focus are `herdr.commands` that herdr-kharka runs; prompts need its
  `--allow-method agent.prompt`. Settings › *Agents through*, the wizard's *Remote agents* step (it checks this machine's
  label and which machines take prompts), `bhote setup --json` (`via`, `prompts`).
- A click on an agent of another machine takes you there on a Mac (with `AGENTS_VIA=kharka`): herdr's API cannot switch its
  window to another machine, so bhote types herdr's own key for that agent's row (`focus_agent`, e.g. `prefix+ctrl+1..9`, or
  `[keys.indexed] agents`) into the herdr window, and herdr switches. The row follows herdr's agent list: this machine's
  agents, then each saved machine's, by workspace number. A panel that runs on the other machine (the Mac's window shows the
  Linux herdr) cannot type there: it asks through kharka (`bhote.jump/<node>`), and the Mac whose frontmost window shows that
  herdr (its `window_title` starting with `{hostname}: `) types the key. Needs the Accessibility permission for the terminal;
  without it, past row 9, or elsewhere the key line still says how to get there.

### Changed
- The machine name in an agent card has the colour herdr gives it in its agent rows (the row token `machine`, its `fg` from herdr's config.toml;
  pink `#ff3ea5` without one), no longer grey or a project colour. The orange third line, the description and the branch are indented like the rows above them
  (under the text of the task).
- *Review ansehen* (the button, or key `1`/`y`) shows the agent and the button goes: you see that you have used it (the note stays).
- A card on *Now* (and *Testing*) with an agent looks like herdr's agent card: the state's icon (working, needs you, done, idle),
  `machine · agent`, and under it `∟` what the agent works on, as herdr shows it (the topic's title when it has no task), then the
  description and the branch. The card keeps its coloured bar and block edges.
  What the icon does not say (handing over, needs you, offline, tests running) stays in one extra line.
- Run from a git checkout that is past the tag of its version (local work), the panel and `bhote version` call it by its local build number,
  `0.9.2 Preview` (`LOCAL_BUILD`; the fourth number counts the builds on); a release install says its version.
- After `/clear` (a new session in the pane) the topic of the pane follows the new session when it starts working; an event without a
  session id no longer hands the old session's topic over.
- A click on a topic selects it and shows its agent in herdr, like a click on the agent, and leaves the focus on the agent (its grey
  bar stays in herdr's list; `CLICK_FOCUS_AGENT=off` gives the focus back to the panel). Double click, →: its page.
- The actions on a topic's page follow its status: the natural next step comes first (*Back to now* in review, *Ready for review* on now,
  *The wait is over* when waiting, *Work on it now* for up next and parked), nothing offers the status the topic has, an agent on
  the topic can be shown (*Show topic*) and handed over, *Start with agent* only when there is none.

### Fixed
- Moving the selection with the arrow keys (and the pointer over the cards) was slow: every key press reloaded all topics and agents and
  rebuilt the agents' area, about 170 ms. Now a key that only moves the selection draws from what is loaded and keeps the agents' area
  (about 70 ms); the idle check reloads at least every 2 s.
- A review or finished card whose agent is gone (its session ended, its pane closed) closes by itself after 30 s, if herdr answers: a review
  or finished card is put away like a dismissed one (a card bhote made is deleted), a topic on now moves to *Recently finished*.
- A topic could stay on *Now* with an agent that had stopped: the stop event moved it to *Recently finished*, and the collector's look at a list of
  agents that was a few seconds old put it back on now (it saw "working"). A finished, dismissed or review card is now reopened only after the
  collector saw its agent idle first. And a topic on now whose agent has been idle for two looks (8 s apart, and the topic untouched for 60 s)
  moves to *Recently finished* (review with a question on the screen) even when no event came, also for agents on another machine.
- A review card goes by itself when the agent that asked works again after it was idle (a decision was made in its session). Until now only the
  agent's event did that, so a missed event (another machine, a restart) left the card open. A finished deployment's card and an answer in Slack
  stay.
- A card bhote makes for an agent without a topic (`AGENT_CARDS`) no longer disappears once someone works with it. `bhote current`
  hands it to the agent as its topic, the agent puts it under review, and when the agent then stopped, the card went with it:
  the review was gone, and the agent's next `bhote now` found no topic. A card that a command uses (`review`, `wait`, `now`,
  `desc` …) or that is moved off *now* is a topic of its own from then on.
- The setup wizard's topics step and its summary work under bash 3.2 again (macOS `/bin/bash`): both stopped with
  `syntax error near unexpected token 'newline'`, because bash 3.2 ends a `$( … )` at the `)` of a `case` pattern. The
  patterns there are written `(pattern)` now.

## [0.8.0] - 2026-10-11

### Added
- A status of its own, `testing`: while a test suite runs in an agent's worktree (read from the live status files in
  `TEST_STATUS_DIR`, also a run in the background) the agent's topic is a card in *Testing*, whether the agent works or not. When the
  tests are done it goes to now (the agent works) or to review (the agent is idle). `bhote testing <ref>` sets it by hand. `TESTS_BUSY` is on by default now (a testing agent counts as busy).

### Changed
- Only *Review*, *Now* and *Testing* topics are cards. *Next*, *Waiting* and parked (*Later*) topics are lines grouped by status, like
  the done ones. Every group shows ten lines, then a selectable `+N more …` row (arrow keys; Enter, `→` or a click opens it,
  the row then reads `− show less`). `DONE_MAX` is `10` by default.
- *Deployments* and *Tests* show the last four entries by default (`MONITOR_ROWS`, was 7).
- The bhote skill starts with the rule *topic back on `now` when the user answers after a review*, as the first step of a
  turn instead of a section further down, where it was overlooked.

### Fixed
- `bhote now <ref>` run by an agent no longer takes over a topic that belongs to another agent (that left two cards for one
  agent in *Now*); `bhote take <ref>` does that on purpose.
- A topic that an agent handed back with `bhote review` returns to *now* by itself when the agent works again (the user
  answered). Until now only topics that bhote had put under review did; the others stayed in *review* while the agent
  worked, unless the agent remembered to run `bhote now`, so *Now* showed fewer agents than were running.
- After `/clear` the pane runs a new session; an event without a session id no longer hands the old session's topic over (it
  went to *review* for a moment). A pane alone only matches a topic with no `agent_session` on record.

### Added
- `ARCHIVE_DAYS` (default `7`): done, next, waiting and parked topics untouched for that long are archived by themselves: gone
  from the panel (a line says how many), still found by the search; touching one brings it back. `0`: never.
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

[Unreleased]: https://github.com/floriankappert/bhote/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/floriankappert/bhote/compare/v0.7.0...v0.8.0
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
