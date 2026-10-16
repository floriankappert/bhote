# Claude Code mod: bhote-topic

A [Claude Code mod](https://code.claude.com/docs/en/plugins/mods/overview) that puts the topic of the agent into the Claude Code
session itself, so you see it without looking at the panel. It needs Claude Code 2.1.287 or newer (terminal and desktop app).

- **A band above the prompt:** a rounded box in the colour of the status, `🐕 title` and the description on the left, the status
  as a badge on the right (`NOW`, `TESTING`, `NEXT`, `REVIEW`, `LATER`, `DONE`, or `WAITING FOR <name>`) with its age (`3min ago`)
  under it. The age moves on by itself every minute. The colours are theme colours: now orange, testing purple, next blue,
  review red, waiting yellow, later grey, done green. `[-]` (or `ctrl+x ctrl+a`) collapses the band, as it does for any band.
- **Progress bars** under the title: a bar for each test suite that runs in the agent's worktree (`Server ■■■□□ 50% (5/10)`, from the live
  status files, see [monitors](monitors.md)), and one for the deployment its topic waits for (`IMS · Backend ■■■□□ 60% (3/5)`; `queued`,
  `needs approval` and `failed` as words). A deployment the agent starts by pushing its branch is bound to the topic by bhote, so it shows up
  without anyone asking. The topic is read every 5 s while a turn runs or something is under way, else once a minute.
- **A pane beside the chat** (`/topic pane`, again to close): the whole picture of the topic in a box docked beside the transcript, in the
  fullscreen layout (`"tui": "fullscreen"`) from 110 columns (else it sits above the prompt, or waits until the terminal is wide enough). Title, status
  and age, the description (Markdown), project, agent, machine and branch, who it waits for and since when, the Slack pin, the pull request of the
  branch (`#26 open ✓3 ✗0 ◌0`, a link) and its checks, the test and deployment bars (the deployment with a link), the review summary with the
  review card's buttons (a button answers the agent and puts the topic back on now), the other topics that are on the cards of the panel (of every machine kharka reaches, each with the agent's code `MAC4`/`OMR3`: in review, now, testing, deploying, recently finished, each with its count and `+n more`; every item has two lines (the title; under it `∟`, the agent's address and when) and a press on either line shows the agent in herdr (`bhote jump`); next,
  waiting, later, done and archived ones are not shown), and buttons for Review, Done, Next and Later. It is read every 10 s while it is open (`bhote current --json --full`). The band above the prompt stays, as the small
  version for a narrow terminal.
- **`/topic`** changes the topic of the agent without a Claude turn (also while Claude works, no tokens):
  `/topic` shows it, `/topic review|done|now|next|later|testing|dismiss` sets the status, `/topic wait <Name>` waits for a name,
  `/topic hide` and `/topic show` hide and show the band.

The mod only calls the CLI (`bhote current --json`, `bhote <status> <id>`), so it finds the topic the way `bhote current` does
(Claude session, else herdr pane). An agent without a topic, or one whose topic is done, shows no band.

## Switching it off

`bhote config set CLAUDE_MOD off` (or settings › *Claude Code mod*) switches the display off on this machine: the band and the pane are empty, `/topic` says so,
and the mod asks bhote for nothing but once a minute whether it may draw again. It needs no `/reload-plugins`. The other machines have a setting of their own.

## Install

```sh
bhote mod install        # every Claude profile; also offered by the setup wizard and /bhote-install
bhote mod status         # where it is installed
bhote mod remove
```

`integrations/claude` of bhote is a marketplace named `bhote`; `bhote-topic` is a plugin of it. `bhote mod install` adds that
marketplace to each Claude profile (`CLAUDE_CONFIG_DIR`, `~/.claude`, `~/.claude*`) and installs the plugin, `claude plugin
update` does the rest after a bhote update (run `bhote mod install` again). Restart Claude Code or run `/reload-plugins`.

A mod runs with your permissions and is not sandboxed; this one reads topics and runs `bhote` and nothing else
(`claude plugin validate integrations/claude/mods/bhote-topic` lists its calls).

## Develop

The code is `integrations/claude/mods/bhote-topic/hooks/register.tsx`. Try a change without installing:
`claude --plugin-dir integrations/claude/mods/bhote-topic`; `claude plugin test integrations/claude/mods/bhote-topic` runs its test.
