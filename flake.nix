{
  description = "pleme-io/k8s-pause-verify — assert pause-contract invariants on an ARC runner pool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    substrate = {
      url = "github:pleme-io/substrate";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ { self, nixpkgs, crate2nix, flake-utils, substrate, ... }:
    (import "${substrate}/lib/rust-action-release-flake.nix" {
      inherit nixpkgs crate2nix flake-utils;
    }) {
      toolName = "k8s-pause-verify";
      src = self;
      repo = "pleme-io/k8s-pause-verify";
      action = {
        description = "Assert pause-contract invariants on a paused-first ARC runner pool: pod-count by label selector is 0 (only the listener), AutoscalingRunnerSet has max=min=0, and the pause-state ConfigMap is present. Mirrors the pleme-arc-runner-pool 0.2.0 chart's validatePause helper at consumer time.";
        inputs = [
          { name = "namespace"; description = "Namespace where the runner pool's resources live"; required = true; }
          { name = "runner-set-label"; description = "Runner scale set name (matches gha-runner-scale-set runnerScaleSetName)"; required = true; }
          { name = "expected-pod-count"; description = "Expected runner-pod count excluding listener; for paused: 0"; default = "0"; }
          { name = "kubectl-context"; description = "kubectl context; defaults to current"; }
        ];
        outputs = [
          { name = "pod-count"; description = "Observed runner pod count (excluding listener)"; }
          { name = "listener-status"; description = "Listener pod state — Running / Pending / CrashLoopBackOff / Missing"; }
          { name = "configmap-paused-flag"; description = "Value of pleme.io/paused label on the pause-state ConfigMap"; }
        ];
      };
    };
}
