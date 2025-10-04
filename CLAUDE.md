# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ghoti is a Puyo Puyo AI bot implementation. This is a fork of [morioprog/ghoti](https://github.com/morioprog/ghoti). The project consists of a Rust-based AI engine with a Next.js web frontend for visualizing game replays.

## Build System

### Rust (Workspace)

This is a Rust workspace with 5 crates:
- **puyoai**: Core library wrapping `puyoai-core` with ES (Esports) extensions
- **cpu**: AI implementations (BeamSearchAI, RandomAI) and evaluation logic
- **simulator**: Game simulation and CLI binaries
- **logger**: Logging utilities
- **optimizer**: Parameter optimization for the evaluator

**Important**: This project requires Rust nightly (`nightly-2023-10-01` specified in `rust-toolchain`).

### Next.js Frontend

Located in `nextjs/` directory. Visualizes game replays (kifus) from the `kifus/` directory.

## Common Commands

### Running Simulations

```sh
# Single-player mode (Toko-Puyo)
cargo run --release -p ghoti-simulator --bin cli_1p

# Two-player battle
cargo run --release -p ghoti-simulator --bin cli_2p

# Replay viewer (WIP)
cargo run --release -p ghoti-simulator --bin replay_kifus
```

### Building and Testing

```sh
# Build all workspace members
cargo build --verbose

# Run tests
cargo test --verbose

# Release build (uses LTO optimization)
cargo build --release
```

### Next.js Development

```sh
cd nextjs
yarn dev          # Development server
yarn build        # Production build
yarn export       # Static export
yarn serve        # Export and serve locally
```

## Architecture

### AI System

The AI system is built around the `AI` trait defined in `cpu/src/bot/ai.rs`:

```rust
pub trait AI {
    fn think(
        &self,
        player_state_1p: PlayerState,
        player_state_2p: Option<PlayerState>,
        think_frame: Option<usize>,
    ) -> AIDecision;
}
```

**AI Implementations**:
- **BeamSearchAI**: Main AI using beam search with evaluation functions
- **RandomAI**: Random move generator for testing
- **OpeningMatcher**: Pattern-matching for opening moves

### Evaluator System

The evaluator (`cpu/src/evaluator/evaluator.rs`) scores board states using 50+ weighted parameters including:
- Board shape metrics (valley, ridge, height differences)
- Connectivity patterns (2-connected, 3-connected groups)
- Chain potential (main chain, sub chain predictions)
- Pattern matching (GTR, NewGTR, Submarine formations)

Parameters are tunable via the optimizer crate for genetic algorithm-based optimization.

### Simulation Flow

1. **simulate_1p.rs / simulate_2p.rs**: Core simulation loops
2. AI thinks and returns decisions
3. Decisions are applied to the game state
4. Results are serialized to JSON in `kifus/` directory
5. Next.js frontend reads these JSON files for visualization

### ES Field Extensions

The `puyoai` crate extends `puyoai-core` with "ES" (Esports) variants:
- `ESBitField`, `ESPlainField`, `ESCoreField`: Field implementations optimized for competitive play
- `ESFrame`: Frame-accurate game state tracking

### Known Issues

**Rust Syntax Compatibility**:
- The `puyoai` crate uses `#![feature(let_chains)]` which is unstable
- Some files may have unstable `let` chain syntax that needs to be rewritten as nested if-let statements for compatibility
- Example fix in `simulator/src/simulate_1p.rs:66`: Changed `if let Some(x) = y && condition` to nested if-let

## Repository Links

- This fork: `https://github.com/puyopop/ghoti-fork`
- Pull requests and CI badges reference this fork
- Original project: `https://github.com/morioprog/ghoti`
