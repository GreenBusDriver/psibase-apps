# Git

V1 proof of concept of git on Psibase (service + plugin + helper + POC PR UI).

- `service/`: on-chain `git` service (tables, actions, dumb-HTTP `serveSys`)
- `plugin/`: Supervisor plugin for open/close/merge (Max trust — Git app only)
- `ui/`: install strip + PR list/merge
- `../../helper/`: `git-remote-psibase`
- `scripts/pack-install-dist.sh`: packs helper tarballs into `install-dist/`
- `test/git_e2e.py`: end-to-end tests against a local node

See the app `README.md` one level above `packages/` for the SDK developer loop.
