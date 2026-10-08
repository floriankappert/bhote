## What and why

<!-- What changes for the user or the code, and why. Link the issue: Fixes #123 -->

## How it was tested

<!-- Commands you ran, what you clicked in the panel, which machines (macOS / Linux, one or several). -->

## Checklist

- [ ] `bash tests/run.sh` passes; new behaviour has a test
- [ ] Runs on macOS with `/bin/bash` (3.2): no bash 4+ features
- [ ] `shellcheck -S warning` adds nothing new for the changed files
- [ ] Docs updated for new keys, settings or commands (README, `docs/`, `bhote help`, the hotkey list)
- [ ] New setting: a row in `SETTINGS`, in `docs/configuration.md`, and `FEATURE_LEVEL` / `step_level` if the wizard offers it
- [ ] Nothing new reaches another machine or a service unless the user switches it on
- [ ] `CHANGELOG.md` updated under *Unreleased* (if users notice the change)
