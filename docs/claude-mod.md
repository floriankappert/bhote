# Claude Code mod: bhote-topic

A [Claude Code mod](https://code.claude.com/docs/en/plugins/mods/overview) that puts the task of the agent into the Claude Code
session itself, so you see it without looking at the panel. It needs Claude Code 2.1.287 or newer (terminal and desktop app).

- **A friendly reminder above the prompt:** now and then, at random and not often (about every ten minutes at the earliest, after a turn
  or between turns, for 45 seconds), a rounded orange box with `🐶` and the title of a task that waits, is next or is parked, and its
  description under it. It is a reminder ("did you still have this on your screen?"), not a status: the band that showed the status of the agent
  is gone. Which task is chosen at random (`bhote reminder`): waiting, next or parked ones, not a wait for a deployment, not an automatic card,
  not a dismissed one. Settings › *Show Friendly Reminder* (`FRIENDLY_REMINDER`, on) switches it off. `[-]` (or `ctrl+x ctrl+a`) collapses it.
- **Progress bars** under the title: a bar for each test suite that runs in the agent's worktree (`Server ■■■□□ 50% (5/10)`, from the live
  status files, see [monitors](monitors.md)), and one for the deployment its task waits for (`IMS · Backend ■■■□□ 60% (3/5)`; `queued`,
  `needs approval` and `failed` as words). A deployment the agent starts by pushing its branch is bound to the task by bhote, so it shows up
  without anyone asking. The task is read every 5 s while a turn runs or something is under way, else once a minute.
- **A pane beside the chat** (`/task pane`, again to close): the whole picture of the task in a box docked beside the transcript, in the
  fullscreen layout (`"tui": "fullscreen"`) from 110 columns (else it sits above the prompt, or waits until the terminal is wide enough). Title, status
  and age, the description (Markdown), project, agent, machine and branch, who it waits for and since when, the Slack pin, the pull request of the
  branch (`#26 open ✓3 ✗0 ◌0`, a link) and its checks, the test and deployment bars (the deployment with a link), the review summary with the
  review card's buttons (a button answers the agent and puts the task back on now), the other tasks that are on the cards of the panel (of every machine kharka reaches, each with the agent's code `MAC4`/`OMR3`: in review, now, testing, deploying, recently finished, each with its count and `+n more`; every item has two lines (the title; under it `∟`, the agent's address and when) and a press on either line shows the agent in herdr (`bhote jump`); next,
  waiting, later, done and archived ones are not shown), and buttons for Review, Done, Next and Later. It is read every 10 s while it is open (`bhote current --json --full`). 
- **`/task`** changes the task of the agent without a Claude turn (also while Claude works, no tokens):
  `/task` shows it, `/task review|done|now|next|later|testing|dismiss` sets the status, `/task wait <Name>` waits for a name,
  `/task hide` and `/task show` hide and show the reminder.

The mod only calls the CLI (`bhote current --json`, `bhote <status> <id>`), so it finds the task the way `bhote current` does
(Claude session, else herdr pane). The reminder does not depend on the agent: it comes from any task of your list.

## Messages between agents: the inbox hook

`bhote send <agent> <text>` keeps a message in kharka; the herdr-app hands it to an agent that is free. An agent that works for an hour never is. The hook `integrations/claude/hooks/inbox-hook.sh` brings the messages to it at its next tool call (event `PostToolUse`) or prompt (`UserPromptSubmit`; not `Stop`) as additional context: who wrote, the text (a long one: the `kharka get` command), and `bhote send <name> "..."` to answer. bhote does not edit your settings; add this to the `settings.json` of each Claude profile:

```json
{ "hooks": {
    "UserPromptSubmit": [ { "hooks": [ { "type": "command", "command": "~/Development/bhote/integrations/claude/hooks/inbox-hook.sh", "timeout": 10 } ] } ],
    "PostToolUse":      [ { "hooks": [ { "type": "command", "command": "~/Development/bhote/integrations/claude/hooks/inbox-hook.sh", "timeout": 10 } ] } ]
} }
```

Cost: when nothing waits, one directory test and no process (about 5 ms with the bash start); only when a marker for this agent exists does it run `bhote inbox --hook <event>` (about 0.1 to 0.4 s, once per message). It needs `HC_TERMINAL_ID` (an agent of herdr), `bhote` on the `PATH` (else it uses its own checkout) and kharka. It shows only this agent's messages, and respects the ear (off = nothing). `bhote inbox [--peek] [--json]` shows them by hand. The markers and the claim are described in [herdr-app](herdr-app.md#agents-and-messages). Other machines: the hook runs on the machine of the receiving agent; a message from another machine arrives once the panel there has marked it (a few seconds).

## Switching it off

`bhote config set CLAUDE_MOD off` (or settings › *Claude Code mod*) switches the display off on this machine: the pane is empty, `/task` says so (the reminder has its own switch, `FRIENDLY_REMINDER`),
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

A mod runs with your permissions and is not sandboxed; this one reads tasks and runs `bhote` and nothing else
(`claude plugin validate integrations/claude/mods/bhote-topic` lists its calls).

## Develop

The code is `integrations/claude/mods/bhote-topic/hooks/register.tsx`. Try a change without installing:
`claude --plugin-dir integrations/claude/mods/bhote-topic`; `claude plugin test integrations/claude/mods/bhote-topic` runs its test.
