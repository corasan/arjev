# arjev

Argent + Jev.

`arjev` runs UI verifications on a device. [Argent](https://github.com/software-mansion/argent) drives the
device and returns the screen's accessibility tree. [Jev](https://openrouter.ai) reads that tree and returns
a typed decision with a probability, so a plan asserts intent ("is the Settings root list visible?") instead
of matching strings.

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
arjev ask <udid> "Is the General settings screen shown?"
```

`run` prints one line per step with a mark, the probability, and the elapsed time. It stops at the first
failure and exits 1. `--json` prints the full report instead.

## Environment

`arjev` reads a `.env` file from the working directory, then from the plan file's directory. Variables
already set in the environment win over the file.

| Variable | Purpose |
| --- | --- |
| `OPENROUTER_API_KEY` | Required for `assert`, `choose`, and `ask`. |
| `ARJEV_MODEL` | Jev model id. Defaults to `typesafe/jev-1.13`. |
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
