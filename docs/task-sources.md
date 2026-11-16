# Task sources

Every task has a **source**: `agent`, `stash`, `linear` or `manual` (the default). The panel shows it as a chip on the row (`agent`, `stash`, `linear`, `manual`) and filters by it in the head of the Tasks section; `bhote list --source <s>` does the same on the command line.

| Source | How a task gets it |
|---|---|
| `agent` | An agent made it through the bhote skill (`bhote add` run by an agent), or bhote made it for an agent (an automatic review card, a card for an agent without a task). The reference is the agent's name. |
| `stash` | `bhote task from-stash <id>` or *Make a task* on a stash row: the entry leaves the stash. The reference is the stash id. |
| `linear` | Mirrored from the Linear issues **in triage** by `bhote task sync`. The reference is the identifier (`ABC-123`). |
| `manual` | Everything else. |

Tasks that were made before sources existed have no `source` line: it is **derived when read** and written only when the task is edited.

Task files of the removed `ims` source (the IMS was a source before) still read: the value is not a known source any more, so the source is derived (`manual`) on the command line and shown as plain text in the panel; the sync never touches them.

## Adapters

`herdr-app/mods/bhote/tasksources.py` has one adapter per outside source with the same interface: `list()` (normalized tasks), `fetch(ref)`, `open(url)` and, if the system allows
it, `set_status()`. It only talks to the outside; `bhote task sync` writes the files. A new system is a new adapter class and one line in `ADAPTERS`.

A mirrored task is an ordinary task file with a deterministic id (`linear-ABC-123`), so two machines make the same file and the merge keeps one; the pair
(`source`, `source_ref`) is what a task is found by. **The outside system decides status and title.** A local status change is kept as long as the outside item does not move. An issue that is gone
from the triage list is marked `done` here, never deleted; a task you deleted here does not come back. The raw state name is kept in `source_status_raw`; `source_status` is the status as mirrored
(bhote's vocabulary), the one a local change is compared with.

```sh
bhote secret set LINEAR_API_KEY      # a personal API key (Linear: Settings > API); typed, never on the command line
bhote task sync                      # now; TASK_SYNC=on (default) also does it now and then
bhote task sync --source linear --json
bhote task sync --source linear --dry-run   # what a sync would create, update, close or skip: nothing is written
bhote task prune --source linear --dry-run  # what a clean-up would delete; then --yes
```

**Dry run:** `bhote task sync [--source linear] --dry-run` (`-n`) asks Linear and applies the same rules as the sync, but writes no task file, no tombstone and no API cache entry and takes no lock. Per source
it counts *would create / would update / unchanged / would close / would skip*, with the reasons of the skipped (`not in triage`, removed here by hand), and lists the tasks it would create or update with their title and status.
The counts are the ones the real sync then reports on the same data.

**Clean-up:** `bhote task prune --source linear [--dry-run] [--yes]` deletes the mirrored tasks of that source that you did **not** change here. A task stays when its status was moved by hand, when it has a field of
its own (project, agent, a wait for someone else ...) or when it was touched after the sync made it and its description is not a mirrored one; `--dry-run` says why. Without `--dry-run` or `--yes` it refuses. It deletes like `bhote rm`
(`deleted=1`, which is the tombstone the merge and kharka carry to the other machines) and marks the file `pruned=1`: a sync brings a pruned task back (as a fresh one) when its issue is in triage again, a task you deleted by hand stays gone.

The keys come from the [secret store](secrets.md) through the module itself (`bhote secret get --reveal` is refused in Claude sessions on purpose). They go to `curl` on stdin, never into an
argument, the API cache (only the answers are cached, in the format of [the shared cache](monitors.md#api-cache)), a log or a task file. The interval is `API_CACHE_EVERY` (x 6, at most 120 s);
an error keeps the last answer and backs off as the cache does.

### Linear

GraphQL at `https://api.linear.app/graphql`, `Authorization: <key>` (a personal key takes no `Bearer`).

**Only issues in triage are mirrored**: issues whose workflow state is of type `triage`, in the teams of `LINEAR_TEAMS` (`ENG,OPS`; empty: every team the key sees), **assigned or not** (triage issues are usually unassigned).
Nothing else from Linear comes over: no Todo, Backlog, In Progress or In Review, and no due-date or overdue rule.

| Linear | Task |
|---|---|
| issue in triage | status `next`; `source_ref` the identifier (`ABC-123`), `source_url` its link, `title`, the first line of the description |
| issue leaves triage (accepted, moved to another state, done, canceled, duplicate, deleted) | the task is marked `done`, the description says *No longer in triage in Linear.*; the file stays |
| issue back in triage | the task is `next` again |

Write-back (`TASK_WRITE=on`, default off) is minimal: marking a mirrored task **done** here moves the Linear issue to the *Done* state of its team (`issueUpdate`; the team's *Done* state, else its first state of type completed).
Every other status change stays local; it is not written back and is no error.
