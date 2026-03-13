# PROJECT KNOWLEDGE BASE

**Generated:** 2026-01-13
**Commit:** b1d6b2b
**Branch:** main

## OVERVIEW

Korean Braille (Jeomja) translation library implementing 2024 Korean Braille Standard. Core in Rust with WASM (Node.js) and PyO3 (Python) bindings.

## STRUCTURE

```
braillify/
├── libs/braillify/        # Core Rust library (see libs/braillify/AGENTS.md)
├── packages/
│   ├── node/              # WASM bindings via wasm-pack
│   └── python/            # PyO3 bindings via maturin
├── apps/landing/          # Next.js 16 docs site (@devup-ui)
├── test_cases/            # CSV rule test cases (61 files)
├── test_case_inputs/      # Input-only test CSVs
├── __tests__/             # Bun JS integration tests
├── py-test/               # Pytest Python tests
└── braillove-case-collector/  # Windows automation tool
```

## WHERE TO LOOK

| Task                      | Location                                   | Notes                                   |
| ------------------------- | ------------------------------------------ | --------------------------------------- |
| Braille encoding logic    | `libs/braillify/src/lib.rs`                | Main `Encoder` struct, `encode()`       |
| Korean character handling | `libs/braillify/src/korean_*.rs`           | Choseong/Jungseong/Jongseong            |
| Rule implementations      | `libs/braillify/src/rule.rs`, `rule_en.rs` | Korean Braille Standard rules           |
| Symbol/shortcut tables    | `libs/braillify/src/*_shortcut.rs`         | PHF static maps                         |
| CLI                       | `libs/braillify/src/cli.rs`                | REPL mode, one-shot mode                |
| Node.js API               | `packages/node/src/lib.rs`                 | `encode`, `translateToUnicode`          |
| Python API                | `packages/python/src/lib.rs`               | Same API + CLI entry                    |
| Landing page              | `apps/landing/src/app/`                    | Next.js App Router                      |
| Test cases                | `test_cases/*.csv`                         | Format: input,internal,expected,unicode |

## CONVENTIONS

### Rust

- Edition 2024, resolver 3
- PHF macros for static lookup tables
- `Result<T, String>` for encoding errors (no custom error type)
- Feature flags: `cli` (default), `wasm`
- Tests inline with `#[cfg(test)]` modules

### TypeScript

- `strict: true`, `moduleResolution: bundler`
- `@/*` path alias to `./src/*`
- ESLint: `eslint-plugin-devup` recommended config
- Bun test with a preload plugin for WASM tests

### Python

- Requires Python >= 3.13 (workspace), >= 3.8 (package)
- `uv` for workspace management
- `maturin` for building wheels
- CLI entry: `braillify = "braillify:cli"`

## ANTI-PATTERNS (THIS PROJECT)

- **Never suppress encoding errors** - propagate `Result<T, String>`
- **Never modify CSV test files without running full test suite** - `rule_map.json` must match
- **Avoid `as any` or `@ts-ignore`** in TypeScript

## COMMANDS

```bash
# Install dependencies (runs uv sync, wasm-pack install, maturin install)
bun install

# Build all packages
bun run build

# Run all tests (Rust coverage + Bun test + Pytest)
bun run test

# Build landing site (requires test_status.json from test run)
bun run build:landing

# Dev server for landing
bun -F landing dev

# Lint
bun run lint
bun run lint:fix
```

## NOTES

- **Test output**: `cargo test test_by_testcase` generates `test_status.json` for landing page
- **WASM build**: `wasm-pack build --target bundler` in `packages/node`
- **Python wheel**: `maturin build --release` in `packages/python`
- **No CI workflows checked in** - build/test orchestrated via root `package.json`
- **Korean comments** in Rust code reference specific rule numbers (e.g., "제14항")
