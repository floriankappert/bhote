# bhote

*by Florian Kappert and Jakob Beyer*

A side panel for [herdr](https://herdr.dev) that keeps your **topics** (what you work on, who you wait for) next to your
**agents** (who is free, who is busy), on every machine you work on. A single bash script, about 44 columns wide, with a
small sheepdog that guards the herd.

- **Topics** move through `now`, `next`, `waiting` (for a name, with a date), `review`, `later` and `done`, each with an optional description.
  The panel shows what is on (review, now, next, waiting); the last done topics sit in their own area, with the agent that did them.
- **Agents of all machines** in one list, grouped by project: working ones in Claude orange, ones that need you in red, idle ones grey,
  with the topic each works on. Every agent has a fixed number on its machine (`MAC4`, `OMR1`), also a ref for the CLI.
- **Start a topic on a free agent** (`s`): bhote sends title and description to its pane. With `AUTO_ASSIGN` a `now` topic is started by itself on the next free agent of its project.
- **Review topics by themselves:** an agent that asks you a question or waits for an answer gets a review topic (`Question from <agent>`); it goes when the agent works again (`AUTO_REVIEW`).
- **Waiting:** agents that have to wait for someone create a waiting topic themselves (bhote skill for Claude Code); when you mark the wait as over, the agent goes on. A waiting topic can carry a **Slack pin** (channel, DM or thread); when someone writes there, it goes to review. Claude Code and its Slack connector do the reading, so bhote needs no Slack token.
- **Steal & Transfer:** an agent commits its state with a handover on its branch, and a free agent takes over ([how](docs/steal-and-transfer.md)).
- **Projects:** an agent belongs to a project by its git repository (worktrees inherit); the panel can list only its project's topics. **GitHub:** the branch a topic's agent works on.
- **Deployment and test monitor:** the last deployments (GitHub Actions, CircleCI with its approvals) and test runs (CI test jobs, local runs through `bhote test run`, live status files) of your projects, seven each, with progress bars, above the agents.
- **Search from anywhere in herdr** (the find key, e.g. `ctrl+alt+t`): all agents and topics, `⏎` jumps there, `ctrl+n` adds a topic.
- **Topics are plain files**, kept in sync with a second machine through herdr's own connection (opt-in); **update notice** when a newer version is out.
- **Omarchy bar widget**, themes (terminal colours or Catppuccin, each role overridable), a **setup wizard** and the Claude skill `/bhote-install`.
- A **CLI** (every command with `--json`) to create and close topics from scripts and agents.

## Install

Needs bash 3.2+, `jq` and herdr. macOS and Linux.

**Recommended: install it, then let Claude Code finish the setup.** Two commands, then one word in a Claude Code session:

```sh
brew install floriankappert/bhote/bhote      # Arch/Omarchy and other Linux: see below
bhote skills install                         # the Claude Code skills bhote and bhote-install, in every Claude profile
```

In Claude Code: **`/bhote-install`**. The skill installs the herdr plugin, the Omarchy bar widget, the Claude hook and skills,
the settings, the connection to your other machines and the deployment and test monitor, and checks each step. (The
interactive wizard `bhote setup` does the same by hand.)

**macOS** (Homebrew):

```sh
brew install floriankappert/bhote/bhote
herdr plugin link "$(brew --prefix)/share/bhote/herdr-plugin"
```

**Arch Linux / Omarchy** (the package from `packaging/arch`; it also brings the Omarchy bar widget):

```sh
git clone https://github.com/floriankappert/bhote && cd bhote/packaging/arch && makepkg -si
herdr plugin link /usr/share/bhote/herdr-plugin
# Omarchy: the bar widget
ln -s /usr/share/bhote/omarchy-plugin ~/.config/omarchy/plugins/bhote.bar && omarchy plugin enable bhote.bar --section right
```

**Any Linux** (also Homebrew on Linux works with the command above), or from a checkout on any system:

```sh
git clone https://github.com/floriankappert/bhote && cd bhote && ./install.sh
```

After an update, `bhote reload` restarts the panels of the machine (never type into a running panel: keys act as hotkeys there).

`install.sh` links `~/.local/bin/bhote` (update with `git pull`) and, when herdr is present, the plugin `bhote.panel`, which
opens the panel on the right of the agent pane of every tab when herdr starts (`AUTOSTART=off` in the config switches that
off; the action "Bhote: open panel" opens it by hand). On Omarchy it also links the bar widget `bhote.bar`: the
number of things for you (topics in review, agents that need an answer), the list on hover, a click jumps to the bhote
panel in herdr (right click: the search). `bhote bar` prints the same as Waybar-style JSON for any other bar.

## Keys

| key | does |
|---|---|
| `↑↓` / `jk` | move |
| `⏎` | jump to the topic's agent in herdr (in another tab the focus goes to the bhote panel there; no agent: the topic's page) |
| `→`, `←` / `esc` | the topic's page / go back |
| `n` | new topic: `Title`, `Title; description`, optionally ending in `@Name` or `> Name` (= waiting for Name) |
| `s` | start the topic on a free agent |
| `t` | Steal & Transfer to another agent |
| `e` / `d` | edit the title / the description (Esc cancels) |
| `x` | mark done |
| `a` | topics suggested from what the running agents work on |
| `,` | settings · `S` welcome screen · `w` the new wizard steps · `q` quit |
| click an agent | focus it in herdr (on another machine: selected there; the panel says which keys switch to it) |
| find key (e.g. `ctrl+alt+t`, ⌘T) | search all agents and topics, jump there (`bhote find` as a herdr popup) |
| herdr prefix, `t` | jump from the agent to the panel and back (the plugin action `bhote.panel.focus`; the keys are shown bottom right) |

