# The panel

`bhote` without arguments is a panel for a herdr pane of about 44 columns (it works from 30 to any width; the list uses up to
60 columns). It opens with a welcome screen that closes on any key (or by itself after 15 s), then shows the topics on top
and the agents below.

The list shows what is on: **review**, **now**, **next** and **waiting**. Parked (`later`) and done topics stay out; a line at
the end counts them, and the search (find key) and the CLI reach them. A topic chosen in the search stays visible while it
is selected.

## Keys

| Key | List |
|---|---|
| `↑` `↓` `j` `k` | move |
| `⏎` | jump to the topic's agent in herdr, as a click on it does (on another machine: selected there). In another tab the focus goes to the bhote panel there, with the topic selected; in the panel's own tab the agent gets the focus. Without an agent that herdr knows now (none, gone, or its machine reconnecting) it opens the topic's page |
| `→` | the topic's page: title, description, status and the actions (start with agent, up next, waiting for, park, done, rename, describe, delete). `↑↓` also reach the title and the description; `⏎` edits them right there (`⏎` keeps, `Esc` throws away) |
| `n` | new topic: `Title`, `Title; description`, optionally ending in `@Name` or `> Name` (= waiting for Name) |
| `e` / `d` | edit the title / the description |
| `x` | mark done |
| `s` | start on a free agent: its title and description are typed into that agent's pane |
| `t` | [Steal & Transfer](steal-and-transfer.md): hand the topic and its branch from its agent to another one (asks first; warns when the target belongs to another project) |
| `a` | suggestions: what the running agents work on (their task) and is not a topic yet |
| `w` | the setup wizard's new steps, when the head says *New Features Available* |
| `,` | settings (the full key list is under *Hotkeys* there; *Connections* holds the data location, remote agents, the Slack monitor and the machine codes; *Setup wizard* runs the wizard again) · `S` welcome screen · `q` quit |

The find key (settings › *Find key*, e.g. `ctrl+alt+t`, on the Mac ⌘T) opens a search popup over all agents and topics:
type, choose, `⏎` jumps there; `x` (`ctrl+x` once something is typed) checks a topic off;
`ctrl+n` adds a topic (the search text is its start). `⏎` on a topic opens its page here in the panel.

Click a topic (any of its lines, also a done one) to select it, as the arrows do; the list stays where it is while the
selected topic is in view; a double click opens its page, where a click on an action runs it and a click on the title or
description edits it in place. Click an agent in the agent area to focus it in herdr. An agent on another machine is
selected there; herdr lets only its client switch machines, so the key line says so, with the keys (herdr 0.9.3 has no
call that switches its window to another machine).

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
Every agent has a number of its own on its machine (`MAC4`, `OMR1`, kept for good; shown in the list, behind done topics and
as a ref in `bhote agents` and `bhote transfer`). When the agents of more than one machine are
listed, each name carries its machine's **code** in front: `MAC|IMS (main)`, `OMR|root` (a session in the home folder,
named `<machine> / root`, keeps only `root`). A code is three capitals made from the machine's name (Mac `MAC`, Omarchy
`OMR`, MacBook Pro `MBP`), unique over all machines; settings › *Connections* › *Code of …* and the setup wizard set one by
hand (empty: made from the name again). A code set by hand is a record in the topic store, so every machine shows the same.
A machine that does not answer shows `· OMR ◐ reconnecting` in the project caption, its agents muted. Drag the `═══`
divider to give the agent area more or fewer rows. A topic handed to an agent shows that agent's state in its meta line (`working`, `needs you`,
`finished · x = done`, `gone`).

## Handing a topic to an agent

`s` sends the topic through `herdr agent prompt`, together with its id and how to report back: `bhote review <id>` (and,
with `AGENT_CAN_CLOSE=on`, `bhote done <id>` when nothing needs checking). The topic remembers the agent's pane and session.
When the agent reports back, or simply stops working, the topic moves to **REVIEW** at the top of the list and herdr shows a
notification (setting `NOTIFY`). Waiting topics are reminded once after `WAIT_REMIND` hours.

## Agents that wait

With the bhote skill (installed by the setup wizard), an agent that has to wait for someone else (a deployment, a review,
infrastructure, an answer) creates a waiting topic itself, with what it will do then as the description. When the wait is
over, set the topic to now (topic menu › *The wait is over: … goes on*, or `bhote now <ref>`): the agent gets a prompt and
goes on.

## Safety

The text sent to an agent is checked live against herdr right before it is typed: the pane must still run an agent, and it
must be free. Everything printed is stripped of control characters, so no topic or agent name can send escape codes to
the terminal.
