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

`choose` offers the decider every labelled interactive element on screen (buttons, cells, links, fields,
switches) and taps the winner. Add `roles: [Group]` to offer other roles instead, which is how the News plan
picks article cards. Each option carries the element's role, label, and accessibility id. The policy in
`choose_target` in `src/verdict.rs` taps only when confidence is at least 0.8 and the winner leads the
runner-up by 0.2; otherwise the step fails and names the top two candidates.

A `choose` step takes `threshold` (default 0.8) as its confidence floor, the same way `assert` does.

`examples/news-browse.yaml` is the larger example: bring Apple News to the front, tap the Today tab twice to
reach the top of the feed, open the top card, scroll, reveal the navigation bar, go back, scroll the feed,
open a second card. Apple News must have been opened once on the simulator so its welcome screen and location
prompt are gone. Cards clipped by a screen edge or sitting under the collapsed navigation bar are never
offered, because a tap there does nothing.

A failed step's JSON report carries `screen`, the accessibility tree as it was after the failure.

## Jev vs Opus 5

Plan `examples/settings-general.yaml`, five runs each, back to back, on an iPhone 17 Pro simulator running
iOS 26.5. Every run asks three questions. One noul asks whether
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

## Demo video

`demo/record.sh` records one real-time run at the simulator's display rate with `simctl recordVideo`, then
`demo/compose.py` stacks two recordings side by side at 60 fps with a label, a timer, and a DONE badge per
side:

```sh
./demo/record.sh <UDID> jev demo/jev-news.mp4
./demo/record.sh <UDID> claude demo/opus5-news.mp4
python3 demo/compose.py demo/jev-news.mp4 "Jev" demo/opus5-news.mp4 "Claude Opus 5" demo/jev-vs-opus5-news.mp4
```

The record script stops Argent's simulator-server for the device before it starts capturing, because the
simulator has one host-recording slot and the server's frame stream holds it. The composer trims each clip
to its first screen change, so both timers read zero at the first tap rather than at the moment capture
began. `compose.py` needs `ffmpeg`
and Pillow. The warm-start News plan ran in 27.1 s with Jev and 46.9 s with Opus 5.
