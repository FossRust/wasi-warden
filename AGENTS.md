# Repository Guidelines

`wasi-warden` tracks the design, policy, and soon the Rust host plus WASI guest that form the secure automation stack. Treat every change as part of defense-in-depth: keep scope tight, document assumptions, and prefer auditable code paths.

## Project Structure & Module Organization
Root holds `README.md`, `docs/`, and will grow three code directories referenced throughout the docs: `hostd/` (privileged Rust daemon), `agent-core/` (WASI component), and `wit/` (shared interfaces). Keep host adapters in `hostd/src/caps_*`, guest planning logic in `agent-core/src`, and stash experiments or threat-model notes under `docs/research/` instead of alongside production code.

## Build, Test, and Development Commands
- `cargo build -p hostd` — compile the Rust host; run inside `hostd/`.
- `cargo component build -p agent-core --release` — produce the WASI component with the latest WIT bindings.
- `cargo fmt && cargo clippy -D warnings` — enforce formatting and lints before every push.
- `cargo run -p hostd -- step --task "<task>" --obs '{}'` — execute a local dry run against the compiled guest.

## Coding Style & Naming Conventions
Rust code follows default `rustfmt` (4-space indent, trailing commas). Modules stay `snake_case`; exported types use `UpperCamelCase`. WIT packages live under `osagent:*` with verb-style functions (`open_workspace`, `list_dir`). Keep capability structs small, reuse `CapabilityImpl` naming for host adapters, and reserve comments for security rationale.

## Testing Guidelines
Add unit tests alongside modules using Rust’s `mod tests`. Integration coverage belongs in `hostd/tests/` and should exercise real capability flows via allowlisted commands (e.g., `cargo test -p hostd -- --nocapture`). Guest components rely on `cargo component test` plus fixture observations. Name tests after behavior (`handles_missing_policy`, `denies_unlisted_proc`) and pair every bug fix with a regression.

## Commit & Pull Request Guidelines
History shows short, imperative commit titles (“add getting started guide”). Keep subject ≤50 chars, follow with a blank line, then detail the why/impact. Pull requests must cite related issues, describe policy implications, and include screenshots or logs when touching CLI UX. Call out any new capabilities, config knobs, or security-sensitive default changes.

## Security & Configuration Tips
Default to deny-by-default policies: keep new capabilities off until the config documents scope, allowlists, and budget knobs. Store sample policies under `docs/` or `wit/examples/`, never in user directories. Log every capability invocation with task IDs, and ensure new code paths return structured errors so auditors—and downstream agents—can reason about failures.
