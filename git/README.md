# git

Third-party psibase package for on-chain git:

- **Git** — on-chain objects + refs, dumb HTTP, POC PR UI, `install.sh`
- **git-remote-psibase** — native helper for `psibase::` remotes (push/fetch)

Built against the `nix-pkg-dev` SDK (psibase **0.29**). No platform whitelist patches. Short account name `git` requires **root/operator** install. PR open/close/merge are **Max** trust (Git app only; no Trust prompt) — Login via Host is still required to sign.

## Develop

The parent `psibase-apps` git repo must track these files before Nix can see the
flake (`git add git` from `~/repos/psibase-apps`).

```bash
cd ~/repos/psibase-apps/git
nix develop   # or: direnv allow
psiapp chain up 8090

# optional: pack installer tarballs into packages/Git/install-dist/
(cd helper && cargo build --release)
# (cd helper && cargo build --profile dist --target x86_64-unknown-linux-musl)
# packages/Git/scripts/pack-install-dist.sh dev \
#   x86_64=helper/target/x86_64-unknown-linux-musl/dist/git-remote-psibase

cd packages/Git && cargo-psibase package
# Short name `git` needs root (Accounts preapproval).
cargo-psibase install -a http://psibase.localhost:8090/

PATH="$(pwd)/../../helper/target/release:$PATH" \
  python3 test/git_e2e.py   # set PSINODE_URL if not default

# Open http://git.psibase.localhost:8090/ → Login → open/merge PRs
psiapp chain down
```

Flake input defaults to the local `nix-pkg-dev` checkout. Override when the SDK is on a release tag:

```bash
nix flake lock --override-input psibase github:gofractally/psibase/mm/nix-pkg-dev
```

## Permission model

Open / close / merge in the plugin use **Max** trust: only the `git` app may call them; there is no Permissions grant prompt. Users must still log in through Host so the keystore can sign. CLI pushes use `git-remote-psibase` with a local key reference — no Supervisor.

## Spec

Design notes live in the psibase monorepo (`git-on-psibase.md` on `mm/git-on-chain`), not in this tree.
