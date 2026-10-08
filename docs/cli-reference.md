# CLI reference

The `bhote` command is the API. The panel, scripts and coding agents all work on the same topics through it: run it with no
arguments for the panel, with a command for everything else. Every command can answer in JSON, every result has a defined
exit code, and messages never mix with output.

```sh
bhote add "Release notes" -d "Collect the changes since 1.4" --json
```

```json
{"id":"1791399300-29508","n":1,"title":"Release notes","description":"Collect the changes since 1.4","status":"now","waiting_for":null,"waiting_since":null,"agent":null,"created":1791399300,"updated":1791399300}
```

**On this page:** [Conventions](#conventions) · [Topic refs](#topic-refs) · [Commands](#commands) ·
[JSON output](#json-output) · [Exit codes](#exit-codes) · [Environment](#environment) · [Using bhote from an agent](#using-bhote-from-an-agent)

## Conventions

| | |
|---|---|
| Output | Results go to **stdout**: human text, or JSON with `--json`. Messages, warnings and errors go to **stderr**. |
| `--json` | Accepted by every command, at any position. One JSON document per call, ended by a newline. |
| Stability | The JSON shape is [versioned by its schema](#json-output). Fields are only added; a removed or renamed field is a new major version. The human text may change at any time; do not parse it. |
| Sync | Every changing command writes locally first. With a [replica machine](data-and-sync.md) it then merges at once (best effort): when the machine is unreachable, the change stays local, a warning goes to stderr and the exit code is still `0`. |
| Input | Titles and descriptions are single lines. Control characters are removed; descriptions are cut to 140 characters (with a warning). |

## Topic refs

Commands that act on one topic take a `<ref>`, resolved in this order:

1. **A number** (up to three digits): the position in `bhote list --all`, decimal (`07` is 7). A number that does not exist
   is an error; it never falls back to a title search.
2. **An id**, exactly, or **the start of an id** when that start is unique.
3. **A part of the title**, case-insensitive (also for non-ASCII letters).

When a ref matches more than one topic (by the start of the id or by the title), nothing changes: the candidates are
listed on stderr and the exit code is `2`. With a replica machine, a ref that matches nothing locally makes bhote merge once
and look again (a topic handed out on the other machine).

```console
$ bhote show re
bhote: 're' matches 2 topics, be more specific:
  2  Release notes
  3  Toast redesign
```

Numbers shift when topics change. For anything that runs later (a script, an agent's next step), keep the `id`.

## Commands

### `bhote add <title> [options]`

Creates a topic. Words without a leading dash form the title; a title that starts with a dash goes after `--`
(`bhote add -- -x flag`). An unknown option is exit `1`, and nothing is created.

| Option | |
|---|---|
| `-d`, `--desc <text>` | Description, at most 140 characters (longer text is cut, with a warning). |
| `@Name`, `-w`, `--waiting <Name>` | The topic waits for Name; `waiting_since` is now. |
| `-s`, `--status <now\|waiting\|review\|later\|done>` | Initial status (default `now`, or `waiting` with a name; `waiting` needs a name). Another value is exit `1`. |

```console
$ bhote add Price list approval @Alex
Price list approval
  status: waiting
  id: 1791399233-10847
  waiting for: Alex (just now)
```

Exit `1` without a title.

### `bhote list [--all] [-s <status>]`

The topics in list order: `review`, `now`, `waiting`, `later`, then (with `--all`) `done`; within a group by creation. The numbers
are the [refs](#topic-refs).

| Option | |
|---|---|
| `-a`, `--all` | Include done topics. |
| `-s`, `--status <now\|waiting\|review\|later\|done>` | Only this status (done included when asked for). |

```console
$ bhote list
  1  now      Release notes
  2  waiting  Price list approval  (Alex, just now)
  3  later    Toast redesign
```

With `--json`: an array of [topic objects](#json-output) (`[]` when there are none). An unknown option or status is exit `1`.

### `bhote show <ref>`

One topic. With `--json`: one [topic object](#json-output).

### `bhote now <ref>` · `bhote review <ref>` · `bhote later <ref>` · `bhote done <ref>`

Set the status: start or resume (`now`, also clears a waiting name), hand over for review (`review`), park (`later`), check
off (`done`). Print the topic.

When a **waiting** topic belongs to an agent (the agent created it, see the bhote skill) and you set it to `now`, that agent
gets a prompt: "The wait is over (<who>): <title>. Go on with: <description>". `bhote now <ref> --quiet` skips the prompt.

Run by an agent (a Claude Code session: `CLAUDECODE=1`), `review` and `done` send a herdr notification and record which
agent it was
(`CLAUDE_CODE_SESSION_ID`, `HERDR_PANE_ID`). With `AGENT_CAN_CLOSE=off` an agent cannot close a topic: its `done` becomes
`review` (with a note on stderr, exit `0`), and you check it off.

### `bhote wait <ref> <Name>`

The topic waits for Name from now on (`status: waiting`, `waiting_since` = now). Exit `1` without a name.

### `bhote rename <ref> <title>`

New title. Exit `1` without a title.

### `bhote desc <ref> [<text>]`

New description, at most 140 characters. Without text the description is removed.

### `bhote rm <ref>`

Deletes the topic. It stays as a tombstone (`deleted=1`) so that the deletion reaches a replica. Prints `deleted`, or with
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

### `bhote agents`

The agents herdr knows (this machine, and the saved machines when `REMOTE_AGENTS=on`), numbered. The numbers are agent refs
for `transfer`. `--json`: an array of `{"n", "machine", "pane", "status", "name", "task", "free", "topic"}`.

### `bhote transfer <from> <to> [--topic <ref>]`

[Steal & Transfer](steal-and-transfer.md): `<from>` commits its work on its branch with a handover and hands it to `<to>`
(a free agent). Agent refs: a number from `bhote agents`, `machine/pane`, a pane id, or a part of the name (exit `2` when
that matches several). The source must not wait for an answer, the target must be free. Prints the topic. Exit `1` when
herdr cannot send the prompt; nothing changes then.

### `bhote handover <topic> --to <machine/pane> --branch <b> --commit <c>`

Run by the source agent of a transfer (its prompt contains the exact command): the topic goes to the target agent, which
gets the branch, the commit and the instruction to go on. Exit `1` when no transfer is pending for the topic, when `--to`
names another agent than the transfer, or with a second topic word (quote `--to` when a machine name has spaces).

### `bhote current`

The topic of the agent that asks: matched by its Claude session (`CLAUDE_CODE_SESSION_ID`), else by its herdr pane
(`HERDR_PANE_ID`). Prints one line with the topic, its id and how to report back (`--json`: the topic object, or `null`);
nothing when there is none. Meant for a Claude Code `SessionStart` hook, so that an agent knows what it works on:

```json
{"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": "bhote current"}]}]}}
```

### `bhote event`

Called by the [herdr plugin](herdr-plugin.md) on `pane.agent_status_changed` with the event in `HERDR_PLUGIN_EVENT_JSON`
(or on stdin). When the agent of a running topic stops working (`done`/`idle`, at least 5 s after it got the topic) and has
not reported back itself, the topic goes to `review` and a herdr notification says so. Every event also makes the panels
refresh at once.

### `bhote find`

Search all agents (every machine) and topics: type to filter (every word must appear), `↑↓` choose, `⏎` jump, `Esc`
close. An agent is focused in herdr (on another machine it is selected there; switch to that machine in herdr). A topic jumps
to its agent; without one, to the bhote panel of the tab, with the topic selected. Meant for a herdr popup: settings ›
*Find key* (`FIND_KEY`) binds it. Needs a terminal.

### `bhote version` · `bhote help`

`bhote version` prints `bhote 0.1.0` (`--json`: `{"version":"0.1.0"}`); also `--version`, `-V`. `bhote help` (`-h`,
`--help`) prints the short usage, with `bhote event` and `bhote setup` among the commands.

### `bhote setup [--json]`

The [setup wizard](setup.md) in this terminal. With `--json` it only reports what is in place:
`{"machine", "jq", "herdr", "plugin", "claude_hook", "machines": [{"label", "reachable", "knows_this_machine"}], "setup_done"}`.

### `bhote` (no arguments)

Starts the [panel](panel.md). Without a terminal on stdin (or with `BHOTE_ONCE=1`) it prints one frame and exits.

## JSON output

All topic output uses one object, described by [`schema/topic.schema.json`](schema/topic.schema.json) (JSON Schema
2020-12); `list --json` is an array of them ([`schema/topic-list.schema.json`](schema/topic-list.schema.json)). A value that
is not set is `null`, never `""` or `0`. Times are Unix seconds.

| Field | Type | |
|---|---|---|
| `id` | string | Stable id (`<created>-<random>`). The same on every machine. |
| `n` | integer | Position in `bhote list --all` at the time of the call; a valid ref until topics change. |
| `title` | string | Never empty. |
| `description` | string \| null | At most 140 characters. |
| `status` | `"now"` \| `"waiting"` \| `"review"` \| `"later"` \| `"done"` | |
| `waiting_for` | string \| null | Only while `status` is `waiting`. |
| `waiting_since` | integer \| null | Only while `status` is `waiting`. |
| `agent` | object \| null | The agent of the topic: `{"name", "machine", "pane", "session"}` (each may be null). |
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
| `1` | Error: unknown command or option, missing argument, no topic for the ref, `sync` failed or is busy. Nothing changed. |
| `2` | The ref matches more than one topic. Nothing changed; the candidates are on stderr. |

## Environment

| Variable | Default | |
|---|---|---|
| `BHOTE_CONFIG` | `$XDG_CONFIG_HOME/bhote/config` | The [settings file](configuration.md). |
| `BHOTE_DATA` | `$XDG_DATA_HOME/bhote` | Topics (`topics/`), sync state and lock. |
| `BHOTE_SHARED` | `$XDG_RUNTIME_DIR/bhote-$UID` (else `$TMPDIR`) | What the panels of one machine share: agent lists, the collector lock. Must belong to the user. |
| `BHOTE_NO_SYNC` | | Set: changing commands do not merge with the replica. |
| `BHOTE_REMOTE_DIR` | `.local/share/bhote/topics` | The replica folder on the other machine, relative to its home. |
| `BHOTE_SSH` | `ssh` | The ssh command (the tests use a fake one). |
| `NO_COLOR` | | Set: no colours. `BHOTE_FORCE_COLOR` forces them without a terminal. |
| `BHOTE_ONCE`, `BHOTE_VIEW`, `BHOTE_COLS`, `BHOTE_ROWS` | | Print one frame of a view (`splash`, `main`, `settings`, `hotkeys`) at a given size, then exit. |
| `BHOTE_SOURCE_ONLY` | | Load the functions without running (for tests). |
| `BHOTE_MIN_COLS`, `BHOTE_MAX_COLS`, `BHOTE_MIN_ROWS` | `30`, `60`, `20` | Size limits of the panel. |

## Using bhote from an agent

A coding agent (or a chat) can keep its topics in step with the panel:

```sh
id=$(bhote add "Migrate the billing export" -d "CSV to Parquet" --json | jq -r .id)   # keep the id, not the number
bhote wait "$id" "DevOps" --json                                                       # blocked on someone
bhote now "$id" && bhote done "$id"                                                    # resumed, finished
bhote list --json | jq -r '.[] | select(.status == "waiting") | "\(.title) (\(.waiting_for))"'
```

Check the exit code: `2` means the ref was ambiguous, so pass the `id` instead.
