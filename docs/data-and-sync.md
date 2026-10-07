# Data and sync

## Topic files

Each topic is one small text file, `$BHOTE_DATA/topics/<id>.topic` (default `~/.local/share/bhote/topics`), readable and
editable by hand:

```ini
id=1791399233-10847
title=Price list approval
status=waiting
waiting_for=Alex
waiting_since=1791399233
created=1791399233
updated=1791399233
deleted=0
description=
agent=IMS (wt1)
agent_machine=Mac
agent_pane=wA:p1
```

| Key | |
|---|---|
| `id` | `<created>-<random>`; also the file name. Only `[A-Za-z0-9_-]`. |
| `status` | `now`, `waiting`, `later` or `done`; anything else reads as `now`. |
| `created`, `updated`, `waiting_since` | Unix seconds. `updated` grows with every change (also within the same second). |
| `deleted` | `1` = a tombstone: hidden everywhere, kept so that the deletion reaches the replica. |
| `agent`, `agent_machine`, `agent_pane` | Set when the topic was handed to a herdr agent. |

Rules: one `KEY=value` per line; when a key appears twice, **the first one counts** everywhere; values have no control
characters. Files are written atomically (a temporary file, then `mv`) with mode `0600` in a `0700` folder.

## Replica and merge

With `STORE=remote` and `STORE_MACHINE=<label>`, the folder `~/.local/share/bhote/topics` on that machine (reached as
`ssh <host>` from `herdr machine list`) holds a replica. The **local copy is always the working copy**: reading and writing
never wait for the network.

A merge (after every local change, every `REMOTE_EVERY` seconds, and on `bhote sync`):

1. Reads all replica files in **one** ssh call. The stream must end with an end marker; a cut-off stream (ssh died, more than
   8 MB) is not merged at all and counts as offline.
2. Accepts only plain file names (`[A-Za-z0-9][A-Za-z0-9_-]*.topic`) and only valid files: `KEY=value` lines, no key twice,
   numeric times, nothing more than 5 minutes in the future, small. Everything else is ignored.
3. Per topic, the newer `updated` wins, in both directions. A local change made while the merge ran is never overwritten.
4. Deletions travel as tombstones.

While the machine cannot be reached, bhote keeps working locally and shows `offline`; the next merge catches up. One merge
runs at a time per machine (a lock with the owner's pid; a lock of a process that is gone is taken over). Remote commands run
through `sh -c` as one quoted word, so the remote login shell (zsh, fish) does not matter; ssh runs with `BatchMode`, a
connect timeout and keep-alives, so a dead connection cannot hang it.

## What the panels share

All panels of one machine share `$BHOTE_SHARED` (private, owned by the user): the agent lists and the collector lock. One
panel (the first one alive) asks herdr for the agents every `LOCAL_EVERY` / `REMOTE_EVERY` seconds and writes the lists; the
others only read them. When that panel ends, another takes over. Agent list lines are
`machine␟pane␟status␟workspace␟task` (␟ = the unit separator `0x1F`).
