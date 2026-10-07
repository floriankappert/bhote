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

When a ref matches more than one topic, nothing changes: the candidates are listed on stderr and the exit code is `2`.

```console
$ bhote show re
bhote: 're' matches 2 topics, be more specific:
  2  Release notes
  3  Toast redesign
```

Numbers shift when topics change. For anything that runs later (a script, an agent's next step), keep the `id`.

## Commands

### `bhote add <title> [options]`

Creates a topic. Words without a leading dash form the title.

| Option | |
|---|---|
| `-d`, `--desc <text>` | Description, at most 140 characters (longer text is cut, with a warning). |
| `@Name`, `-w`, `--waiting <Name>` | The topic waits for Name; `waiting_since` is now. |
| `-s`, `--status <now\|later\|done>` | Initial status (default `now`, or `waiting` with a name). |

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

Run by an agent (a Claude Code session: `CLAUDECODE=1`), the command also records which agent it was
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

### `bhote version` · `bhote help`

`bhote version` prints `bhote 0.1.0` (`--json`: `{"version":"0.1.0"}`); also `--version`, `-V`. `bhote help` (`-h`,
`--help`) prints the short usage.

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