The full list is in [the panel](docs/panel.md).

## CLI

```
bhote add <title> [-d <text>] [@Name]   new topic          bhote list [--all] [--json] [--status s]
bhote show <ref> [--json]               one topic          bhote now|next|review|later|done <ref>
bhote wait <ref> <Name>                 waiting for Name   bhote slack <ref> <pin|off>   bhote rename|desc <ref> <text>   bhote rm <ref>
bhote agents                            the agents, numbered (MAC4)        bhote transfer <from> <to>   Steal & Transfer
bhote deploys · bhote tests             the monitors        bhote test run -- <cmd>   a test run they see
bhote sync · bhote config               merge now · read and change the settings
bhote setup · bhote skills install      the wizard · the Claude skills        bhote update · bhote reload   newer version · restart panels
bhote find · bhote bar                  the search · a status bar line (JSON)
```

`<ref>` is the number from `bhote list`, an id, or a part of the title. Everything is in the [CLI reference](docs/cli-reference.md).

## Settings

Stored in `~/.config/bhote/config` (`KEY=value`, never sourced); the panel's settings screen (`,`) edits them, `bhote config list`
shows all. The most used; every key is in [configuration](docs/configuration.md):

| key | default | meaning |
|---|---|---|
| `STORE`, `STORE_MACHINE` | `local` | keep a replica on a saved herdr machine (`STORE=remote`, `STORE_MACHINE=<label>`) |
| `REMOTE_AGENTS` | `off` | also list the agents of the saved herdr machines (opens ssh connections, only when `on`) |
| `AUTO_ASSIGN` | `off` | a `now` topic is started by itself on the next free agent of its project |
| `AUTO_REVIEW` | `on` | an agent's question becomes a review topic |
| `SLACK_WATCH` | `off` | watch the Slack pins of waiting topics |
| `DEPLOY_MONITOR`, `TEST_MONITOR` | `off` | the deployment and test monitor |
| `FIND_KEY`, `JUMP_KEY` | | the herdr keys for the search and for jumping between agent and panel |
| `THEME` | `terminal` | `terminal` or `catppuccin` |
| `AUTOSTART` | `on` | the herdr plugin opens the panel at start |
| `HOST_LABEL` | `Mac` / host name | how this machine is called in the lists |
| `LOCAL_EVERY`, `REMOTE_EVERY` | `3`, `5` | seconds between agent queries |

Data lives in `${XDG_DATA_HOME:-~/.local/share}/bhote/topics` (one small file per topic). With a replica machine, the local copy
stays the working copy; the newer `updated` wins per topic, deletions travel as tombstones, and while the machine is not
reachable bhote works offline and merges later.

## Safety

bhote opens no connection to another machine unless you switch it on. Names that arrive from a replica are validated before they
become files; control characters are stripped from everything shown or sent; private (0700) scratch and shared folders; one
panel per machine collects agent data, the others read its result. `tests/` holds the checks (`bash tests/smoke.sh`,
`cli.sh`, `agents.sh`, `store.sh`, `slack.sh`, `monitor.sh`).

MIT licensed. Documentation: [docs/](docs/README.md) — start with the [CLI reference](docs/cli-reference.md).
