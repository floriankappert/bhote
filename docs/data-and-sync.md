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
| `status` | `now`, `next`, `waiting`, `review`, `later` or `done`; anything else reads as `now`. |
| `created`, `updated`, `waiting_since` | Unix seconds. `updated` grows with every change (also within the same second). |
| `deleted` | `1` = a tombstone: hidden everywhere, kept so that the deletion reaches the replica. |
| `agent`, `agent_machine`, `agent_pane`, `agent_session` | Set when the topic was handed to a herdr agent (or an agent created it). |
| `transfer_to`, `transfer_to_name`, `transfer_from` | Only while a [Steal & Transfer](steal-and-transfer.md) is pending; any status other than `now` clears them. |
| `handover_branch`, `handover_commit` | Where the last transfer left the work. |

Records with a `kind` sit in the same folder and travel the same way, but are no topics: `proj-<slug>.topic`
(`kind=project`: `title`, `repos`, `workspaces`) and `mach-<slug>.topic` (`kind=machine`: `title` = the machine's name,
`code` = its three capitals, set by hand).

Rules: one `KEY=value` per line; when a key appears twice, **the first one counts** everywhere; values have no control
characters. Files are written atomically (a temporary file, then `mv`) with mode `0600` in a `0700` folder.

## Replica and merge

With `STORE=remote` and `STORE_MACHINE=<label>`, the folder `~/.local/share/bhote/topics` on that machine (reached as
`ssh <host>` from `herdr machine list`) holds a replica. The **local copy is always the working copy**: reading and writing
never wait for the network.

How the topics travel is `SYNC_VIA`:

- **`herdr`** (default): no ssh of bhote's own. bhote asks the herdr plugin on the other machine for its topics (the plugin
  action `dump`) and takes the newer ones. Nothing is written to the other side: **it pulls from this machine by itself**, so
  both machines need `STORE=remote` with the other one as `STORE_MACHINE` (the setup wizard sets that up). herdr passes at
  most 64 KB of plugin output; the dump is packed (gzip, base64) and carries the most recently changed topics first, up to
  about 400 KB of topic files. Older ones beyond that wait until they change.
- **`ssh`**: bhote reads the replica folder in one ssh call and writes newer local topics back (push and pull).

A merge (after every local change, every `REMOTE_EVERY` seconds, and on `bhote sync`):

1. Reads all replica files at once. The stream must end with an end marker; a cut-off stream (the connection died, more
   than 8 MB) is not merged at all and counts as offline.
2. Accepts only plain file names (`[A-Za-z0-9][A-Za-z0-9_-]*.topic`) and only valid files: `KEY=value` lines, no key twice,
   numeric times, nothing more than 5 minutes in the future, the `id` equal to the file name, small; at most 2000 topics.
   Everything else is not taken, with a warning on stderr.
3. Per topic, the newer `updated` wins, in both directions. With the same `updated` and different contents, both sides pick
   the same one (the larger checksum). A local change made while the merge ran is never overwritten; over ssh, a topic is
   written to the other side only when it is still the version the merge read there.
4. Deletions travel as tombstones; a tombstone older than 30 days is removed (both sides have it by then).

While the machine cannot be reached, bhote keeps working locally and shows `offline`; the next merge catches up. One merge
runs at a time per machine (the lock is a symlink to the owner's pid, made atomically; a lock of a process that is gone is
taken over). Remote commands run
through `sh -c` as one quoted word, so the remote login shell (zsh, fish) does not matter; ssh runs with `BatchMode`, a
connect timeout and keep-alives, so a dead connection cannot hang it.

## What the panels share

All panels of one machine share `$BHOTE_SHARED` (private, owned by the user): the agent lists and the collector lock. One
panel (the first one alive) asks herdr for the agents every `LOCAL_EVERY` / `REMOTE_EVERY` seconds and writes the lists; the
others only read them. When that panel ends, another takes over. Agent list lines are
`machine␟pane␟status␟workspace␟task` (␟ = the unit separator `0x1F`).
