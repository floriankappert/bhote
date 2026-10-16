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
- **`/topic`** changes the topic of the agent without a Claude turn (also while Claude works, no tokens):
  `/topic` shows it, `/topic review|done|now|next|later|testing|dismiss` sets the status, `/topic wait <Name>` waits for a name,
  `/topic hide` and `/topic show` hide and show the band.

The mod only calls the CLI (`bhote current --json`, `bhote <status> <id>`), so it finds the topic the way `bhote current` does
(Claude session, else herdr pane). An agent without a topic, or one whose topic is done, shows no band.

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
