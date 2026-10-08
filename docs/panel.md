# The panel

`bhote` without arguments is a panel for a herdr pane of about 44 columns (it works from 30 to any width; the list uses up to
60 columns). It opens with a welcome screen that closes on any key (or by itself after 15 s), then shows the topics on top
and the agents below.

## Keys

| Key | List |
|---|---|
| `↑` `↓` `j` `k` | move |
| `→` `⏎` | the topic's menu (start with agent, waiting for, park, done, rename, describe, delete) |
| `n` | new topic: `Title`, `Title; description`, optionally ending in `@Name` or `> Name` (= waiting for Name) |
| `e` / `d` | edit the title / the description |
| `x` | mark done |
| `s` | start on a free agent: its title and description are typed into that agent's pane |
| `t` | [Steal & Transfer](steal-and-transfer.md): hand the topic and its branch from its agent to another one |
| `a` | suggestions: what the running agents work on (their task) and is not a topic yet |
| `,` | settings (the full key list is under *Hotkeys* there) · `S` welcome screen · `q` quit |

From anywhere in herdr, the key bound to `bhote.panel.focus` ([herdr plugin](herdr-plugin.md), e.g. prefix then `t`) jumps
to the panel and back; the panel shows it bottom right (`^b t ⇄`).

In menus and pickers: `↑↓` choose, `→`/`⏎` do it, `←`/`Esc` back; `x` dismisses a suggestion.

## The editor

New topics, titles, descriptions and names are typed on the bottom row: `←→` move, `Option+←→` (or `Ctrl`) jump a word,
`Shift` (with `Option`: by words) selects, typing replaces the selection, `Backspace`/`Delete`, `Option+Backspace` (a word),
`Ctrl-A`/`Ctrl-E` (start/end), `Ctrl-U`/`Ctrl-K` (cut to start/end). `⏎` keeps the text, `Esc` throws it away. A
description shows how many of its 140 characters are left.

## Agents

Agents are split into **free** (idle, done) and **busy** (working, blocked, running tests with `TESTS_BUSY=on`). Agents of
other machines carry the machine's initial unless their name starts with it. Drag the `═══` divider to give the agent area
more or fewer rows. A topic handed to an agent shows that agent's state in its meta line (`working`, `needs you`,
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
