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
| click a topic | select it (any of its lines; the list does not scroll while it is in view); a double click opens its page, where a click runs an action or edits the title or description |
| click an agent | focus it in herdr (on another machine: selected there; the panel says which keys switch to it) |
| find key (e.g. `ctrl+alt+t`, ⌘T) | search all agents and topics, jump there (`bhote find` as a herdr popup) |
| herdr prefix, `t` | jump from the agent to the panel and back (the plugin action `bhote.panel.focus`; the keys are shown bottom right) |

The full list is in [the panel](docs/panel.md).

## CLI reference

The CLI is the API: the panel, scripts and agents all work on the same topics through it. Every command takes `--json`
(one document per call), results go to stdout, messages to stderr. Exit codes: `0` ok · `1` error · `2` the ref matches
more than one topic. `<ref>` is a number from `bhote list --all`, an id (or its start), or part of the title.
Full detail: **[docs/cli-reference.md](docs/cli-reference.md)**.

<table>
<tr><th width="150">Topics</th><th></th></tr>
<tr><td><code>bhote add &lt;title&gt;</code></td><td>New topic. <code>-d</code> description · <code>@Name</code> / <code>-w Name</code> waiting for Name · <code>-s now|next|later|done</code> · <code>-p</code> project · <code>--slack</code> pin</td></tr>
<tr><td><code>bhote list</code> · <code>show</code></td><td>The open topics, numbered (<code>--all</code> adds done ones, <code>-s</code> filters) · one topic</td></tr>
<tr><td><code>bhote now|next|review|later|done</code></td><td>Start or resume · up next · ready for review · park · mark done</td></tr>
<tr><td><code>bhote wait &lt;ref&gt; &lt;Name&gt;</code></td><td>Waiting for Name, from now</td></tr>
<tr><td><code>bhote slack &lt;ref&gt; &lt;pin|off&gt;</code></td><td>Pin a Slack channel, DM or thread: a message from someone else sets the waiting topic to review</td></tr>
<tr><td><code>bhote rename</code> · <code>desc</code> · <code>rm</code></td><td>New title · new description (140 characters) · delete</td></tr>
<tr><td><code>bhote project</code></td><td>List, add, rename, remove projects; pin repositories and workspaces to them</td></tr>
<tr><th>Agents</th><th></th></tr>
<tr><td><code>bhote agents</code></td><td>The agents of herdr, numbered, with their codes (<code>MAC4</code>, <code>OMR1</code>)</td></tr>
<tr><td><code>bhote take &lt;ref&gt;</code></td><td>Run by an agent: it takes the topic over</td></tr>
<tr><td><code>bhote transfer &lt;from&gt; &lt;to&gt;</code></td><td>Steal &amp; Transfer: <code>from</code> commits and hands its work over to <code>to</code> (<code>--force</code> for another project)</td></tr>
<tr><td><code>bhote current</code></td><td>The topic of the agent that asks (for a Claude Code SessionStart hook)</td></tr>
<tr><th>Monitors</th><th></th></tr>
<tr><td><code>bhote deploys</code> · <code>tests</code></td><td>The deployment / test monitor, newest first</td></tr>
<tr><td><code>bhote test run -- &lt;cmd&gt;</code></td><td>Run tests so that the test monitor sees them (output and exit code unchanged)</td></tr>
<tr><td><code>bhote monitor detect</code></td><td>Find each project's CI from its repository (GitHub Actions, CircleCI)</td></tr>
<tr><th>Machine</th><th></th></tr>
<tr><td><code>bhote setup</code> · <code>skills install</code></td><td>The setup wizard · the Claude skills <code>bhote</code> and <code>bhote-install</code> into every profile</td></tr>
<tr><td><code>bhote config</code></td><td><code>list</code> · <code>get</code> · <code>set</code> · <code>unset</code></td></tr>
<tr><td><code>bhote sync</code></td><td>Merge with the data location now</td></tr>
<tr><td><code>bhote update</code> · <code>reload</code></td><td>Is there a newer version, and how to install it · restart the panels of this machine</td></tr>
<tr><td><code>bhote find</code> · <code>bar</code></td><td>Search and jump (a herdr popup) · one status bar line as Waybar-style JSON</td></tr>
</table>

```sh
id=$(bhote add "Migrate the billing export" -d "CSV to Parquet" --json | jq -r .id)   # keep the id, not the number
bhote wait "$id" DevOps && bhote now "$id" && bhote done "$id"
```

## Configuration reference

Settings live in `~/.config/bhote/config` (`KEY=value` lines, never sourced, so a value cannot run as code). Edit them in the
panel (`,`) or with `bhote config set KEY value`; `bhote config list` shows all. Nothing that reaches another machine is on
by default. Full detail: **[docs/configuration.md](docs/configuration.md)**.

<details open>
<summary><b>Machines and sync</b></summary>

