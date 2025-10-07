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
- **ChainFocusedAI**: Chain-oriented AI that prioritizes building large chains
- **StableAI**: Stability-focused AI that maintains clean board shapes
- **HybridAI**: Adaptive AI that switches strategy based on game phase
- **RandomAI**: Random move generator for testing (baseline)
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

## AI Evaluation and Benchmarking

### Comparing AI Performance

Use the `compare_ai_bots` tool to benchmark and compare different AI implementations:

```sh
# Compare all AI bots with default settings
cargo run --release -p ghoti-simulator --bin compare_ai_bots

# Compare specific AIs with custom parameters
cargo run --release -p ghoti-simulator --bin compare_ai_bots -- \
  --ai-types "BeamSearch,ChainFocused,Stable" \
  -n 50              # Number of games per AI
  -p 4               # Parallel threads
  --max-tumos 100    # Maximum moves per game
  --required-chain-score 20000  # Target score threshold
  --verbose          # Show detailed results
  --output-json results.json    # Save results to JSON
```

### Performance Metrics

The benchmark tool evaluates AIs on multiple metrics:

1. **Score Metrics**:
   - Average score across all games
   - Maximum score achieved
   - Standard deviation (consistency)
   - Median score

2. **Success Rates**:
   - Games exceeding 10,000 points
   - Games exceeding 20,000 points
   - Games exceeding 50,000 points
   - Games exceeding 70,000 points
   - Success rate (% reaching target score)

3. **Efficiency Metrics**:
   - Average moves per game
   - Average thinking time per game

### AI Strategy Profiles

#### BeamSearchAI
- **Strategy**: Comprehensive beam search with depth/width based on available time
- **Strengths**: Best overall performance, high scores, good success rate
- **Typical Performance**: 30,000-50,000 average score, 70%+ success rate at 20k target

#### ChainFocusedAI
- **Strategy**: Maximizes chain potential, 3x weight on chain-related parameters
- **Strengths**: Can build large chains when successful
- **Weaknesses**: Poor board stability, inconsistent results
- **Typical Performance**: 2,000-5,000 average score, 10-20% success rate

#### StableAI
- **Strategy**: Prioritizes clean board shape, minimizes valleys and ridges
- **Strengths**: Consistent board management, low variance
- **Weaknesses**: Too conservative, rarely builds large chains
- **Typical Performance**: 100-500 average score, rarely exceeds 10k

#### HybridAI
- **Strategy**: Adaptive - starts with stability, transitions to chain-focus mid-game
- **Strengths**: Balanced approach, adapts to game state
- **Weaknesses**: Phase transition timing needs tuning
- **Typical Performance**: 500-1,000 average score, improving with tuning

#### RandomAI
- **Strategy**: Completely random valid moves
- **Purpose**: Baseline for comparison, testing robustness
- **Typical Performance**: 1,000-2,000 average score, 0% success rate at 10k

### Tuning AI Parameters

To create and tune a custom AI:

1. **Create new AI in `cpu/src/bot/`**:
   ```rust
   pub struct CustomAI {
       evaluator: Evaluator,
   }
   ```

2. **Adjust evaluator weights**:
   ```rust
   let mut evaluator = Evaluator::default();
   evaluator.chain *= 2;           // Double chain importance
   evaluator.valley *= 3;          // Triple valley penalty
   evaluator.connectivity_3 *= 2;  // Encourage 3-connections
   ```

3. **Key parameters to tune**:
   - **Chain-related**: `chain`, `chain_score`, `potential_main_chain`
   - **Shape-related**: `valley`, `ridge`, `ideal_height_diff`
   - **Connectivity**: `connectivity_2`, `connectivity_3`
   - **Patterns**: GTR-related parameters for opening strategy

4. **Test iterations**:
   ```sh
   # Run multiple iterations to test consistency
   for i in {1..5}; do
     cargo run --release -p ghoti-simulator --bin compare_ai_bots -- \
       --ai-types "CustomAI" -n 20 --seed-start $((i*100))
   done
   ```

### Genetic Algorithm Optimization

For automated parameter tuning, use the optimizer:

```sh
# Optimize evaluator parameters using genetic algorithm
cargo run --release -p ghoti-optimizer --bin ga_tuning_1p -- \
  --simulate-count 20    # Games per configuration
  --population-size 20   # Number of variants to test
  --parallel 4           # Parallel simulations
```

## Repository Links

- This fork: `https://github.com/puyopop/ghoti-fork`
- Pull requests and CI badges reference this fork
- Original project: `https://github.com/morioprog/ghoti`
