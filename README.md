# arjev

Argent + Jev.

`arjev` runs UI verifications on a device. [Argent](https://github.com/software-mansion/argent) drives the
device and returns the screen's accessibility tree. A decision model reads that tree and returns a typed
decision with a probability, so a plan asserts intent ("is the Settings root list visible?") instead of
matching strings.

Two deciders answer the same questions. `jev` calls the TypeSafe decisions endpoint through OpenRouter.
`claude` runs the local Claude Code CLI (`claude -p`) with a JSON schema built from the questions, so it
uses your Claude login and needs no API key.

## Install

```sh
cargo build --release
```

The binary is `target/release/arjev`.

## Run

```sh
arjev devices                                  # devices Argent can reach
arjev tools                                    # tool names the tool-server exposes
arjev run examples/settings-general.yaml       # run a plan
arjev run examples/settings-general.yaml --json
arjev run examples/settings-general.yaml --decider claude --model claude-opus-5
arjev ask <udid> "Is the General settings screen shown?"
arjev bench examples/settings-general.yaml --runs 5 --decider claude
```

`run` prints one line per step with a mark, the probability, and the elapsed time. It stops at the first
failure and exits 1. `--json` prints the full report instead, with `decide_ms`, `input_tokens`,
`output_tokens`, and `cost` on every step that asked a question.

`bench` runs a plan N times on one decider, writes each raw report to
`bench/<model-slug>/<timestamp>-<n>.json`, and prints one markdown summary row.

`run`, `ask`, and `bench` take `--decider jev|claude` and `--model <id>`. Both flags beat the environment.

## Environment

`arjev` reads a `.env` file from the working directory, then from the plan file's directory. Variables
already set in the environment win over the file.

| Variable | Purpose |
| --- | --- |
| `OPENROUTER_API_KEY` | Required by the `jev` decider. |
| `ARJEV_DECIDER` | `jev` or `claude`. Defaults to `jev`. |
| `ARJEV_MODEL` | Model id for the active decider. Defaults to `typesafe/jev-1.13` for `jev` and `claude-opus-5` for `claude`. |
| `ARGENT_URL`, `ARGENT_TOKEN` | Skip tool-server discovery and use this endpoint. |

Without `ARGENT_URL`, `arjev` reads every `~/.argent/tool-server-*.json`, sorts them by version descending,
and takes the first one that answers.

## Plan format

```yaml
name: settings-general
device: first           # or { udid: "..." } or { name: "iPhone 17" }
prerequisite: setup.yaml  # optional; its steps run first

steps:
  - kind: act           # call any Argent tool; udid is filled in when absent
    tool: launch-app
    args:
      bundleId: com.apple.Preferences

  - kind: assert        # Jev noul over the screen; passes at threshold or above
    name: root-list
    question: Is the iOS Settings root list visible?
    threshold: 0.8      # optional, defaults to 0.8

  - kind: choose        # Jev picks one on-screen element, then arjev acts on it
    name: general-entry
    question: Which element opens the General settings screen?
    then: tap
```

`choose` offers Jev every labelled interactive element on screen and acts on the winner. The confidence
policy that turns Jev's answer into a tap lives in `choose_target` in `src/verdict.rs`, which is deliberately
left unimplemented for the owner to write.

## Jev vs Opus 5

Measured on 2026-09-18. Plan `examples/settings-general.yaml`, five runs each, back to back, iOS 26.5
simulator `F1036AD5-E359-40CB-8011-67252C1A93BE`. Every run asks three questions. One noul asks whether
the Settings root list is on screen, one choice asks which labelled interactive element opens General, and
one noul asks whether the General screen is on screen. Jev went through OpenRouter. Opus 5 went through the
local `claude -p` CLI with a minimal system prompt, no tools, and no MCP servers.

| model | runs | passes | mean decide ms | p50 | p95 | mean total ms | input tokens | output tokens | cost USD |
|---|---|---|---|---|---|---|---|---|---|
| typesafe/jev-1.13 | 5 | 5 | 268 | 250 | 297 | 5584 | 26843 | 885 | 0.0011 |
| claude-opus-5 (claude CLI) | 5 | 5 | 4053 | 3976 | 4752 | 17025 | 195166 | 3086 | 1.2599 |

`decide ms` is wall time inside one decision, including process start for the CLI. `total ms` is one full
plan run, including the app restart and the two waits. Opus input tokens include the Claude Code base
prompt that the CLI sends on every call (about 11k tokens per decision). The cost column for Opus is the
list price the CLI reports; a Claude subscription does not bill it per call.

Raw reports are under `bench/typesafe-jev-1-13/` and `bench/claude-opus-5/`.
