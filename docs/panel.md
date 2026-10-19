# The panel

`bhote` without arguments is a panel for a herdr pane of about 56 columns (it works from 30 to any width; the list uses up to
100 columns). It opens with a welcome screen that closes on any key (or by itself after 15 s), then shows the topics on top
and the agents below.

By default (`CARD_GROUPING=ungrouped`, settings › *Agent cards: grouping/sorting*) the cards are **one list without group headings**, in the order of the agents in herdr's agent panel (an idle agent without a card has its own card in its place; cards without an agent follow). With `CARD_GROUPING=status` they are grouped as described below.

The list shows what is on. **Review**, **now**, **testing**, **deploying** and **recently finished** topics are cards, all drawn the same way, three rows
(a blank row between two cards, the status colour as a bar on the left):

```
▌ ✶ Omarchy · IMS (main)               working        icon, machine · agent, state (and age) on the right
▌ ∟ Sales Analytics                                   what the agent works on (its task; the topic's title without one)
▌   ⎇ feature/opportunity-detail                      its branch
```

A working agent has a fourth row (orange) under the task: what it does right now. The bars of a deployment or of running tests come under the
branch. The **description** of a topic is shown under the card you have selected. A card without an agent shows the title, then `∟ who · state · age`.
A review card has no summary, question or button under it (the summary is in `bhote show` and in the Claude mod's pane); `1` focuses the agent. A topic is in **testing** when its agent stopped while a test suite runs (also one in the background); review follows when the tests are done, now when the agent works again. **Next**, **waiting** and parked (**later**) topics are lines grouped by status, like the done ones: a
dot, the title and the time (waiting: for whom). A group shows ten lines; `+3 more next` below it is a row of its own: select it
with the arrow keys and press `Enter` or `→` (or click it) to open the group, again to close it (`− show less`). Done, next,
waiting and parked topics that nobody touched for `ARCHIVE_DAYS` (7) days are archived: they leave the panel (the last line of the Done section counts them: `✓ 42 done · ▤ 30 archived`),
the search and the CLI still reach them, and a change brings one back. A topic chosen in the search stays visible while it
is selected.

**Idle** is the last group of cards: every idle agent that has no card of its own, with its last task; a click shows the agent. The agent list at the bottom holds the others.

**Recently finished** is a group of cards under now and testing: a topic whose agent stopped without a question (nothing to check; with a
question it goes to review). It shows the agent with a green check and how long ago it finished. When the agent works again the topic is back on
now; after `FINISHED_MINUTES` it is dismissed by itself; `c` dismisses it, `x` checks it off.

**Deploying** is a group of cards between testing and next: a waiting topic with `bhote wait <ref> --deploy` (topic menu ›
*Waiting for the deployment*), or a topic of an agent whose branch is being deployed (also one that is under recently finished: the agent has stopped, the
branch is still on its way; the deployment is found by the branch, by the pull request, or by the last commit in the agent's folder). The card shows
project and deployment (`IMS · Backend`) behind the branch, its state (deploying, queued, needs approval) and the progress as a bar with a percentage,
taken from the deployment monitor. When the deployment is over the topic goes to review. A topic that says it waits for a deployment while none runs
for it is a line in the normal Waiting list.

## Keys

The list is for the mouse: **click a card** to show its agent in herdr (on another machine: selected there; in another tab the focus goes to the bhote panel there), **double-click it** to open its page, **click a line** (next, waiting, later, done) to open its page. There are no arrow keys or `⏎` on the list, and no context menu (the page has the actions).

| Key | List |
|---|---|
| `→` | the topic's page (also a double click on a card, or a click on a line: next, waiting, later, done): title, description, status and the actions (start with agent, up next, waiting for, park, done, rename, describe, delete). `↑↓` also reach the title and the description; `⏎` edits them right there (`⏎` keeps, `Esc` throws away) |
| `n` | new topic: `Title`, `Title; description`, optionally ending in `@Name` or `> Name` (= waiting for Name) |
| `e` / `d` | edit the title / the description |
| `x` | mark done |
| `c` | dismiss a review card: it closes, and opens again as the work card when its agent works again |
| `s` | start on a free agent: its title and description are typed into that agent's pane |
| `t` | [Steal & Transfer](steal-and-transfer.md): hand the topic and its branch from its agent to another one (asks first; warns when the target belongs to another project) |
| `1` (`y`) | *Review ansehen* for the selected review topic (`bhote review --look`): focuses the topic's agent; the topic stays in review |
| `a` | suggestions: what the running agents work on (their task) and is not a topic yet |
| `w` | the setup wizard's new steps, when the head says *New Features Available* |
| `,` | settings (the full key list is under *Hotkeys* there; *Connections* holds the data location, remote agents, the Slack monitor and the machine codes; *Setup wizard* runs the wizard again) · `S` welcome screen · `q` quit |

The find key (settings › *Find key*, e.g. `ctrl+alt+t`, on the Mac ⌘T) opens a search popup over all agents and topics:
type, choose, `⏎` jumps there; `x` (`ctrl+x` once something is typed) checks a topic off;
`ctrl+n` adds a topic (the search text is its start). `⏎` on a topic opens its page here in the panel.

Click a topic (any of its lines, also a done one) to select it, as the arrows do; the list stays where it is while the
selected topic is in view; a double click opens its page, where a click on an action runs it and a click on the title or
description edits it in place. Click an agent in the agent area to focus it in herdr. An agent on another machine is
selected there; herdr lets only its client switch machines (herdr 0.9.3 has no call that switches its window to another
machine). On a Mac with `AGENTS_VIA=kharka` bhote therefore types herdr's key for the agent's row into the herdr window
(`focus_agent`, e.g. `prefix+ctrl+1..9`, or `[keys.indexed] agents`), and herdr switches to it. The row
is the agent's place in herdr's agent list: this machine's agents first, then each machine of `herdr machine list` in its
order, each sorted by workspace number. Typing needs the Accessibility permission for the terminal herdr runs in. A panel
runs on the machine whose herdr the window shows, so when a Mac shows the other machine's herdr, the click lands in a panel
that cannot type on the Mac: that panel asks through kharka (`bhote.jump/<node>`, the agent and its herdr's host name), and a
Mac whose frontmost window title starts with that host name and `: ` types the key (herdr's `window_title` has to start with
`{hostname}: ` for that). Without all this, without such a key, or past row 9 the key line says how to get there, with the keys.

From anywhere in herdr, the key bound to `bhote.panel.focus` ([herdr plugin](herdr-plugin.md), e.g. prefix then `t`) jumps
to the panel and back; the panel shows it bottom right (`Ctrl+B|T ⇄` (the keys as they are on the keyboard of the machine you type on: Ctrl+Option+T on a Mac, Ctrl+Alt+T elsewhere)).

In menus and pickers: `↑↓` choose, `→`/`⏎` do it, `←`/`Esc` back; `x` dismisses a suggestion.

## The editor

New topics, titles, descriptions and names are typed on the bottom row: `←→` move, `Option+←→` (or `Ctrl`) jump a word,
`Shift` (with `Option`: by words) selects, typing replaces the selection, `Backspace`/`Delete`, `Option+Backspace` (a word),
`Ctrl-A`/`Ctrl-E` (start/end), `Ctrl-U`/`Ctrl-K` (cut to start/end). `⏎` keeps the text, `Esc` throws it away. A
description shows how many of its 140 characters are left.

## Agents

Agents are listed by project (the panel's own project first, those without one last), in two columns; the icon says the
state as in the herdr sidebar (`✶` busy, `◉` needs you, `✓` done, `○` idle). Inside a project the working agents come first
(in Claude orange as a whole name), then those that need you (red), then idle ones (grey); the project captions are coloured
by `PROJECT_COLORS`. Behind an agent's name stands the topic it works on (seven characters at most; not a parked one).
Every agent has a name of its own: herdr's workspace and pane number, `w3:a1` for the pane `w3:p1` (shown in the list, behind done topics and
as a ref in `bhote agents` and `bhote transfer`; the older code `MAC4`, a machine code and a number, still works as a ref). When the agents of
more than one machine are listed, the name is shown in front of each agent, and an agent of another machine carries its machine's **code** in
front of it: `w3:a1|Shop (main)`, `MAC/w3:a1|root` (a session in the home folder, named `<machine> / root`, keeps only `root`). A code is three capitals made from the machine's name (Mac `MAC`, Omarchy
`OMR`, MacBook Pro `MBP`), unique over all machines; settings › *Connections* › *Code of …* and the setup wizard set one by
hand (empty: made from the name again). A code set by hand is a record in the topic store, so every machine shows the same.
A machine that does not answer shows `· OMR ◐ reconnecting` in the project caption, its agents muted. The ↕ at the end of the rule above the agents
switches the agent overview to counts only (there is no drag handle; `AGENT_ROWS` sets the area's height). A topic handed to an agent shows that agent's state in its meta line (`working`, `needs you`,
`finished · x = done`, `gone`).

## Handing a topic to an agent

`s` sends the topic through `herdr agent prompt`, together with its id and how to report back: `bhote review <id>` (and,
with `AGENT_CAN_CLOSE=on`, `bhote done <id>` when nothing needs checking). The topic remembers the agent's pane and session.
When the agent reports back, or simply stops working, the topic moves to **REVIEW** at the top of the list and herdr shows a
notification (setting `NOTIFY`). Waiting topics are reminded once after `WAIT_REMIND` hours.

The `↕` button at the right end of the divider above the agents switches the overview to counts only (`✶ 3 active · ○ 4 inactive`, with
`◉ n need you` when an agent waits for an answer) and back (`AGENTS_COMPACT`). Dragging the rest of the divider still resizes the area.

## Agents that wait

With the bhote skill (installed by the setup wizard), an agent that has to wait for someone else (a deployment, a review,
infrastructure, an answer) creates a waiting topic itself, with what it will do then as the description. When the wait is
over, set the topic to now (topic menu › *The wait is over: … goes on*, or `bhote now <ref>`): the agent gets a prompt and
goes on.

## Safety

The text sent to an agent is checked live against herdr right before it is typed: the pane must still run an agent, and it
must be free. Everything printed is stripped of control characters, so no topic or agent name can send escape codes to
the terminal.