| Key | Default | |
|---|---|---|
| `STORE` · `STORE_MACHINE` | `local` | `remote`: keep a replica of the topics on a saved herdr machine |
| `SYNC_VIA` | `herdr` | How topics travel: `herdr` (its own connection) or `ssh` |
| `REMOTE_AGENTS` | `off` | Also list the agents of the saved herdr machines |
| `MACHINES_OFF` | | Machines that stay out |
| `PUSH_STATES` | `on` | Tell other machines at once when an agent here changes state |
| `HOST_LABEL` | `Mac` / host | How this machine is called in the lists |
| `LOCAL_EVERY` · `REMOTE_EVERY` | `3` · `5` | Seconds between agent queries |
</details>

<details open>
<summary><b>Agents and topics</b></summary>

| Key | Default | |
|---|---|---|
| `AUTO_ASSIGN` | `off` | A `now` topic goes to the next free agent of its project, 20 s after its last change |
| `AUTO_ASSIGN_MACHINE` | `any` | Whose free agents come first |
| `AUTO_REVIEW` | `on` | An agent's question becomes a review topic |
| `AGENT_CAN_CLOSE` | `on` | An agent may mark its topic done; `off`: only review |
| `NOTIFY` | `on` | herdr notifications: ready, done, waits too long |
| `WAIT_REMIND` | `24` | Hours until a waiting topic is reminded (`0`: never) |
| `TESTS_BUSY` | `off` | An agent whose workspace runs tests counts as busy |
| `GITHUB_BRANCHES` | `on` | Show the git branch a topic's agent works on |
| `SLACK_WATCH` · `SLACK_EVERY` | `off` · `120` | Watch Slack pins through Claude Code, every N seconds |
| `CLAUDE_CONFIG_DIR` | | The Claude profile the Slack watch runs in |
</details>

<details>
<summary><b>Monitors</b></summary>

| Key | Default | |
|---|---|---|
| `DEPLOY_MONITOR` · `TEST_MONITOR` | `off` | The deployment / test monitor above the agents |
| `DEPLOY_EVERY` · `TEST_EVERY` | `60` | Seconds between checks |
| `DEPLOY_GITHUB` · `DEPLOY_CIRCLECI` | `on` | Deployment modules |
| `TEST_LOCAL` · `TEST_GITHUB` · `TEST_CIRCLECI` | `on` | Test modules |
| `TEST_STATUS_DIR` | `~/.cache/ims-test-status` | Live test status files |
| `CIRCLECI_TOKEN` | | CircleCI API token (a secret, never printed) |
| `MONITOR_ROWS` | `7` | Entries per monitor (the rest is counted) |
</details>

<details>
<summary><b>Panel, keys and colours</b></summary>

| Key | Default | |
|---|---|---|
| `AUTOSTART` | `on` | The herdr plugin opens the panel when herdr starts |
| `FIND_KEY` · `JUMP_KEY` | | The herdr keys for the search popup and for jumping between agent and panel |
| `HERDR_KEYS_MACHINE` | `any` | The machine whose herdr you type into (the keys shown are read from it) |
| `PANEL_MIN_WIDTH` · `PANEL_MAX_WIDTH` | `36` · `60` | Width limits of the panel pane |
| `TOPICS_SCOPE` | `all` | `project`: only the topics of the panel's project |
| `DONE_MAX` | `7` | Done topics shown (`0`: none) |
| `AGENT_ROWS` | automatic | Height of the agent area |
| `SPLASH_AUTOCLOSE` · `SPLASH_SECONDS` | `on` · `15` | The welcome screen closes by itself |
| `UPDATE_CHECK` | `on` | Look for a newer version every 6 hours |
| `THEME` | `terminal` | `terminal` follows your terminal and system theme, `catppuccin` is exact |
| `PROJECT_COLORS` · `NAME_COLORS` | | Colours of project captions and agent names |
| `COLOR_<ROLE>` | | Override one role with `#rrggbb` (`TEXT`, `DIM`, `FRAME`, `SELECTION`, `ACCENT`, `NOW`, `WAITING`, `REVIEW`, `LATER`, `DONE`, `ERROR`) |
</details>

## Data

Data lives in `${XDG_DATA_HOME:-~/.local/share}/bhote/topics` (one small file per topic). With a replica machine, the local copy
stays the working copy; the newer `updated` wins per topic, deletions travel as tombstones, and while the machine is not
reachable bhote works offline and merges later.

## Safety

bhote opens no connection to another machine unless you switch it on. Names that arrive from a replica are validated before they
become files; control characters are stripped from everything shown or sent; private (0700) scratch and shared folders; one
panel per machine collects agent data, the others read its result. `tests/` holds the checks (`bash tests/smoke.sh`,
`cli.sh`, `agents.sh`, `store.sh`, `slack.sh`, `monitor.sh`).

MIT licensed. Documentation: [docs/](docs/README.md) — start with the [CLI reference](docs/cli-reference.md).
