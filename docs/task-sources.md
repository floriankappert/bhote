# Task sources

Every task has a **source**: `agent`, `stash`, `linear`, `ims` or `manual` (the default). The panel shows it as a chip on the row (`agent`, `stash`, `linear`, `IMS task`,
`IMS ticket`, `manual`) and filters by it in the head of the Tasks section; `bhote list --source <s>` does the same on the command line.

| Source | How a task gets it |
|---|---|
| `agent` | An agent made it through the bhote skill (`bhote add` run by an agent), or bhote made it for an agent (an automatic review card, a card for an agent without a task). The reference is the agent's name. |
| `stash` | `bhote task from-stash <id>` or *Make a task* on a stash row: the entry leaves the stash. The reference is the stash id. |
| `linear` | Mirrored from your open Linear issues by `bhote task sync`. The reference is the identifier (`IMS-123`). |
| `ims` | Mirrored from the IMS: your personal tasks (`IMS-T<id>`, kind `task`) and your work items (`IMS-<n>`, kind `work_item`). |
| `manual` | Everything else. |

Tasks that were made before sources existed have no `source` line: it is **derived when read** and written only when the task is edited.

## Adapters

`herdr-app/mods/bhote/tasksources.py` has one adapter per outside source with the same interface: `list()` (normalized tasks), `fetch(ref)`, `open(url)` and, if the system allows
it, `set_status()`. It only talks to the outside; `bhote task sync` writes the files. A new system is a new adapter class and one line in `ADAPTERS`.

A mirrored task is an ordinary task file with a deterministic id (`linear-IMS-123`, `ims-IMS-42`), so two machines make the same file and the merge keeps one; the pair
(`source`, `source_ref`) is what a task is found by. **The outside system decides status and title.** A local status change is kept as long as the outside item does not move, and goes
back only with `TASK_WRITE=on`. An item that is gone from your open list (done, canceled, handed to someone else) is marked `done` here, never deleted; a task you deleted here does not come back.

```sh
bhote secret set LINEAR_API_KEY      # a personal API key (Linear: Settings > API); typed, never on the command line
bhote secret set IMS_API_KEY         # the ims_ key of the IMS
bhote task sync                      # now; TASK_SYNC=on (default) also does it now and then
bhote task sync --source ims --json
```

The keys come from the [secret store](secrets.md) through the module itself (`bhote secret get --reveal` is refused in Claude sessions on purpose). They go to `curl` on stdin, never into an
argument, the API cache (only the answers are cached, in the format of [the shared cache](monitors.md#api-cache)), a log or a task file. The interval is `API_CACHE_EVERY` (x 6, at most 120 s);
an error keeps the last answer and backs off as the cache does.

### Linear

GraphQL at `https://api.linear.app/graphql`, `Authorization: <key>` (a personal key takes no `Bearer`). The query is the issues assigned to you whose state is not completed or canceled
(`identifier`, `title`, `description`, `url`, `priority`, `updatedAt`, `state { name type }`, `team { key }`, `cycle`). `LINEAR_TEAMS` (`IMS,ENG`) limits it to teams.

| Linear state | Status |
|---|---|
| Backlog | `later` |
| Todo | `next` |
| In Progress | `now` |
| In Review | `review` |
| Done | `done` |
| Canceled | `done`, the description says *Canceled in Linear.* |
| any other name | by the state's type: backlog `later`, unstarted `next`, started `now`, completed or canceled `done` |

Write-back (`TASK_WRITE=on`): `issueUpdate` moves the issue to the team's state of that kind (*Done* for `done`, *In Progress* for `now`, ...). Linear has no state for `waiting`: not changeable.

### IMS

`IMS_URL` (default `https://ims-api.bilendo.de`), `Authorization: Bearer <ims_ key>`. Two kinds, both with `source: ims`:

- **work items** (tickets): `GET /api/tasks/my-work-items`; reference `IMS-<item_number>` (a Jira link is named in the description); kind `work_item`.
- **tasks** (personal to-dos): `GET /api/tasks`; reference `IMS-T<id>`; kind `task`. `work_item_id` becomes `source_parent` (`IMS-<n>`), so the work item is not listed twice.

Soft-deleted items and items in the done set (Done, Closed, Archived, Completed, Erledigt) are not mirrored. Status names: Open `next`, In Progress / In Bearbeitung `now`, Review / In Review `review`,
Blocked / Waiting / Wartend `waiting`, Backlog `later`; anything else `next`. The vocabulary of work items is configurable per IMS environment: `IMS_STATUS_MAP` (`In Test=review,Parked=later`) adds names and replaces
defaults. Write-back: `PATCH /api/company/work-items/:id` with `{status}` for a ticket, `PUT /api/tasks/:id` for a task. An answer of **409** (the status of an Insight moves only through its workflow) is
*not changeable*: the task says so (`source_note`) and nothing is retried.

`IMS_WEB_URL` tells where an item opens in the browser (`<it>/work-items/<id>`, `<it>/tasks/<id>`); without it the link of a task is the API address of the item.
