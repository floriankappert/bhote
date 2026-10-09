---
name: bhote
description: Keep the user's bhote topics in step with your work. Use it whenever you end a turn that hands work back to the user (a result to check, a decision, a permission or an answer only they can give), even when no topic was given; whenever you commit (with any commit skill or command, or plain git); when you reach a point where someone or something else has to act before you can go on (a deployment, a review, infrastructure, an answer), when the user answers your question or otherwise lets you go on after you handed work back (your topic is in review and you continue: put it back on now), when you finish or hand over a topic you were given, when you are told to hand your work over or the user asks to move work from one agent to another (Steal & Transfer), and when the user asks to pin a Slack channel, DM or thread to a topic.
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

If `bhote add` creates nothing because agents may not create topics here (`AGENT_TOPICS=off`: it prints `null` and says
so), go on without a topic and do not try again. This holds for every `bhote add` below.

## When you hand the work back to the user

Whenever you end a turn and the next step is the user's (check a result, decide, grant a permission, answer a question),
the work belongs under review, also when nobody gave you a topic. Before you write the final message:

```sh
bhote current --json                       # your topic, or null
bhote add "<the work, short>" -d "<what the user has to check or decide, at most 140 characters>" --json   # only when null
bhote review <id> --summary "<result, what is open, what only the user can do>" --look
```

### The card: summary and the jump button

The card is what the user reads **without opening your session**: a short summary and one button, *Review ansehen*, that takes
the user to your session. There are **no buttons that trigger an action**: an answer can be *and* as well as *or* (commit **and**
push, this **or** that), and one press cannot say which. The user types the answer in your session. Always pass `--look`
(`--ask` and `--reply` still exist, but bhote turns them into the same jump button).

**`--summary`** (at most 240 characters), in this order:

1. the result in one clause,
2. what is open or unverified (not committed, not tested, not deployed),
3. anything only the user can do (rotate a secret, approve, log in), and the question or proposal you want answered, in short:
   `Vorschlag: Commit + Push?`, `2 offene Punkte`.

Not the way you got there. A warning that only stands in your long message gets lost; put it in the summary. With several open
points, say how many (`Zwei offene Punkte: …`) and what blocks. Write it in the user's language.

Your final message in the session says the same in full, with the proposal or the questions as its last lines, so that card and
message never disagree.

**Examples** (the situation → the text in the card):

| Situation | Text in the card |
|---|---|
| Fix done, you want to commit | Fix im Export fertig, Tests grün. Nichts committet, nichts deployt. Vorschlag: Commit. |
| Feature done, the user usually merges | Neue Filter im Report fertig, Tests grün. Nichts committet. Vorschlag: Commit, PR, Merge. |
| You proposed creating a repository | Skripte und Doku liegen vor, nichts committet. Vorschlag: neues privates Repo `export-tools`. |
| You wait for a deploy only the user can start | Fix gebaut und getestet. Zum Prüfen muss er in Staging deployt werden, das kannst nur du. |
| You wait for a login on a device | Installation fertig. Es fehlt eine Anmeldung am Gerät, die nur du machen kannst. |
| Two ways, you recommend the second | Zwei Wege: A läuft sofort lokal, B dauerhaft im Container. Ich empfehle B. |
| A choice with your proposal | Ansicht fertig. Offen: welche Bereiche einklappbar sind. Vorschlag: nur Archiv und Berichte. |
| Migration written, never run | Migration und Test geschrieben, Tests grün. Die Migration lief noch nicht gegen eine Datenbank. Nichts committet. |
| A secret showed up in the chat | Aufgabe erledigt. Ein Zugangstoken lag im Klartext im Chat: bitte rotieren, das kannst nur du. |
| Release prepared, version unclear | Release vorbereitet. Vorschlag 0.6.0 statt 0.5.9, weil viele Änderungen enthalten sind. |
| Two open points | Zwei offene Punkte: Freigabe der Schnittstelle und der Fix für den Import. Details in der Session. |
| An open scope question | Eintrag hängt an drei Datensätzen, Löschen kaskadiert. Offen: ganz löschen oder nur ausblenden? |
| You need a fact from the user | Ich brauche eine Information von dir: Läuft die Anwendung im Produktivmodus? Davon hängt der Test ab. |

bhote also watches herdr: when you wait for an answer or end your turn with a question and have no topic, a review topic
"Question from <you>" appears by itself and goes away when you work again. A topic you create yourself, with what the user has to check or decide in the description,
is still better: it tells the user what the question is.

When the work is accepted and nothing is left, `bhote done <id>`. Going on after an answer is the next section.
A topic the user parked (`later`) or closed (`done`) stays that way: `bhote now` refuses it for you. Do not work on it or move it
again unless the user asks you to; then `bhote take <ref>`.
Do not use `-w` for the user: waiting is for other people and things.

## When the user answers and you go on

A topic under review stays there until someone moves it, and the user does not: they just answer in your session. So when
you handed work back (your topic is in `review`, or you asked a question) and the user's next message lets you continue (an
answer, a decision, a permission, "go on"), put the topic back on `now` **as the first thing you do in that turn**, before
you start working:

```sh
bhote current --json        # your topic (null: there is none, nothing to move)
bhote now <id>              # review → now: the panel shows you as working on it again
```

