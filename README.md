# bhote

*by Florian Kappert and Jakob Beyer*

A side panel for [herdr](https://herdr.dev) that keeps your **topics** (what you work on, who you wait for) next to your
**agents** (who is free, who is busy). A single bash script, about 44 columns wide, with a small sheepdog that guards the herd.

- Topics are `now`, `waiting` (for a name, with a date), `later` or `done`, each with an optional description.
- Agents of every herdr tab are listed by project, then machine; the icon says free (○ ✓) or busy (the dancing star).
- Start a topic on a free agent: bhote sends the title and description to its pane (`herdr pane run`, with `--machine` for agents on another machine).
- Agents' tasks that are no topic yet are suggested (`a`); a finished agent is marked, `x` closes its topic.
- Topics are plain files; they can be kept in sync with a second machine over ssh (opt-in).
- **Steal & Transfer**: an agent commits its state with a handover on its branch, and a free agent takes over ([how](docs/steal-and-transfer.md)).
- Agents that have to wait for someone create a waiting topic themselves (bhote skill for Claude Code); when you mark the wait as over, the agent goes on.
- **Projects**: agents are listed by project (and machine); an agent belongs to a project by its git repository (worktrees inherit), and the panel can list only its project's topics.
- **GitHub**: the git branch a topic's agent works on, under the topic and in the search.
- **Deployment and test monitor**: the last deployments (GitHub Actions, CircleCI with its approvals) and test runs (CI test
  jobs, local runs through `bhote test run`, live status files) of your projects, five each, above the agents.
- **Omarchy bar widget**: what waits for you, in the bar; a click jumps to the panel.
- **Slack pins**: a waiting topic can carry a Slack channel, DM or thread; when someone writes there, it goes to review. Claude Code (its Slack connector) does the reading, so bhote needs no Slack token.
- Themes: follows your terminal colours (and Omarchy's system theme) by default, or exact Catppuccin colours, each role overridable.
- A setup wizard on the first start: machines in both directions, sync through herdr, notifications.
- A CLI (every command with `--json`) to create and close topics from scripts and agents.

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

`install.sh` links `~/.local/bin/bhote` (update with `git pull`) and, when herdr is present, the plugin `bhote.panel`, which
opens the panel on the right of the agent pane of every tab when herdr starts (`AUTOSTART=off` in the config switches that
off; the action "Bhote: open panel" opens it by hand). On Omarchy it also links the bar widget `bhote.bar`: the
number of things for you (topics in review, agents that need an answer), the list on hover, a click jumps to the bhote
panel in herdr (right click: the search). `bhote bar` prints the same as Waybar-style JSON for any other bar.

## Keys

| key | does |
|---|---|
| `↑↓` / `jk` | move |
| `⏎` | jump to the topic's agent in herdr (in another tab the focus stays in the bhote panel there, the topic selected; no agent: the topic's page) |
| `→`, `←` / `esc` | the topic's page / go back |
| `n` | new topic: `Title`, `Title; description`, optionally ending in `@Name` or `> Name` (= waiting for Name) |
| `s` | start the topic on a free agent |
| `t` | Steal & Transfer to another agent |
| `e` / `d` | edit the title / the description (Esc cancels) |
| `x` | mark done |
| `a` | topics suggested from what the running agents work on |
| `,` | settings · `S` welcome screen · `q` quit |
| click an agent | focus it in herdr (on another machine: selected there, switch to it in herdr) |
| find key (e.g. `ctrl+alt+t`, ⌘T) | search all agents and topics, jump there (`bhote find` as a herdr popup) |
| herdr prefix, `t` | jump from the agent to the panel and back (a herdr key for the plugin action `bhote.panel.focus`; shown bottom right) |

## CLI

```
bhote add <title> [-d <text>] [@Name]   new topic          bhote list [--all] [--json] [--status s]
bhote show <ref> [--json]               one topic          bhote done|now|later <ref>
bhote wait <ref> <Name>                 waiting for Name   bhote rename|desc <ref> <text>    bhote rm <ref>
bhote sync                              merge with the data location
bhote deploys · bhote tests             the monitors        bhote test run -- <cmd>   a test run they see
bhote monitor detect                    each project's CI    bhote bar                 a status bar line (JSON)
```

`<ref>` is the number from `bhote list`, an id, or a part of the title.

## Settings

Stored in `~/.config/bhote/config` (`KEY=value`, never sourced); the panel's settings screen (`,`) edits the common ones.

| key | default | meaning |
|---|---|---|
| `STORE`, `STORE_MACHINE` | `local` | keep a replica on a saved herdr machine (`STORE=remote`, `STORE_MACHINE=<label>`) |
| `REMOTE_AGENTS` | `off` | also list the agents of the saved herdr machines (opens ssh connections, only when `on`) |
| `MACHINES_OFF` | | saved machines that stay out |
| `TESTS_BUSY` | `off` | an agent whose workspace runs tests counts as busy (reads the `tests` workspace token) |
| `SPLASH_AUTOCLOSE`, `SPLASH_SECONDS` | `on`, `15` | the welcome screen closes by itself |
| `HOST_LABEL` | `Mac` / host name | how this machine is called in the lists |
| `NAME_COLORS` | | colour agents by part of their name, e.g. `api=teal,web=mauve` |
| `AUTOSTART` | `on` | the herdr plugin opens the panel at start |
| `LOCAL_EVERY`, `REMOTE_EVERY` | `3`, `10` | seconds between agent queries |

Data lives in `${XDG_DATA_HOME:-~/.local/share}/bhote/topics` (one small file per topic). With a replica machine, the local copy
stays the working copy; the newer `updated` wins per topic, deletions travel as tombstones, and while the machine is not
reachable bhote works offline and merges later.

## Safety

bhote opens no connection to another machine unless you switch it on. Names that arrive from a replica are validated before they
become files; control characters are stripped from everything shown or sent; private (0700) scratch and shared folders; one
panel per machine collects agent data, the others read its result. `tests/` holds the checks (`bash tests/smoke.sh`,
`cli.sh`, `agents.sh`, `store.sh`, `slack.sh`, `monitor.sh`).

MIT licensed. Documentation: [docs/](docs/README.md) — start with the [CLI reference](docs/cli-reference.md).
