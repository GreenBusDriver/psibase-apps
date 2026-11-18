# mcp-server

Third-party psibase packages for on-chain MCP:

- **Mcp** — owner UI, agent allowlist, owner auth service (`mcp` / `mcp+1`)
- **XMcp** — node-local MCP endpoint (`x-mcp`, privileged local package)

Built against the `nix-pkg-dev` SDK (psibase **0.29**). No platform whitelist patches: Trust prompts and ordinary Host login/import cover permissions.

## Develop

The parent `psibase-apps` git repo must track these files before Nix can see the
flake (`git add mcp-server` from `~/repos/psibase-apps`).

```bash
cd ~/repos/psibase-apps/mcp-server
nix develop   # or: direnv allow
psiapp chain up 8090

cd packages/Mcp/ui && yarn && yarn build
cd ../../Mcp && cargo-psibase package
cargo-psibase install -a http://psibase.localhost:8090/

cd ../XMcp && cargo-psibase package
psibase install --local -a http://psibase.localhost:8090/ \
  target/wasm32-wasip1/release/packages/XMcp.psi

# Open http://mcp.psibase.localhost:8090/
# Setup: grant Trust when prompted; import agent/enforcer keys in Host first.
psiapp chain down
```

Flake input defaults to the local `nix-pkg-dev` checkout. Override when the SDK is on a release tag:

```bash
nix flake lock --override-input psibase github:gofractally/psibase/mm/nix-pkg-dev
```

## Permission model

MCP does not rely on Transact/Host/AuthSig allowlists. Setup acquires user Trust (Profiles-style). Vault import/login uses Host / `host:accounts/active-app`; agent propose uses ordinary `staged-tx::propose` via `add_action_to_transaction`.
