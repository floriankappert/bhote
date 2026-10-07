# Steal & Transfer

Move a piece of work from one coding agent to another without losing it: the first agent stops, commits its state on
its branch with a written handover, and a free agent picks up that branch and goes on. Useful when an agent runs out of
context, when another machine or worktree should continue, or when you need the first agent for something else.

## Start it

- **In the panel:** select the topic, press `t` (or `→` › *Transfer to another agent …*), pick a free agent.
- **From the command line:** `bhote transfer <from> <to> [--topic <ref>]`. `<from>` and `<to>` are agents from
  `bhote agents`: a number, `machine/pane` (`Omarchy/w4:p1`), a pane id, or a part of the name. Without `--topic` bhote
  takes the topic of the source agent, or creates one from its current task.

The target must be free (idle or done), and checked live right before anything is sent.

## What happens

1. **The source agent** gets a prompt: commit everything on the **current branch** (no new branch), with a handover in the
   commit message body (goal, what is done, what is open, next steps, how to test); push if the repository has a remote;
   run the `bhote handover …` command it is given; if the target works in another worktree of the same repository, free
   the branch with `git switch --detach`; stop. The list shows `source → target · handing over`.
2. **`bhote handover <topic> --to <machine/pane> --branch <b> --commit <c>`** (run by the source agent) hands the topic to
   the target: it now belongs to that agent, and you get a herdr notification.
3. **The target agent** gets a prompt: fetch if there is a remote, `git switch <branch>`, read the handover with
   `git log -1 <commit>`, continue, and report back with `bhote review` / `bhote done` like any delegated topic.

Everything travels through herdr (`herdr agent prompt`, with `--machine` for agents on another machine), so a transfer can
go from the Mac to a Linux box and back. The handover lives in git, where the code is.

The agents learn the procedure from the bhote skill for Claude Code (`integrations/claude/skills/bhote`, installed by the
setup wizard).
