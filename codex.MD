# Codex Runtime Notes

## Host Escape for Sandboxed Commands

- Prefer `flatpak-spawn --host` for commands that must run on the host from a sandboxed environment.
- Use this for tools such as `docker`, `docker compose`, and host-level `bash` invocations when needed.
- Keep command behavior identical to host execution, only changing the launcher prefix.

## Common Host Command Examples

- Docker direct: `flatpak-spawn --host docker ps`
- Docker compose (plugin): `flatpak-spawn --host docker compose up -d`
- Docker compose (legacy binary): `flatpak-spawn --host docker-compose up -d`
- Host shell command chain: `flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && docker compose ps'`

## Launch Script Chain Example

- Run launcher from repo root: `flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && ./scripts/launch.sh up --hardware --zenoh'`
- Stop stack: `flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && ./scripts/launch.sh down'`
- Follow logs: `flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && ./scripts/launch.sh logs'`
- Non-interactive port cleanup mode: `flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && CERES_NONINTERACTIVE=1 CERES_AUTO_KILL_PORTS=1 ./scripts/launch.sh up --mock'`

## Script Shebang and Execution

- Use shebang: `#!/usr/bin/env bash`
- Keep strict mode in scripts: `set -euo pipefail`
- Ensure script is executable: `flatpak-spawn --host chmod +x ./scripts/launch.sh`

## Playwright via Host (Post-Launch UI Tests)

- Discover Playwright on host:
`flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && command -v ./node_modules/.bin/playwright || command -v playwright || command -v npx'`
- Preferred test command (project-local binary):
`flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && ./node_modules/.bin/playwright test'`
- Fallback test command (npx):
`flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && npx playwright test'`
- Full chain (start containers, run UI tests, then tear down):
`flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && ./scripts/launch.sh up --mock && ./node_modules/.bin/playwright test && ./scripts/launch.sh down'`

## Cargo / Crates Access Rule

- Always prefer host-run Cargo commands through `flatpak-spawn --host` when dependency resolution, crate downloads, or index access are needed.
- Standard pattern:
`flatpak-spawn --host /usr/bin/env bash -lc 'cd "/path/to/repo" && cargo check'`
- Crates reachability check pattern:
`flatpak-spawn --host /usr/bin/env bash -lc 'curl -I https://index.crates.io/config.json | head -n 5 && cargo search async-opcua --limit 1'`
- Use this same host pattern for `cargo build`, `cargo test`, `cargo fetch`, and `cargo update`.
