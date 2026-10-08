# Security policy

bhote runs on your machines with your rights: it opens ssh or herdr connections to the machines you add, types text into
agent panes, reads a CircleCI token from its config and runs Claude Code for the Slack watch. We take reports about any
of that seriously.

## Supported versions

Only the latest release gets fixes. Update with `bhote update` (it prints the command for your installation).

## Reporting a vulnerability

Please do **not** open a public issue. Report it privately through GitHub:
[Security › Report a vulnerability](https://github.com/floriankappert/bhote/security/advisories/new).

Tell us what an attacker can do, the version (`bhote version`), your system, and the steps to reproduce. We answer
within a week and agree with you on when the fix and the advisory go public.

## Scope

In scope: anything that lets data from another machine, a topic, a Slack message, an agent pane, herdr or the network
run code, write outside bhote's folders, reach a machine or service the user did not switch on, or expose a secret.

Out of scope: a user who already controls your account or your config file.
