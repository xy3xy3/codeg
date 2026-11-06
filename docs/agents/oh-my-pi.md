# Oh My Pi

Codeg connects directly to [Oh My Pi](https://github.com/can1357/oh-my-pi) with `omp acp` over stdio. It is a separate native agent from Pi: no `pi-acp` bridge or custom-agent registration is needed.

## Setup

Codeg can install a pinned standalone OMP binary from Agent Settings on supported platforms. This bundles its runtime and does not require Node.js or Bun.

Alternatively, install and configure Oh My Pi on the machine running Codeg's backend, not just the machine running your browser. Follow the [upstream installation instructions](https://github.com/can1357/oh-my-pi#install) for your platform. Manual package installations require Bun 1.4.4 or newer; an ordinary Node.js/npm installation is not an equivalent runtime.

Check `omp --version`, then start `omp` to configure providers and authentication using the native CLI. Restart Codeg if necessary so the backend inherits the updated PATH and environment. Select **Oh My Pi** in Agent Settings and in your workspace. Codeg reuses OMP's native configuration rather than copying credentials into generic OpenAI settings.

## Session configuration

Model, thinking-level and mode choices are advertised by OMP through ACP `configOptions`. Codeg sends changes through `session/set_config_option`, retaining the advertised option IDs and values. It does not hardcode model names or use the legacy model setter. Available choices and input capabilities depend on the installed OMP version and selected provider/model.

## History and compatibility

Native sessions use `~/.omp/agent/sessions` by default. Codeg imports native OMP history separately from Pi history, including conversations started through ACP in Codeg. Existing `custom:omp`, `custom:oh-my-pi` and `custom:oh_my_pi` registrations remain separate and are not renamed, deleted or silently migrated to the built-in agent.

Provider credentials and native configuration remain owned by Oh My Pi. Consult the installed version's documentation before moving its data or changing its environment. Configure MCP servers through Codeg's Oh My Pi assignment to send session overlays without overwriting native credential files.

## Skills and data directories

Codeg manages `<name>/SKILL.md` bundles in `~/.omp/agent/skills` and workspace `.omp/skills`. Unlike Pi, OMP does not load standalone `<name>.md` skill files. Include a description in each skill's frontmatter.

OMP can select another agent directory with `PI_CODING_AGENT_DIR`, another configuration root with `PI_CONFIG_DIR`, or a profile with `OMP_PROFILE` (preferred over `PI_PROFILE`). Named profiles use `~/.omp/profiles/<name>/agent` by default. Codeg hides global skill management for an OMP entry with these per-agent environment overrides, because those overrides reach the CLI rather than Codeg's global skill store. `XDG_DATA_HOME` can relocate session storage but does not relocate the skill directory. Keep the backend and CLI environment consistent for native history discovery.
