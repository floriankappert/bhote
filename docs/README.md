# bhote documentation

*bhote by Florian Kappert and Jakob Beyer — a native add-on for the herdr Mac app: topics, agents, servers and headless agents.*

| | |
|---|---|
| [The herdr Mac app add-on](herdr-app.md) | What bhote is since 0.9.3: install, what it shows, messages between agents, dev servers. |
| [CLI reference](cli-reference.md) | Every command, topic refs, JSON output, exit codes, environment. The CLI is the API. |
| [JSON schemas](schema/) | [`topic.schema.json`](schema/topic.schema.json), [`topic-list.schema.json`](schema/topic-list.schema.json) (JSON Schema 2020-12). |
| [Steal & Transfer](steal-and-transfer.md) | Hand an agent's work and branch to another agent. |
| [Claude Code mod](claude-mod.md) | The topic as a band above the prompt, and `/topic`: a Claude Code mod (`bhote mod install`). |
| [Setup wizard](setup.md) | The first-start wizard: checks, machines both ways, sync, agents, notifications. |
| [The terminal UI](panel.md) | *Deprecated since 0.9.3.* Keys, the editor, agents, safety. |
| [Configuration](configuration.md) | Every setting in `~/.config/bhote/config`. |
| [Deployment and test monitor](monitors.md) | The last deployments and test runs above the agents: GitHub Actions, CircleCI, local runs. |
| [Data and sync](data-and-sync.md) | Topic files, the replica and how it merges, what panels share. |
| [herdr plugin](herdr-plugin.md) | *Deprecated since 0.9.3.* Opening the terminal panel next to your agents. |
| [Architecture](architecture.md) | For contributors: processes, shared files, the sections of the script, how a frame is drawn. |
