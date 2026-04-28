# pleme-io/k8s-pause-verify

Assert pause-contract invariants on a paused-first ARC runner pool. Mirrors the helmworks `pleme-arc-runner-pool@0.2.0` chart's `validatePause` helper at consumer time — caught against the live cluster, not at helm render.

## Usage

```yaml
- uses: pleme-io/k8s-pause-verify@v1
  with:
    namespace: pitr-drill-staging
    runner-set-label: pitr-drill-staging
    expected-pod-count: 0
```

## Inputs

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `namespace` | string | yes | — | Runner-pool namespace |
| `runner-set-label` | string | yes | — | `runnerScaleSetName` value |
| `expected-pod-count` | number | no | `0` | Expected runner pods (excluding listener) |
| `kubectl-context` | string | no | — | kubectl context |

## Outputs

| Name | Type | Description |
|---|---|---|
| `pod-count` | number | Observed runner pod count |
| `listener-status` | string | Running / Pending / CrashLoopBackOff / Missing |
| `configmap-paused-flag` | string | `pleme.io/paused` label value or `absent` |

## v1 stability guarantees

Inputs guaranteed within `v1`: `namespace`, `runner-set-label`, `expected-pod-count`.
Outputs guaranteed within `v1`: `pod-count`, `listener-status`.

## Part of the pleme-io action library

This action is one of 11 in [`pleme-io/pleme-actions`](https://github.com/pleme-io/pleme-actions) — discovery hub, version compat matrix, contributing guide, and reusable SDLC workflows shared across the library.