The card's button only takes the user to your session and leaves the topic in `review`: when the user then writes to you, it is this
section that puts it back on `now`.

This is not for a topic the user parked or closed (`bhote now` refuses those, leave them). When the answer shows that the
topic is finished after all, use `bhote done <id>` instead. Without this step the panel keeps showing "ready for review"
while you are working, and the user cannot tell what is waiting for them.

## When you commit

Whatever makes the commit (the user's own commit skill or slash command, or plain `git commit`), close the topics it
finishes right after it:

```sh
bhote list --status review --json      # the topics waiting for the user's check
bhote done <id>                        # each one this commit finishes
```

A topic belongs to the commit when it is yours (`bhote current`), when its `agent.branch` is the feature branch you committed on (not `main`, which many share),
or when its title or description names the work the commit contains. Leave a topic whose work the commit only starts
or does not touch, and topics of other repositories. Say in one line which topics you closed.

## When you run tests

When the test monitor is on (`bhote config get TEST_MONITOR` says `on`), run test commands through bhote, so that the user
sees the run in the panel; output and exit code stay the command's own:

```sh
bhote test run -- bin/rails test test/models/user_test.rb
bhote test run -n server -- npm run test:server
```

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

## When you take over a topic

When the user asks you to work on an existing topic (by its title or number), run `bhote take <ref> --json` first: it
becomes your topic (your session and pane), status `now`. Then report back as below (`review`, `done`, `wait`).

## When you were given a topic

A prompt from bhote ends with "(bhote topic <id>: when you are finished, run …)". When you are finished, run exactly what
it says: `bhote review <id>` when the user should check the result, `bhote done <id>` when nothing needs checking (only
when the prompt offers it). If you are blocked, use `bhote wait <id> "<who>"`.

## Steal & Transfer

**Never start a transfer yourself.** `bhote transfer` (and `bhote handover` without a prompt that asks for it) is for when the user
explicitly tells you to move work to another agent, by name. "Done", "good" or "thanks" is not that. bhote refuses a target in
another project of the topic; do not add `--force` on your own, ask the user. Hand-overs interrupt the other agent's work.

When a prompt starts with "Steal & Transfer (bhote topic <id>…): stop here and hand this work over", do exactly its
steps, in this order: commit everything on the **current** branch (no new branch; only on a detached HEAD create the one
it names) with a handover in the commit message body (goal, what is done, what is open, next steps, how to test), push if
there is a remote, note the branch and the commit, free the branch with `git switch --detach` if the other agent works in
another worktree of the repository, then run the `bhote handover …` command it gives you and stop. When a prompt says
"Steal & Transfer: take over …", switch to the branch (or the commit) it names, read the handover with
`git log -1 <commit>`, and continue. The prompt assumes the repository you are in: when the work lives in another one
(e.g. `~/Development/<repo>` next to yours), fetch and switch there, cloning it first if it is missing. A source only hands
over what it knew: check the handover for what it says is missing before you build on it.

### Starting a transfer for the user

```sh
bhote agents --json                                           # machine, pane, status, free
bhote add "<what moves>" -d "<from where to where>" --json    # the topic it travels with
bhote transfer <machine/pane> <machine/pane> --topic <id>     # from, to
```

- **Pass `--topic`.** Without it bhote takes the source's current topic, which is often unrelated.
- **The target must be free** (idle or done), checked live right before the prompt. If *you* are the target you are
  `working` while you run the command, so start it from a detached waiter and end your turn; the take-over prompt then
  arrives as your next turn:
  `setsid nohup bash -c 'until herdr pane list | jq -e ".result.panes[] | select(.pane_id==\"<your pane>\") | .agent_status | test(\"idle|done\")" >/dev/null; do sleep 10; done; bhote transfer …' >log 2>&1 </dev/null &`
  (a `run_in_background` task does not do: it wakes you when it ends).
- **The handover travels through git.** The source must work in a repository with a remote the target can reach. When it
  works outside one (e.g. its home directory), prepare it first with a prompt: clone the repository, `git switch -c <branch>`,
  write its state as files, do not commit yet. Never let it `git init` a home directory.
- **Look at the target's working tree.** Uncommitted files at the paths the source commits block `git switch`; have the
  source write to a separate path (e.g. `docs/<machine>/`).
- **Agents on another machine:** prompt with `herdr --machine <label> agent prompt <pane> "<text>"`; read status and folder
  with `herdr --machine <label> pane list | jq '.result.panes[] | select(.pane_id=="<pane>") | {agent_status, cwd}'`.
  Wait for `idle`/`done` before the transfer.

## bhote is not set up yet

If `bhote setup --json` shows something missing (`plugin`, `claude_hook` false, no machines), or the user has just installed
bhote (`brew install floriankappert/bhote/bhote`, `pacman`, `install.sh`) and wants it working: continue with the skill
**bhote-install** (`/bhote-install`). It installs and sets up everything through the CLI. Without the skill in this Claude
profile run `bhote skills install` first.

## Useful

```sh
bhote list --json          # the topics (review, now, waiting, later)
bhote current --json       # your own topic, if you have one
bhote agents --json        # the herdr agents, free or busy
```
