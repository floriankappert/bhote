# Steal & Transfer

Move a piece of work from one coding agent to another without losing it: the first agent stops, commits its state on
its branch with a written handover, and a free agent picks up that branch and goes on. Useful when an agent runs out of
context, when another machine or worktree should continue, or when you need the first agent for something else.

## Start it

- **In the panel:** select the topic, press `t` (or `→` › *Transfer to another agent …*), pick a free agent. The panel
  asks before it starts, and warns when the target belongs to another project than the topic. A pending hand-over can be
  cancelled from the topic's page.
- **From the command line:** `bhote transfer <from> <to> [--topic <ref>] [--force]`. `<from>` and `<to>` are agents from
  `bhote agents`: a number or a code (`MAC4`), `machine/pane` (`Omarchy/w4:p1`), a pane id, or a part of the name. A target of
  another project is refused unless you add `--force`. Without `--topic` bhote
  takes the topic of the source agent, or creates one from its current task.

The target must be free (idle or done), and checked live right before anything is sent. A source agent that waits for an
answer (a permission prompt) is refused: answer it first. When the prompt cannot be sent, nothing changes (a topic that
`bhote transfer` created for it goes again).

## What happens

1. **The source agent** gets a prompt: commit everything on the **current branch** (no new branch; only on a detached HEAD
   it creates `bhote-<topic id>`), with a handover in the commit message body (goal, what is done, what is open, next
   steps, how to test); push if the repository has a remote; note the branch and the commit; if the target works in
   another worktree of the same repository, free the branch with `git switch --detach`; then run the `bhote handover …`
   command it is given, and stop. The list shows `source → target · handing over`.
2. **`bhote handover <topic> --to <machine/pane> --branch <b> --commit <c>`** (run by the source agent) hands the topic to
   the target: it now belongs to that agent, and you get a herdr notification. It works only while the transfer is
   pending and only for the agent it was meant for; parking, closing or setting the topic to wait or review ends a
   pending transfer.
3. **The target agent** gets a prompt: fetch if there is a remote, `git switch <branch>` (no branch: the commit itself),
   read the handover with `git log -1 <commit>`, continue, and report back with `bhote review` / `bhote done` like any
   delegated topic.

Everything travels through herdr (`herdr agent prompt`, with `--machine` for agents on another machine), so a transfer can
go from the Mac to a Linux box and back. The handover lives in git, where the code is.

Agents never start a hand-over on their own: the skill tells them to do it only when you ask.

The agents learn the procedure from the bhote skill for Claude Code (`integrations/claude/skills/bhote`, installed by the
setup wizard).
