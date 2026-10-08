# Deployment and test monitor

Two optional blocks between the done topics and the agents: the last **deployments** and the last **test runs** of your
projects, five each (`MONITOR_ROWS`). The setup wizard asks for both (step *Monitors*); settings › *Connections* switches
them, their modules and their connections; settings › *Projects* › *name* › *CI* holds each project's CI.

```
  ┈┈ Deployments ┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈
  ✶ IMS · Backend                  60% · 1m
  ⏸ Bilendo Marketing · Production approval · 4m
  ✓ IMS · Frontend     feat/secure-files · 23m
  ┈┈ Tests ┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈
  ✓ IMS-wt1 · server         7508 passed · 29m
  ✗ Bilendo · build              develop · 1h
```

`✶` running (progress from the jobs' steps), `◌` queued, `⏸` waiting for an approval, `✓` done, `✗` failed, `⊘` canceled.

## Connections

| Connection | How | Used by |
|---|---|---|
| GitHub Actions | `gh` and its own login (`gh auth login`); bhote keeps no GitHub token | `DEPLOY_GITHUB`, `TEST_GITHUB` |
| CircleCI | API v2 with a personal API token: `CIRCLECI_TOKEN` in the bhote config (settings › *Connections* › *CircleCI token*, kept `0600`, never printed), else `$CIRCLECI_TOKEN`, else a `CIRCLECI_TOKEN=` line in `~/.config/zsh/secrets.zsh`. The token reaches `curl` on stdin, never on a command line. | `DEPLOY_CIRCLECI`, `TEST_CIRCLECI` |

## A project's CI

Kept in the project record (so both machines know it) and found from the repository on GitHub (`bhote monitor detect`,
the wizard, or ⏎ on *CI* in the project's settings):

| Key | GitHub Actions | CircleCI |
|---|---|---|
| `ci` | `github:<owner/repo>` | `circleci:gh/<owner/repo>` (when the repository has `.circleci/config.yml`) |
| `deploy_branches` | the default branch (`main`) | `branch=Label` from the deploy jobs' branch filters: `develop=Staging,main=Production` |
| `deploy_names` | the workflows with "deploy" in their name, `Workflow=Label` (`Backend Deployment=Backend`) | job names; empty: every job with "deploy" or "approv" in it |
| `test_names` | job names; empty: every job with "test" in it (matrix shards `Tests (1/4)` are one entry) | job names; empty: every job with "test" or "build" in it |

A deployment pipeline with a manual approval (a CircleCI `type: approval` job, a GitHub environment that waits) shows
`approval` until someone approves it. Production deployments made by hand outside CI (Capistrano from a laptop) are not seen.

## Local test runs

- **`bhote test run [-n <name>] -- <command>`** runs any test command as it is (same output, same exit code) and records
  the run: project (from the repository), start, end, exit code, and the result line of Minitest, Vitest, RSpec, pytest or
  bhote's own tests (`12 runs · 1 failures · 0 errors`). The last 100 runs are kept in `$BHOTE_DATA/testruns/`. The bhote
  skill tells Claude Code agents to run tests this way while the test monitor is on.
- **Live status files** (`TEST_STATUS_DIR`, default `~/.cache/ims-test-status`): one JSON per worktree and suite
  (`tree`, `suite`, `done`, `total`, `passed`, `failed`, `startedAt`, `updatedAt` in ms, `ok`), as IMS's Vitest reporter
  writes them. A running file that has not changed for 30 s counts as gone.

## How it runs

The collector of a machine with a monitor on fetches in the background every `DEPLOY_EVERY` / `TEST_EVERY` seconds
(default 60) and writes `deploys.list` / `tests.list` to the shared folder; the panels only read them. A run's jobs are
read once per state (cached for a day), so a quiet repository costs one `gh run list` per round. `bhote deploys` and
`bhote tests` print the lists (`--json` too) and fetch first when they are older than the interval.
