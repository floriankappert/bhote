---
name: bhote
description: Keep the user's bhote topics in step with your work. Use it whenever you end a turn that hands work back to the user (a result to check, a decision, a permission or an answer only they can give), even when no topic was given; when you reach a point where someone or something else has to act before you can go on (a deployment, a review, infrastructure, an answer), when you finish or hand over a topic you were given, and when you are told to hand your work over (Steal & Transfer), and when the user asks to pin a Slack channel, DM or thread to a topic.
---

# bhote: topics next to herdr

`bhote` is the user's list of topics (what is being worked on, who is waited for) in a herdr side panel. You work on
it through its CLI; every command takes `--json`. A topic you create from your session belongs to you: bhote records
your session and pane by itself.

## When you have to wait for someone or something

When you cannot go on until someone else acts (a deployment has to finish, a reviewer has to approve, infrastructure has
to be ready, a person has to answer), do not poll and do not stop silently. Create a waiting topic:

```sh
bhote add "<what is needed, short>" -w "<who or what you wait for>" -d "<what you will check or do then, at most 140 characters>" --json
```

Examples:

```sh
bhote add "Deploy of billing-export to staging" -w "DevOps" -d "then run the export e2e test against staging" --json
bhote add "Review of PR #1412" -w "Alex" -d "then merge and test the release notes page" --json
```

Keep the returned `id`. Tell the user in one sentence what you wait for, then stop or do other work. When the user marks
the topic as `now` in bhote, you get a prompt "The wait is over …" with the description: continue from there.

## When you hand the work back to the user

Whenever you end a turn and the next step is the user's (check a result, decide, grant a permission, answer a question),
the work belongs under review, also when nobody gave you a topic. Before you write the final message:

```sh
bhote current --json                       # your topic, or null
bhote add "<the work, short>" -d "<what the user has to check or decide, at most 140 characters>" --json   # only when null
bhote review <id>
```

When the user answers and you go on, run `bhote now <id>`; when the work is accepted and nothing is left, `bhote done <id>`.
Do not use `-w` for the user: waiting is for other people and things.

## Slack: pin a channel, DM or thread

The user can pin Slack to a topic ("create a topic for X and pin Karen's Slack channel"). bhote then watches it while the
topic waits: when someone else writes there, the topic goes to review with that message. Someone's "Slack channel" is
the DM with them (`@Name`). Such a topic usually waits for that person:

```sh
bhote add "<title>" -w "Karen" --slack "@Karen" --json      # a new topic
bhote slack <ref> "@Karen" --json                          # pin on an existing topic (bhote wait <ref> "Karen" if it does not wait yet)
bhote slack <ref> off --json                               # unpin
```

`<pin>` is `@person` (the DM), `#channel`, or a Slack link (a channel, a DM, or a message: a message link watches its
thread). `@` and `#` take a few seconds: bhote asks Claude Code to look them up. When bhote says SLACK_WATCH is off, tell
the user that one machine needs it on (`bhote config set SLACK_WATCH on` where Claude Code has the Slack connector).

## When you were given a topic

A prompt from bhote ends with "(bhote topic <id>: when you are finished, run …)". When you are finished, run exactly what
it says: `bhote review <id>` when the user should check the result, `bhote done <id>` when nothing needs checking (only
when the prompt offers it). If you are blocked, use `bhote wait <id> "<who>"`.

## Steal & Transfer

When a prompt starts with "Steal & Transfer (bhote topic <id>…): stop here and hand this work over", do exactly its
steps, in this order: commit everything on the **current** branch (no new branch; only on a detached HEAD create the one
it names) with a handover in the commit message body (goal, what is done, what is open, next steps, how to test), push if
there is a remote, note the branch and the commit, free the branch with `git switch --detach` if the other agent works in
another worktree of the repository, then run the `bhote handover …` command it gives you and stop. When a prompt says
"Steal & Transfer: take over …", switch to the branch (or the commit) it names, read the handover with
`git log -1 <commit>`, and continue.

## Useful

```sh
bhote list --json          # the topics (review, now, waiting, later)
bhote current --json       # your own topic, if you have one
bhote agents --json        # the herdr agents, free or busy
```
