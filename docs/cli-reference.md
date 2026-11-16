# CLI reference

The `bhote` command is the API. The panel, scripts and coding agents all work on the same tasks through it: run it with no
arguments for the panel, with a command for everything else. Every command can answer in JSON, every result has a defined
exit code, and messages never mix with output.

```sh
bhote add "Release notes" -d "Collect the changes since 1.4" --json
```

```json
{"id":"1791399300-29508","n":1,"title":"Release notes","description":"Collect the changes since 1.4","status":"now","waiting_for":null,"waiting_since":null,"agent":null,"created":1791399300,"updated":1791399300}
```

**On this page:** [Conventions](#conventions) · [Task refs](#task-refs) · [Commands](#commands) ·
[JSON output](#json-output) · [Exit codes](#exit-codes) · [Environment](#environment) · [Using bhote from an agent](#using-bhote-from-an-agent)

## Conventions

| | |
|---|---|
| Output | Results go to **stdout**: human text, or JSON with `--json`. Messages, warnings and errors go to **stderr**. |
| `--json` | Accepted by every command, at any position. One JSON document per call, ended by a newline. |
| Stability | The JSON shape is [versioned by its schema](#json-output). Fields are only added; a removed or renamed field is a new major version. The human text may change at any time; do not parse it. |
| Sync | Every changing command writes locally first. With a [replica machine](data-and-sync.md) it then merges at once (best effort): when the machine is unreachable, the change stays local, a warning goes to stderr and the exit code is still `0`. |
| Input | Titles and descriptions are single lines. Control characters are removed; descriptions are cut to 140 characters (with a warning). |

## Task refs

Commands that act on one task take a `<ref>`, resolved in this order:

1. **A number** (up to three digits): the position in `bhote list --all`, decimal (`07` is 7). A number that does not exist
   is an error; it never falls back to a title search.
2. **An id**, exactly, or **the start of an id** when that start is unique.
3. **A part of the title**, case-insensitive (also for non-ASCII letters).

When a ref matches more than one task (by the start of the id or by the title), nothing changes: the candidates are
listed on stderr and the exit code is `2`. With a replica machine, a ref that matches nothing locally makes bhote merge once
and look again (a task handed out on the other machine).

```console
$ bhote show re
bhote: 're' matches 2 tasks, be more specific:
  2  Release notes
  3  Toast redesign
```

Numbers shift when tasks change. For anything that runs later (a script, an agent's next step), keep the `id`.

## Commands

### `bhote add <title> [options]`

Creates a task. Words without a leading dash form the title; a title that starts with a dash goes after `--`
(`bhote add -- -x flag`). An unknown option is exit `1`, and nothing is created.

| Option | |
|---|---|
| `-d`, `--desc <text>` | Description, at most 140 characters (longer text is cut, with a warning). |
| `@Name`, `-w`, `--waiting <Name>` | The task waits for Name; `waiting_since` is now. |
| `-s`, `--status <now\|testing\|next\|waiting\|review\|later\|done>` | Initial status (default `now`, or `waiting` with a name; `waiting` needs a name). Another value is exit `1`. |
| `--source <agent\|stash\|linear\|manual>` | Where the task comes from (default `manual`; a task an agent makes is `agent`, with its name as the reference). Another value is exit `1`. |
| `--source-ref <ref>`, `--source-url <link>`, `--source-kind <kind>`, `--source-parent <ref>` | The reference in that system (`ABC-123`), a link (http or https only); kind and parent are legacy fields of the removed IMS source (still accepted, not used). |

```console
$ bhote add Price list approval @Alex
Price list approval
  status: waiting
  id: 1791399233-10847
  waiting for: Alex (just now)
```

Exit `1` without a title.

### `bhote task <command>` (and the old name `bhote topic <command>`)

The commands of this page under one name: `bhote task add ...`, `bhote task list`, `bhote task now <ref>` and so on are `bhote add ...`, `bhote list`, `bhote now <ref>`.
`bhote topic <command>` is the old name (before 0.9.4 tasks were called topics) and **stays for good**; on a terminal it adds a one-line hint on stderr.
Only `bhote task` has `sync`, `prune` and `from-stash`:

#### `bhote task sync [--source linear] [--dry-run|-n] [--json] [--quiet]`

Mirrors the Linear issues in triage into tasks (status `next`; one that leaves triage becomes `done`) (see [task sources](task-sources.md)). `--json`: `{"created", "updated", "done", "written_back", "not_changeable", "sources", "errors", "skipped"}`.
Exit `1` when a source failed (the others still ran); a source without a key in the secret store is *skipped*, not an error.
`--dry-run` (`-n`) fetches from Linear and decides exactly as the real sync, but **writes nothing**: no task file, no tombstone, no lock, no API cache entry. It prints, per source, how many tasks it would create, update, close or skip (with the reasons of the skipped ones: `not in triage`, removed here by hand), plus the candidates it would create or update (title cut to 80 characters, status). `--json`: `{"dry_run", "sources", "errors", "skipped", "totals", "by_source": {source: {would_create, would_update, unchanged, would_write_back, would_close, would_skip, skip_reasons}}, "candidates": [{action, source, title, status}]}`. Keys and tokens are never printed.

#### `bhote task triage <task> accept|decline|done [--priority urgent|high|medium|low|none]`

Decides a mirrored Linear issue in triage: `accept` assigns it to you (`--no-assign`: not), moves it to the default state of its team (and sets the priority), `decline` to the team's canceled state, `done` to its Done state. The task is marked done here. Works without `TASK_WRITE`.

#### `bhote task prune --source linear [--dry-run] [--yes] [--json]`

Deletes the mirrored tasks of that source that were not changed here (status moved by hand, own fields, an edited description stay; `--dry-run` lists why). Refuses without `--dry-run` or `--yes`. Deletion is `deleted=1` (a tombstone, as `bhote rm`), so it replicates through the store and kharka; the file also gets `pruned=1`, so a later sync can bring a task back whose item qualifies again. `--json`: `{"dry_run", "deleted", "kept"}`. Exit `1` for a missing or unknown `--source`.

#### `bhote task from-stash <stash id> [-s now|next|later]`

The entry of the stash (the dock of the panel) becomes a task with source `stash` and the stash id as its reference, and leaves the stash. Exit `1` for an unknown id.

### `bhote migrate tasks [--dry-run] [--yes] [--json]`

Wave 2 of the rename, **not run by anything else**: files `*.topic` become `*.task`; with `STORE=kharka` the entries `bhote.topics/<id>` become `bhote.tasks/<id>`. Idempotent (a task that
is a `.task` stays; a `.topic` next to a `.task` of the same id keeps the one with the newer `updated`). `--dry-run` lists the plan and changes nothing; without `--yes` the real run refuses.
Run it only when **every** machine has the version that reads both names, see [data and sync](data-and-sync.md#renaming-topics-to-tasks).

### `bhote list [--all] [-s <status>] [--source <source>]`

The tasks in list order: `review`, `now`, `testing`, `next`, `waiting`, `later`, then (with `--all`) `done`; within a group by creation. The numbers
are the [refs](#task-refs).

| Option | |
|---|---|
| `-a`, `--all` | Include done tasks. |
| `--source <source>` | Only the tasks of one source (also those whose source is derived). |
| `-s`, `--status <now\|waiting\|review\|later\|done>` | Only this status (done included when asked for). |

```console
$ bhote list
  1  now      Release notes
  2  waiting  Price list approval  (Alex, just now)
  3  later    Toast redesign
```

With `--json`: an array of [task objects](#json-output) (`[]` when there are none). An unknown option or status is exit `1`.

### `bhote show <ref>`

One task. With `--json`: one [task object](#json-output).

### `bhote now <ref>` · `bhote testing <ref>` · `bhote next <ref>` · `bhote review <ref>` · `bhote later <ref>` · `bhote done <ref>`

Set the status: start or resume (`now`, also clears a waiting name), tests are running (`testing`: bhote sets it by itself when the agent stops while a test suite runs), up next (`next`), hand over for review (`review`), park
(`later`), check off (`done`). Print the task.

When a **waiting** task belongs to an agent (the agent created it, see the bhote skill) and you set it to `now`, that agent
gets a prompt: "The wait is over (<who>): <title>. Go on with: <description>". `bhote now <ref> --quiet` skips the prompt.

`review` takes a **card** for the panel: `--summary <text>` (what was done, what is open, what only you can do; at most 240 characters; it is in
`bhote show` and in the Claude mod's pane, the panel's card does not show it) and `--look`, which adds the one button *Review ansehen*: it focuses the task's agent, sends nothing and the
task stays in review. There are no buttons that trigger an action, because an answer can be several things at once: `--ask <label>` (with an
optional `--reply <text>`) is still accepted, but the card shows the same jump button instead (a note on stderr). A card goes when the task leaves review.
`--json` shows it as `"review": {"summary", "actions": [{"label", "reply", "jump"}]}` (`null` without a card; `reply` is `null` and `jump` is `true`).

```sh
bhote review <id> --summary "Fix ready, tests green, nothing committed. Proposal: commit." --look
```

Run by an agent (a Claude Code session: `CLAUDECODE=1`), `review` and `done` send a herdr notification and record which
agent it was
(`CLAUDE_CODE_SESSION_ID`, `HERDR_PANE_ID`). With `AGENT_CAN_CLOSE=off` an agent cannot close a task: its `done` becomes
`review` (with a note on stderr, exit `0`), and you check it off. With `AGENT_TOPICS=off` an agent's `bhote add` creates
nothing (a note on stderr, `null` with `--json`, exit `0`).

### `bhote dismiss <ref>`

Closes the card of a task in review or recently finished (`status: finished`: its agent stopped without a question; `bhote list -s finished`)
(read, nothing to do): the task is parked (`status: later`, `dismissed: 1`) and no longer in the
panel's list; search and CLI still find it. When its agent works again it is back on `now` by itself (the agent's event, or the collector when
it saw the agent idle and then working), and with the agent already working it goes straight to `now`. Exit `1` for a task that is not in
review. Also key `c` and the task menu.

### `bhote wait <ref> <Name> [--slack <pin>]`

The task waits for Name from now on (`status: waiting`, `waiting_since` = now). Exit `1` without a name. With `--slack`
it also pins Slack (see below); a task that is pinned already waits again from now on (older messages do not count).

### `bhote wait <ref> --deploy[=Label]` and `bhote add <title> --deploy[=Label]`

The task waits for a deployment of its project (`status: waiting`, `deploy_wait: 1`, `waiting_for: Deployment`). The panel lists it
in its own group *Waiting for deployment*, as a card with the progress of that deployment (a bar, `44 of 56` jobs or steps as a
percentage). It binds to the newest deployment under way (or begun after the wait) of its project; `=Label` narrows it to one
(`Backend`, `Production`, the labels of the deployment monitor). When that deployment is over the task goes to `review` by itself:
done says *check it in production*, failed or canceled says so, and a notification comes. Needs the deployment monitor
(`DEPLOY_MONITOR=on`) on one machine. A task of an agent whose branch is the one a running deployment comes from joins the group
without a command. Any other status or `bhote wait <ref> <Name>` ends it.

### `bhote slack <ref> <pin | off>`

Pins a Slack channel, DM or thread on the task. `<pin>` is a Slack link (copy link of a channel, a DM or a message; a
message link, also of a reply, watches its thread), `#channel` or `@person` (their DM). `#` and `@` are looked up through
Claude Code (a few seconds); exit `1` when there is no clear match. `off` unpins. `bhote add … --slack <pin>` pins on a
new task.

The pin is watched while the task **waits**, by the machine with `SLACK_WATCH=on` (see configuration): every
`SLACK_EVERY` seconds bhote asks Claude Code (`claude -p`, Haiku, only the read-only tools of its Slack connector; bhote
keeps no Slack token) for newer messages. A message from someone else sets the task to `review`, with a herdr
notification; the message (sender and up to 1500 characters, all messages of one check together) is in `slack.message`
and on the task page. While the task stays in that review, the pin is still watched: further messages (a sentence that
goes on in a second message, a reply from someone else) join it, with a notification each. Your own messages only move
the watch on.

### `bhote take <ref>`

Run by an agent (in herdr): it takes the task over, it becomes the task's agent (its session, pane, machine and name),
status `now`. `bhote now <ref>` run by an agent does the same. Exit `1` outside an agent.

### `bhote project [list]` · `add <name>` · `rename <ref> <name>` · `rm <ref>` · `pin <ref> <what>` · `unpin <ref> <key>` · `set <ref> <field> <value>` · `of [folder]`

Projects group the agents (and the tasks). An agent belongs to a project by its git repository, known by its key: the
origin URL as `host/owner/name` (`git@github.com:Acme/Shop.git` → `github.com/acme/shop`) or `dir:<folder>` without a
remote; a worktree has the key of its main repository, so it inherits the project. Outside a repository an agent belongs
to a project by its herdr workspace (`ws:<name>`). `pin` takes a folder, an agent (`bhote agents`: its repository, else
its workspace), `ws:<workspace>` or a key. `of` prints the project of a folder (default: here). Projects are records in
the task store and travel with the tasks; `--json` on `list` gives `[{"id", "n", "name", "repos", "workspaces"}]`.
`set` writes one field of a project: `repos` and `workspaces` (comma-separated lists, as `pin` keeps them), and its CI and runbook: `ci` (`github:owner/repo` or `circleci:gh/owner/repo`, empty for none), `deploy_branches` (`develop=Staging,main=Production`), `deploy_names`, `test_names` and `runbook` (the stages after the merge, comma-separated; `none`: the path ends at the merge; empty: the default for its CI); the values are described in [monitors](monitors.md).
`bhote add … -p <project>` gives a task its project; a task an agent makes gets the project of the agent's repository.

### `bhote rename <ref> <title>`

New title. Exit `1` without a title.

### `bhote desc <ref> [<text>]`

New description, at most 140 characters. Without text the description is removed.

### `bhote rm <ref>`

Deletes the task. It stays as a tombstone (`deleted=1`) so that the deletion reaches a replica. Prints `deleted`, or with
`--json`:

```json
{"deleted":"1791399300-29508"}
```

### `bhote sync`

Merges with the [data location](data-and-sync.md) now.

| Result | Text (stdout / stderr) | `--json` | Exit |
|---|---|---|---|
| Data is local only | `data location is local, nothing to merge` | `{"sync":"local","machine":null}` | 0 |
| Merged | `in sync with <machine>` | `{"sync":"ok","machine":"<machine>"}` | 0 |
| Another panel is syncing | message on stderr | `{"sync":"busy","machine":"<machine>"}` | 1 |
| Machine not reachable | message on stderr | `{"sync":"offline","machine":"<machine>"}` | 1 |

### `bhote config [list]` · `config get <KEY>` · `config set <KEY> <value>` · `config unset <KEY>`

Read and change the [settings](configuration.md). `set` checks the value against the setting's type (`on`/`off`, a number in
range, one of a list) and refuses anything else with exit `1`; `unset` goes back to the default.

```console
$ bhote config set AGENT_CAN_CLOSE off --json
{"key":"AGENT_CAN_CLOSE","value":"off","default":"on"}
```

`config list --json` is an array of `{"key", "value", "default", "type", "description"}`.

### `bhote secret <set | get | ls | rm | rename | verify | scan | import | purge-context | machines>` · `bhote env <ls | set | unset | rm | bind | unbind | which | run>`

The secret manager and named environments; see [secrets](secrets.md). `--json` works on `ls`, `get`, `scan`, `import`, `verify`, `purge-context`, `machines` and `env ls`. Values are never printed (masked: first 6, last 3
characters and the length) except by `get --reveal`, which is refused inside a Claude Code session. `bhote env run [NAME] -- cmd` replaces itself with `cmd` and gives it the variables. Exit codes: `0` ok, `1` error
(also `verify` when a secret is not readable or does not match).

### `bhote agents`

The agents herdr knows (this machine, and the other machines when `REMOTE_AGENTS=on`: herdr's saved ones, or the ones in
kharka with `AGENTS_VIA=kharka`), numbered. Every agent also has a
name of its own from Middle-earth (`Frodo`, `Sam`, `Merry` … 60 short names, then `Frodo2`), the same on every machine: it follows from the rank of the agent's record among all agent records (by the time it was made), shown on the agent card in the herdr-app and as `ring` in `--json`; and the older name, herdr's workspace and pane number, `w3:a1` for the pane `w3:p1` (with the machine's code in front on another machine,
`MAC/w3:a1`; it is shown in the agent list and behind done tasks). The older code, a machine code and a number kept for good (`MAC4`, `OMR1`),
still works as a ref and is the `alias` in `--json`. The numbers, the names (`frodo` too, in any case) and the codes are agent refs for `transfer` and `jump`; a name that two
machines have (`w3:a1`) asks for the machine (`MAC/w3:a1`). `--json`: an array of `{"n", "code", "alias", "machine", "pane", "status", "name", "task", "free", "topic"}`.

### `bhote jump <machine> <pane> | <agent>`

Shows that agent in herdr: an agent of this machine is focused, one of another machine is focused there (through kharka or herdr; to go there is
up to you, as with a click in the panel). The [Claude Code mod's](claude-mod.md) pane calls it when you press one of its items.

### `bhote transfer <from> <to> [--topic <ref>] [--force]`

[Steal & Transfer](steal-and-transfer.md): `<from>` commits its work on its branch with a handover and hands it to `<to>`
(a free agent). Agent refs: a number, a name (`w3:a1`) or a code (`MAC4`) from `bhote agents`, `machine/pane`, a pane id, or a part of the name
(exit `2` when that matches several). A target that belongs to another project than the task is refused; `--force` does it anyway. The source must not wait for an answer, the target must be free. Prints the task. Exit `1` when
herdr cannot send the prompt; nothing changes then.

### `bhote handover <topic> --to <machine/pane> --branch <b> --commit <c>`

Run by the source agent of a transfer (its prompt contains the exact command): the task goes to the target agent, which
gets the branch, the commit and the instruction to go on. Exit `1` when no transfer is pending for the task, when `--to`
names another agent than the transfer, or with a second task word (quote `--to` when a machine name has spaces).

### `bhote current`

The task of the agent that asks: matched by its Claude session (`CLAUDE_CODE_SESSION_ID`), else by its herdr pane
(`HERDR_PANE_ID`). Prints one line with the task, its id and how to report back (`--json`: the task object, or `null`);
nothing when there is none. `--json` adds `progress` (the [Claude Code mod](claude-mod.md) draws it): `tests`, the suites running in the worktree it is called from (`{suite, done, total}`, the live status files, three at most) and `deploy`, the deployment its task waits for (`{project, label, state, done, total, url}`, or null). `--json --full` adds `context` for the mod's pane: `pr` (the newest pull request of the agent's branch: `number`, `title`, `state`, `draft`, `url`, `checks` with `total`/`success`/`failed`/`pending`; read through `gh`, kept for a minute) `siblings` (the other open tasks of the same project, at most 8: `n`, `title`, `status`), and `lists`: the cards of the panel (`review`, `now`, `testing`, `deploying` = waiting for a deployment, `finished`), each `{total, items}` with the other tasks of that status, newest first (at most 6, review 8, finished 5), `updated`, `n`, `title`, the `agent` and its `code` (`MAC4`). The tasks are those of every machine the store reaches (kharka, herdr), as in the panel. Next, waiting, later, done and archived tasks are not in it. Meant for a Claude Code `SessionStart` hook, so that an agent knows what it works on:

```json
{"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "bhote current"}]}]}}
```

### `bhote reminder`

A random task that waits, is next or is parked, as one JSON line (`id`, `title`, `description`, `status`, `waiting_for`): what the [Claude Code
mod](claude-mod.md) shows as its friendly reminder. Not a wait for a deployment, not an automatic card, not a dismissed one, not
`Waiting for New Work`. `{}` when there is none, `{"disabled":true}` with `FRIENDLY_REMINDER` off.

### `bhote event`

Called by the [herdr plugin](herdr-plugin.md) on `pane.agent_status_changed` with the event in `HERDR_PLUGIN_EVENT_JSON`
(or on stdin). When the agent of a running task stops working (`done`/`idle`, at least 5 s after it got the task) and has
not reported back itself, the task goes to `review` and a herdr notification says so. Every event also makes the panels
refresh at once.

### `bhote deploys` · `bhote tests`

The entries of the [deployment and test monitor](monitors.md), newest first (`--json`: `time`, `project`, `label`,
`state` running|queued|approval|ok|failed|canceled, `done`, `total`, `note`, `url`). Fetched first when older than
`DEPLOY_EVERY` / `TEST_EVERY`.

### `bhote test run [-n <name>] -- <command>`

Runs the command as it is (output and exit code unchanged) and records the run for the test monitor, with the result line
of Minitest, Vitest, RSpec, pytest or bhote's own tests.

### `bhote refresh`

Makes every entry of the shared API cache due again, so the next cycle of the daemon, the CLI and the panels asks GitHub and
CircleCI at once ([API cache](monitors.md#api-cache)).

### `bhote monitor detect [<project>]` · `bhote monitor check`

`detect` finds each project's CI from its repository on GitHub (GitHub Actions deploy workflows, or CircleCI deploy jobs and
their branches) and writes it into the project. `check` fetches both monitors now and prints them.

### `bhote bar`

One line for a status bar, Waybar-style JSON: `text` (the number of things for you; empty when nothing waits), `tooltip`, `class`
(attention, busy, idle) and the counts (`review`, `blocked`, `busy`, `now`, `waiting`, `finished`: tasks recently finished). The Omarchy bar widget `bhote.bar` shows it.

### `bhote find`

Search all agents (every machine) and tasks: type to filter (every word must appear), `↑↓` choose, `⏎` jump, `Esc`
close. `x` checks the selected task off (while nothing is typed; else `Ctrl-X`). `Ctrl-N` adds a task, starting with the search text (`Title; description @Name` as in the panel); `⏎` makes it at
once from the search text when nothing matches. The new task is then selected. An agent is focused in herdr (on another machine it is selected there; switch to that machine in herdr). A task opens
its page (title, description, status, actions) in the bhote panel of the tab, and the panel gets the focus. Meant for a herdr popup: settings ›
*Find key* (`FIND_KEY`) binds it. Needs a terminal.

### `bhote version` · `bhote help`

`bhote version` prints `bhote 0.6.0` (`--json`: `{"version":"0.6.0","build":"0.6.0"}`; from a checkout past the release tag both
the text and `build` say the local build, `0.9.3.1 Preview`); also `--version`, `-V`. `bhote help` (`-h`,
`--help`) prints the short usage, with `bhote event` and `bhote setup` among the commands.

### `bhote skills [install]`

Puts the Claude Code skills `bhote` and `bhote-install` into every Claude profile (`CLAUDE_CONFIG_DIR`, `~/.claude`,
`~/.claude*`) and says to continue with `/bhote-install`. From a git checkout each skill is a link into it (it stays current
with every `git pull`; an earlier copy that holds only `SKILL.md` becomes a link), from a package a copy.

### `bhote kharka watch [install|uninstall|status]`

For machines with [kharka](data-and-sync.md#through-kharka) and `herdr-kharka`. One look (`bhote kharka watch`, no argument): when the machine's entry
`herdr/<node>` in kharka was not written for `KHARKA_WATCH_STALE` seconds (90) while the kharka daemon is online, `herdr-kharka` is restarted (launchd
`dev.kharka.herdr` on macOS, the systemd user unit `herdr-kharka` on Linux); when the daemon itself is offline twice in a row, it is restarted too. A
restart happens at most once in three minutes, and each one is written with its reasons (age of the entry, `pending`, the age of the other machines'
entries) to `kharka-watch.log` in the data folder. `install` sets up a timer that does this look every minute (launchd `dev.bhote.kharka-watch`, or a
systemd user timer `bhote-kharka-watch.timer`), `uninstall` removes it, `status` says how old the entry is and whether the timer is there. Why: a
`herdr-kharka` that lives on while it writes nothing is not noticed by the service manager, nor by a check that only asks the daemon whether it is online.

### `bhote mod [install|remove|status]`

The [Claude Code mod](claude-mod.md) `bhote-topic` (the task as a band above the prompt, `/task`) in every Claude profile:
`install` (the default; again after a bhote update) adds `integrations/claude` as the marketplace `bhote` and installs the plugin,
`remove` takes both away, `status` says where it is installed. Needs `claude` (Claude Code 2.1.287+) on the PATH; exit 1 when
a profile failed.

### `bhote update`

Asks GitHub whether a newer version is out and prints the command that updates this installation (brew, `git pull`,
pacman). The panel also says so, checked every 6 hours (`UPDATE_CHECK`).

### `bhote app install` · `bhote app check`

The add-on for the herdr Mac app ([the add-on](herdr-app.md)). `install` copies the mod (`herdr-app/mods/bhote`) and the layout (`herdr-app/layouts/bhote.json`) from this checkout or from the package
into `~/.config/herdr-app` (`HERDR_APP_HOME` picks another folder), then loads the mod once (`herdr-app check`) and says how to show the layout. `check` only loads it. Exit `1` when the files are not here or the check fails.

### `bhote send <agent> <text...> [--title <t>] [--from <name>]`

A message for another agent. `<agent>` is any agent ref (its name `Frodo`, a number, `machine/pane`). The message is kept in kharka (`msg/<kharka node>/<name of its space, letters and digits in lower case>/<id>`, state `queued`), and the herdr-app hands it to the agent, as a one-line prompt that says where to read it (`kharka get <key>`, the field `text`), as soon as the **ear** on the agent's card is on and the agent is free (`idle` or `done`: not working, not asking). With the ear off, or while the agent works, it waits; the card shows how many wait next to the ear. Afterwards the record says `delivered`. The sender is the agent the command is called from (its terminal, `HC_TERMINAL_ID`) or the agent `--from` names (or `$BHOTE_FROM`); the receiver sees its name and answers with `bhote send <that name>`. A message from the person (no agent behind it) says `the user` and is answered in the receiver's own session. The target and the sender are found by their records, so it works on a machine that has no list of agents (`bhote agents` is empty without herdr-app); a space called `~` is `home` in the key.

### `bhote inbox [--hook <event>] [--peek] [--json] [--as <node>/<slug>]`

The messages that wait for **this** agent (found like the panel does: the kharka node of the machine and the name of its space, from `HC_TERMINAL_ID`), each claimed in kharka (state `delivered`) before it is printed; `--peek` does not claim, `--json` prints an array. Nothing when the ear is off. `--hook UserPromptSubmit|PostToolUse` prints the Claude Code hook JSON (`systemMessage`, `hookSpecificOutput.additionalContext`) and nothing else; other events print and claim nothing. At most 5 per call. `--mark [<node>-<slug> ...]` replaces the marker files `~/.cache/bhote/inbox/` (the panel calls it). The hook script and its settings: [claude-mod](claude-mod.md#messages-between-agents-the-inbox-hook).

### `bhote reload`

Restarts the bhote panels of this machine, for example after an update. It ends each panel itself and starts bhote again
only in a pane that is a plain shell afterwards; it never types into a pane that still runs something. Prints how many
panels it restarted and how many it left alone.

### `bhote setup [--json]`

The [setup wizard](setup.md) in this terminal. With `--json` it only reports what is in place:
`{"machine", "jq", "herdr", "plugin", "claude_hook", "machines": [...], "setup_done"}`. A machine is
`{"label", "via": "herdr", "reachable", "knows_this_machine"}` (herdr's saved machines) or, with `AGENTS_VIA=kharka`,
`{"label", "via": "kharka", "node", "reachable", "prompts"}` (`prompts`: its herdr-kharka runs `agent.prompt`).

### `bhote` (no arguments)

Starts the [panel](panel.md). Without a terminal on stdin (or with `BHOTE_ONCE=1`) it prints one frame and exits.

## JSON output

All task output uses one object, described by [`schema/task.schema.json`](schema/task.schema.json) (JSON Schema
2020-12); `list --json` is an array of them ([`schema/task-list.schema.json`](schema/task-list.schema.json)). The old names `topic.schema.json` and
`topic-list.schema.json` stay and point to the new files. A value that
is not set is `null`, never `""` or `0`. Times are Unix seconds.

| Field | Type | |
|---|---|---|
| `id` | string | Stable id (`<created>-<random>`). The same on every machine. |
| `n` | integer | Position in `bhote list --all` at the time of the call; a valid ref until tasks change. |
| `title` | string | Never empty. |
| `description` | string \| null | At most 140 characters. |
| `status` | `"now"` \| `"testing"` \| `"finished"` \| `"next"` \| `"waiting"` \| `"review"` \| `"later"` \| `"done"` | |
| `waiting_for` | string \| null | Only while `status` is `waiting`. |
| `waiting_since` | integer \| null | Only while `status` is `waiting`. |
| `agent` | object \| null | The agent of the task: `{"name", "machine", "pane", "session", "branch"}` (each may be null; `branch`: the git branch it works on, `@1234abc` when detached). |
| `project` | string \| null | The task's project (its name). |
| `source` | `"agent"` \| `"stash"` \| `"linear"` \| `"manual"` | Where the task comes from. Derived (not stored) for tasks made before sources existed or with a legacy value (`ims`). |
| `source_ref` | string \| null | The reference there: `ABC-123`, the agent's name, the stash id. |
| `source_url` | string \| null | A link to the item (http or https). |
| `source_kind` | `"task"` \| `"work_item"` \| null | Legacy (removed IMS source), still read. |
| `source_parent` | string \| null | Legacy (removed IMS source), still read. |
| `slack` | object \| null | A Slack pin: `{"channel", "thread", "label", "message"}`; `message` is what came from others (senders: text), while in review. |
| `created` | integer \| null | |
| `updated` | integer \| null | Last change; the newer one wins in a merge. |

```json
{
  "id": "1791399300-15763",
  "n": 2,
  "title": "Price list approval",
  "description": null,
  "status": "waiting",
  "waiting_for": "Alex",
  "waiting_since": 1791399300,
  "agent": null,
  "created": 1791399300,
  "updated": 1791399300
}
```

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done. Also when a change was saved locally but the replica could not be reached (see stderr). |
| `1` | Error: unknown command or option, missing argument, no task for the ref, `sync` failed or is busy. Nothing changed. |
| `2` | The ref matches more than one task. Nothing changed; the candidates are on stderr. |

## Environment

| Variable | Default | |
|---|---|---|
| `BHOTE_CONFIG` | `$XDG_CONFIG_HOME/bhote/config` | The [settings file](configuration.md). |
| `BHOTE_DATA` | `$XDG_DATA_HOME/bhote` | Tasks (`topics/`, the folder keeps its name), sync state and lock. |
| `BHOTE_SHARED` | `$XDG_RUNTIME_DIR/bhote-$UID` (else `$TMPDIR`) | What the panels of one machine share: agent lists, the collector lock. Must belong to the user. |
| `BHOTE_NO_SYNC` | | Set: changing commands do not merge with the replica. |
| `BHOTE_REMOTE_DIR` | `.local/share/bhote/topics` | The replica folder on the other machine, relative to its home. |
| `BHOTE_SSH` | `ssh` | The ssh command (the tests use a fake one). |
| `NO_COLOR` | | Set: no colours. `BHOTE_FORCE_COLOR` forces them without a terminal. |
| `BHOTE_ONCE`, `BHOTE_VIEW`, `BHOTE_COLS`, `BHOTE_ROWS` | | Print one frame of a view (`splash`, `main`, `settings`, `hotkeys`) at a given size, then exit. |
| `BHOTE_SOURCE_ONLY` | | Load the functions without running (for tests). |
| `BHOTE_MIN_COLS`, `BHOTE_MAX_COLS`, `BHOTE_MIN_ROWS` | `30`, `60`, `20` | Size limits of the panel. |

## Using bhote from an agent

A coding agent (or a chat) can keep its tasks in step with the panel:

```sh
id=$(bhote add "Migrate the billing export" -d "CSV to Parquet" --json | jq -r .id)   # keep the id, not the number
bhote wait "$id" "DevOps" --json                                                       # blocked on someone
bhote now "$id" && bhote done "$id"                                                    # resumed, finished
bhote list --json | jq -r '.[] | select(.status == "waiting") | "\(.title) (\(.waiting_for))"'
```

Check the exit code: `2` means the ref was ambiguous, so pass the `id` instead.
