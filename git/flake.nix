{
  description = "Psibase package workspace (SDK consumer)";

  inputs = {
    # Local checkout of the 0.29 SDK train (nix-pkg-dev) while the SDK branch
    # hasn't landed on main / been tagged yet.
    psibase.url = "path:/home/mike/repos/fractally/psibase-wktrees/nix-pkg-dev";
  };

  outputs =
    { psibase, ... }:
    {
      # Package-dev shell as default (not the contributor monorepo shell).
      devShells = builtins.mapAttrs (
        system: shells:
        let
          sdk =
            shells.sdk or (throw "psibase: no devShells.${system}.sdk (need x86_64-linux SDK flake)");
        in
        {
          default = sdk;
          inherit sdk;
        }
      ) psibase.devShells;
    };
}
