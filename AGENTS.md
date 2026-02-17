# AGENTS.md

## Coding Style & Naming Conventions

- **Functions**: `snake_case`
- **Variables**: `snake_case`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Structs/Types**: `PascalCase`
- **Enums**: `PascalCase` for enum name, `PascalCase` for variants (e.g., `ConfigError`, `ParseError`)
- **Type Aliases**: `PascalCase`
- **Modules**: `snake_case` (e.g., `error`, `config`, `api`)
- **Crate Names**: `snake_case` with hyphens in Cargo.toml (e.g., `my-app`)

### Code Organization

- Group imports in three sections separated by blank lines: stdlib, external crates, internal modules
- Use `pub` visibility only when necessary; keep implementation details private
- Organize modules by feature or domain (e.g., `auth`, `api`, `db`, `config`)

## Error Handling

- Use `anyhow` for application-level error handling
- Import `anyhow::Result` and use it as the return type: `use anyhow::Result;`
- Use `.context("description")` to add context to errors
- Use `anyhow::bail!("message")` for early returns with custom errors
- For domain logic, use `thiserror` for typed errors, then convert to `anyhow` at boundaries

```rust
use anyhow::{Context, Result};

fn load_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read configuration file")?;
    parse_config(&content).context("Failed to parse configuration")
}
```

### Domain Errors with thiserror

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("configuration file not found at {0}")]
    NotFound(String),
    #[error("invalid configuration: {0}")]
    Invalid(String),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}
```

### Result Unwrapping

- Always prefer the `?` operator for error propagation
- Use `.expect("descriptive message")` only in tests
- Avoid `.unwrap()` in production code

### Type Aliases

- Always create a `Result<T>` type alias for custom error types: `pub type Result<T> = std::result::Result<T, YourError>;`
- Use type aliases for frequently-used complex types (e.g., `Arc<Mutex<HashMap<String, Value>>>`)
- Avoid over-aliasing simple types

```rust
// Always do this for Result
pub type Result<T> = std::result::Result<T, ConfigError>;

// Good for complex types
pub type ConnectionPool = Arc<Mutex<Vec<Connection>>>;

// Don't alias simple types
// Bad: pub type UserId = u64;
// Good: just use u64 directly
```

Complex types repeated in 2+ locations are refactoring candidates. Suggest a type alias to reduce duplication, but do not implement without asking first.

## Dependencies

- Always pin dependencies to the latest specific version (e.g., `chrono = "0.4.43"`)
- Avoid version ranges like `"0.4"` or `"^0.4"` - be explicit about the exact version
- Check the latest version on [crates.io](https://crates.io/) before adding a dependency

This is a single-crate project. Define dependencies directly under `[dependencies]` in `Cargo.toml`.

## String Parameters

- Use `&str` for function parameters that only need to read the string
- Use `String` only when the function needs to own or modify the string

## Trait Bounds

- Use inline syntax for simple bounds: `fn foo<T: Trait>(item: T)`
- Use `where` clause for complex bounds

```rust
// Simple
fn process<T: Serialize>(item: T) -> Result<String> {
    serde_json::to_string(&item)
}

// Complex
fn complex<T, U>(item: T, other: U) -> Result<()>
where
    T: Serialize + DeserializeOwned + Clone,
    U: Display + Debug,
{
    // Implementation
}
```

## Logging and Tracing

- Use `tracing` for all logging (avoid `println!` in production code)
- Use appropriate log levels: `info!`, `warn!`, `error!`, `debug!`, `trace!`
- Use `#[instrument]` for automatic span creation
- Configure log level via `RUST_LOG` environment variable

## Dependency Injection

Use constructor injection with `Arc` for shared state:

```rust
use std::sync::Arc;

pub struct Service {
    db: Arc<Database>,
    cache: Arc<Cache>,
}

impl Service {
    pub fn new(db: Arc<Database>, cache: Arc<Cache>) -> Self {
        Self { db, cache }
    }
}
```

## Documentation & Comments

- English is used for all code comments and documentation
- Code identifiers remain in English
- Code comments (`///` and `//!` for rustdoc, `//` for inline) must be in English
- No emojis anywhere

### Rustdoc Guidelines

Use `//!` for module/crate docs and `///` for item docs (functions, structs, enums).

### Module-Level Docs (`//!`)

- Every `.rs` file must have module-level documentation at the top
- 3-5 lines: first line describes what the module does, following lines add context

### Item-Level Docs (`///`)

- All items (public and private) must be documented
- Maximum 2-3 lines per item; keep it brief

## Commit Guidelines

- Use **Conventional Commits**: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`
- Always include a commit body describing **what the change does and why it exists**, not how it is implemented
- Do not reference function/file names or hard-coded values; keep the body implementation-agnostic and future-proof
- The commit message should be a single continuous block with title and body together, separated by a blank line (do not separate them into different sections)

## Development Commands

### Building

```bash
cargo build                            # Build (debug)
cargo build --release                  # Build optimized binary
cargo build -v                         # Build with verbose output
cargo check                            # Type-check without producing binaries (faster)
```

### Code Formatting

```bash
cargo fmt --all                        # Format code
cargo fmt --all --check                # Check formatting without modifying files
```

### Linting

```bash
cargo clippy --all-targets -- -D warnings  # Run clippy, fail on warnings
```

### Security Audit

```bash
cargo audit                                        # Check for known vulnerabilities
cargo deny check advisories licenses bans sources  # Full dependency policy check
```
