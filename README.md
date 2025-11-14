# wasi-warden

Secure, local-first OS automation for LLMs.

`wasi-warden` runs an untrusted “brain” inside a WebAssembly/WASI sandbox and exposes a narrow, auditable set of capabilities from a Rust host: files, processes, browser automation, and (optionally) keyboard/mouse input. It is designed so an LLM can plan and execute complex tasks on a machine without ever getting raw, ambient system access.

Think: **computer-use / RPA / ops runbooks**, but:

* local-first
* capability-based
* WASI-sandboxed
* policy-driven
* LLM-agnostic

For instance, let's say you want to loginto a website using browser automation:

```
cargo run -p hostd -- step \
  --task "Log into https://demo.example/login with creds in secrets/demo-login.json and confirm the dashboard greets the user."
```

---

## Why this exists

Most “LLM controls your computer” stacks are:

* closed-source,
* cloud-run,
* or built as ad-hoc Python glue with weak isolation.

`wasi-warden` targets a different quadrant:

* Organizations that need **local or air-gapped** automation.
* Engineers who want **verifiable guardrails** around powerful agents.
* Tooling ecosystems (MCP, IDEs, CI, ops) that need a **standardized, secure OS agent** instead of bespoke scripts.

If you are fine with a hosted black box, you do not need this project.

---

## Core ideas

1. **Untrusted brain, trusted hands**

   * The planner/agent logic runs as a WASI component (WebAssembly).
   * The Rust host is the only thing allowed to touch the real OS.

2. **Deny-by-default capabilities**

   * No ambient filesystem, network, or env by default.
   * All access goes through explicit, typed capabilities defined in WIT.

3. **Narrow, composable tool surface**

   * Files: scoped to configured workspaces.
   * Processes: allowlisted commands.
   * Browser: controlled via WebDriver / CDP.
   * Input: optional, off by default, behind a hard switch.

4. **LLM-agnostic control**

   * Any LLM that can speak JSON / tool-calls can drive `wasi-warden`.
   * Local models and cloud APIs both work via a single `llm` capability.

5. **Audit, budgets, verification**

   * Every action is logged.
   * Per-task limits on steps, time, and scope.
   * Tasks are “done” only when explicit success predicates are met.

---

## High-level architecture

```text
+-----------------------------+
|        LLM (any)            |
|  - gets tool schema         |
|  - returns actions JSON     |
+--------------+--------------+
               |
         (local HTTP / pipe)
               |
+--------------v--------------+
|        wasi-warden hostd    |   Rust (native)
|  - loads Wasm agent-core    |
|  - enforces policy          |
|  - logs all actions         |
|  - implements capabilities: |
|    fs / proc / browser /    |
|    input / llm              |
+--------------+--------------+
               |
        (WIT imports / WASI)
               |
+--------------v--------------+
|       agent-core.wasm       |   Rust -> WASI component
|  - untrusted planner        |
|  - uses only imported APIs  |
|  - plan/act/observe loop    |
+-----------------------------+
```

The agent never talks to the OS directly. It can only call what `hostd` exposes.

---

## Status

Early design and scaffolding.

Initial goals:

* [ ] Minimal WIT interfaces for `fs`, `proc`, `llm`, `agent`.
* [ ] Rust host using Wasmtime component model.
* [ ] Rust guest built as WASI component calling `llm` + returning action plans.
* [ ] End-to-end: given a task, run a bounded loop of “plan → act via capabilities → observe”.

Expect breaking changes. Do not run in production yet.

---

## Features (planned)

* **Filesystem**

  * Workspace-scoped read/write via directory handles.
  * No arbitrary absolute paths.
* **Process**

  * Spawn and wait for allowlisted commands.
  * Used for tests, formatters, build tools, etc.
* **Browser**

  * Sessions over WebDriver/CDP.
  * Navigate, click, type, scrape DOM/HTML.
* **Input (opt-in, high-risk)**

  * Global keyboard and mouse via OS-specific backends.
  * Fully disabled by default; gated by config and build flag.
* **LLM integration**

  * Single interface for OpenAI-compatible and local models.
  * Supports tool-calling / JSON plans.
* **Policies**

  * TOML/YAML config:

    * allowed paths, binaries, domains,
    * per-task budgets,
    * capability toggles.
* **Observability**

  * Structured logs for all calls.
  * Optional JSON event stream for external auditors/UIS.

---

## Getting started (dev)

Prerequisites:

* Rust (stable)
* `cargo-component` (for WASI components)
* `wasmtime` runtime via Cargo dependencies

Clone:

```bash
git clone https://github.com/FossRust/wasi-warden.git
cd wasi-warden
```

Build host:

```bash
cd hostd
cargo build
```

Build agent-core (WASI component):

```bash
cd ../agent-core
cargo component build --release
```

Copy the sample configuration and edit it with your workspace, process allowlist, and API keys:

```bash
cp ../hostd-sample.toml ../hostd.toml
# edit hostd.toml
```

To enable browser tasks, point the host at a running WebDriver (Chromedriver, Geckodriver, etc.):

```toml
[browser]
webdriver_url = "http://127.0.0.1:9515"
default_profile = "default"
```
 
Each browser action in planner JSON must assign an `alias` for new sessions/elements so follow-up actions (click/type/etc.) can reference them.

See `docs/browser-login-task.md` for a fully worked example and `docs/status.md` for the latest progress snapshot / TODOs before resuming work.

Run a test step (placeholder, subject to change):

```bash
cd ../hostd
cargo run -- step \
  --task "List workspace files" \
  --obs '{}'
```

This should:

1. Load `agent-core.wasm`.
2. Call `agent.step(task, obs)`.
3. Parse returned JSON actions.
4. Execute allowed actions via `fs` capability.
5. Print results.

Concrete commands and config paths will evolve as the API stabilizes.

---

## Security model

Non-negotiable rules:

* Sandbox first:

  * WASI with **no** inherited FS, env, or network.
* Capabilities only:

  * Every sensitive operation is a WIT-defined import.
  * No “run_shell(string)” primitive.
* Configurable scope:

  * Per-install policies define what is reachable.
* Bounded execution:

  * Max steps, max runtime per task, and wasm “fuel” limits.
* Full audit:

  * Every capability call is logged with inputs, outputs, and task ID.

If any of this is compromised, it is a bug.

---

## Roadmap

Short-term:

* Solidify WIT contracts.
* Basic fs + proc + llm loop.
* Simple JSON action format for LLMs.
* Example: “clone repo, run tests, fix lint, summarize” workflow.
* Browser login sample task using `browser.*` selectors (see `docs/browser-login-task.md`).

Medium-term:

* Browser automation.
* Pluggable LLM backends.
* Policy DSL.
* MCP / tool protocol integration.
* Hardened CI examples and threat model documentation.

Long-term:

* Cross-platform input automation (Linux/Windows/macOS).
* Visual runner and policy inspector.
* Library mode for embedding into other tools.

---

## Contributing

Contributions welcome once the initial skeleton lands.

Planned expectations:

* No unsafe OS shortcuts that bypass the model.
* Every new capability must be:

  * WIT-defined,
  * scoped,
  * logged,
  * behind config.

Open issues and discussions will drive the initial design; send PRs that tighten guarantees, not weaken them.
