use std::{cmp::min, sync::mpsc, thread, time::Instant, vec::Vec};

use puyoai::{
    color::{Color, PuyoColor},
    column_puyo_list::ColumnPuyoList,
    decision::Decision,
    field::CoreField,
    kumipuyo::{kumipuyo_seq::generate_random_puyocolor_sequence, Kumipuyo},
    plan::Plan,
    rensa_detector::{detector::detect_by_drop, PurposeForFindingRensa},
};

use crate::bot::*;

/// Takapt AI - Beam search based AI inspired by takapt's implementation
pub struct TakaptAI {
    beam_width: usize,
    beam_depth: usize,
    parallel: usize,
}

impl TakaptAI {
    pub fn new_customize(beam_width: usize, beam_depth: usize, parallel: usize) -> Self {
        TakaptAI {
            beam_width,
            beam_depth,
            parallel,
        }
    }
}

impl AI for TakaptAI {
    fn new() -> Self {
        TakaptAI {
            beam_width: 400,
            beam_depth: 20,
            parallel: 5, // Run 5 simulations with different random sequences (balance speed vs accuracy)
        }
    }

    fn name(&self) -> &'static str {
        "TakaptAI"
    }

    fn think(
        &self,
        player_state_1p: PlayerState,
        _player_state_2p: Option<PlayerState>,
        _think_frame: Option<usize>,
    ) -> AIDecision {
        let start = Instant::now();

        // If we have enough visible tumos, don't need Monte Carlo
        let parallel = if player_state_1p.seq.len() < self.beam_depth {
            self.parallel
        } else {
            1
        };

        // Run multiple simulations with different random sequences
        let (tx, rx): (mpsc::Sender<SimulationResult>, mpsc::Receiver<SimulationResult>) =
            mpsc::channel();

        for simulation_id in 0..parallel {
            let depth = self.beam_depth;
            let width = self.beam_width;
            let tx_c = tx.clone();
            let player_state_c = player_state_1p.clone();

            thread::spawn(move || {
                tx_c.send(run_single_simulation(
                    &player_state_c,
                    depth,
                    width,
                    simulation_id,
                ))
                .ok();
            });
        }

        // Aggregate results: collect chains for each first decision (like original takapt)
        let mut chains: [[Vec<usize>; 4]; 7] = Default::default(); // [x][rotation] -> vector of chain counts
        let mut result_map: std::collections::HashMap<(usize, usize), SimulationResult> =
            std::collections::HashMap::new();

        for _ in 0..parallel {
            if let Ok(result) = rx.recv() {
                if !result.decisions.is_empty() {
                    let first_dec = &result.decisions[0];
                    let x = first_dec.axis_x();
                    let rot = first_dec.rot();

                    // Collect chain count for this decision
                    chains[x][rot].push(result.max_chains);

                    // Keep one result for this decision
                    result_map.insert((x, rot), result);
                }
            }
        }

        // Select the decision with highest total chains (like original takapt)
        let mut best_sum_chains = 0;
        let mut best_decision = Decision::new(3, 0);

        for x in 1..=6 {
            for rot in 0..4 {
                if !chains[x][rot].is_empty() {
                    let sum_chains: usize = chains[x][rot].iter().sum();
                    if sum_chains > best_sum_chains {
                        best_sum_chains = sum_chains;
                        best_decision = Decision::new(x, rot);
                    }
                }
            }
        }

        // Find the result for this decision
        if let Some(result) = result_map.get(&(best_decision.axis_x(), best_decision.rot())) {
            let avg_chains = if !chains[best_decision.axis_x()][best_decision.rot()].is_empty() {
                best_sum_chains as f64 / chains[best_decision.axis_x()][best_decision.rot()].len() as f64
            } else {
                0.0
            };

            return AIDecision::new(
                result.decisions.clone(),
                format!("{} (avg_chains: {:.1})", result.log_output, avg_chains),
                start.elapsed(),
            );
        }

        // Fallback
        AIDecision::new(
            vec![Decision::new(3, 0)],
            "no valid move".to_string(),
            start.elapsed(),
        )
    }
}

#[derive(Clone)]
struct SimulationResult {
    decisions: Vec<Decision>,
    log_output: String,
    max_chains: usize,
}

fn run_single_simulation(
    player_state_1p: &PlayerState,
    beam_depth: usize,
    beam_width: usize,
    simulation_id: usize,
) -> SimulationResult {
    let cf = &player_state_1p.field;
    let seq = &player_state_1p.seq;

    // Calculate good_chains threshold based on field complexity
    let good_chains = calculate_good_chains(cf);

    // Extend sequence with random puyos if needed (using simulation_id for seed variation)
    let visible_tumos = seq.len();
    let seq: Vec<Kumipuyo> = seq
        .iter()
        .cloned()
        .chain(generate_random_puyocolor_sequence(
            if beam_depth > visible_tumos {
                beam_depth - visible_tumos
            } else {
                0
            },
        ))
        .collect();

    let mut states: Vec<State> = vec![State::from_field(cf)];
    let mut fired_states: Vec<State> = Vec::new();
    let mut max_chains = 0;
    let mut first_decision_for_max_chains: Option<Decision> = None;

    // Beam search
    for depth in 0..beam_depth.min(seq.len()) {
        let mut next_states: Vec<State> = Vec::new();
        let mut next_fired: Vec<State> = Vec::new();

        for state in &states {
            // Generate all possible placements
            let seq_vec = vec![seq[depth].clone()];
            Plan::iterate_available_plans(&state.field, &seq_vec, 1, &mut |plan: &Plan| {
                let mut decisions = state.decisions.clone();
                decisions.push(plan.first_decision().clone());

                let score = evaluate_state(plan, &state.field);
                let chain = plan.chain();
                let plan_score = plan.score();

                let new_state = State {
                    field: plan.field().clone(),
                    decisions,
                    score,
                    chain,
                    plan_score,
                    is_fired: chain > 0,
                };

                // Track fired states separately
                if chain > 0 {
                    // Update max chains tracking
                    if chain > max_chains {
                        max_chains = chain;
                        first_decision_for_max_chains = new_state.first_decision().cloned();
                    }
                    next_fired.push(new_state.clone());
                }

                next_states.push(new_state);
            });
        }

        if next_states.is_empty() {
            break;
        }

        // Sort by score and keep top beam_width states
        next_states.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        if next_states.len() > beam_width {
            next_states.truncate(beam_width);
        }

        // Merge fired states
        fired_states.extend(next_fired);

        // Early termination: if we found a good chain
        if max_chains >= good_chains {
            if let Some(ref first_decision) = first_decision_for_max_chains {
                // Check if the best state has the same first decision
                if let Some(best_first) = next_states.first().and_then(|s| s.first_decision()) {
                    if best_first == first_decision {
                        break;
                    }
                }
            }
        }

        states = next_states;
    }

    // Decision selection: prioritize max chains from fired states
    if !fired_states.is_empty() {
        // Sort fired states by chain count (desc), then score (desc)
        fired_states.sort_by(|a, b| {
            b.chain.cmp(&a.chain)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
        });

        if let Some(best_fired) = fired_states.first() {
            if !best_fired.decisions.is_empty() {
                return SimulationResult {
                    decisions: best_fired.decisions.clone(),
                    log_output: format!(
                        "FIRE: chain: {}, score: {}, eval: {:.0}",
                        best_fired.chain, best_fired.plan_score, best_fired.score
                    ),
                    max_chains: best_fired.chain,
                };
            }
        }
    }

    // No fired states, select best non-fired state
    if let Some(best_state) = states.first() {
        if !best_state.decisions.is_empty() {
            return SimulationResult {
                decisions: best_state.decisions.clone(),
                log_output: format!("BUILD: eval: {:.0}", best_state.score),
                max_chains: 0,
            };
        }
    }

    // Fallback
    SimulationResult {
        decisions: vec![Decision::new(3, 0)],
        log_output: "no valid move".to_string(),
        max_chains: 0,
    }
}

#[derive(Clone)]
struct State {
    field: CoreField,
    decisions: Vec<Decision>,
    score: f64,
    chain: usize,
    plan_score: usize,
    is_fired: bool, // Whether this state fired a chain
}

impl State {
    fn from_field(field: &CoreField) -> Self {
        State {
            field: field.clone(),
            decisions: vec![],
            score: 0.0,
            chain: 0,
            plan_score: 0,
            is_fired: false,
        }
    }

    fn first_decision(&self) -> Option<&Decision> {
        self.decisions.first()
    }
}

/// Evaluate a game state based on takapt's original evaluation function
fn evaluate_state(plan: &Plan, _base_field: &CoreField) -> f64 {
    let field = plan.field();
    let actual_chain = plan.chain();
    let chain_score = plan.score();

    let mut score = 0.0;

    // If a chain was fired, use the chain score directly as the primary score
    if actual_chain > 0 {
        score = chain_score as f64;
    }

    // Use RensaDetector to find potential chains (complementing with 2-13 puyos)
    let (max_chains, highest_ignition_y) = detect_potential_chains(field);

    // 1. Chain bonus: chains >= 2 get additional bonus: max_chains * 1000
    if max_chains >= 2 {
        score += (max_chains as f64) * 1000.0;
    }

    // 2. Calculate average height
    let heights = field.height_array();
    let mut ave_height = 0.0;
    for x in 1..=6 {
        ave_height += heights[x] as f64;
    }
    ave_height /= 6.0;

    // 3. Height uniformity scoring with ideal pattern
    // good_diff[] = { 0, 2, 0, -2, -2, 0, 2, 0 }
    const GOOD_DIFF: [i32; 8] = [0, 2, 0, -2, -2, 0, 2, 0];
    let mut u_score = 0.0;
    for x in 1..=6 {
        let height_diff = (heights[x] as f64 - ave_height) - GOOD_DIFF[x] as f64;
        u_score -= height_diff.abs();
    }
    score += 60.0 * u_score;

    // 4. Ignition height bonus: score += 10 * (highest_ignition_y - ave_height)
    // highest_ignition_y comes from RensaDetector
    score += 10.0 * (highest_ignition_y - ave_height);

    // 5. Penalty for columns reaching row 13 (top row)
    // coef[] = { 0, 1, 3, 0, 3, 2, 1, 0 }
    const COEF: [f64; 8] = [0.0, 1.0, 3.0, 0.0, 3.0, 2.0, 1.0, 0.0];
    for x in 1..=6 {
        if !field.is_empty(x, 13) {
            score -= COEF[x];
        }
    }

    score
}

/// Detect potential chains using RensaDetector-like algorithm
/// Returns (max_chains, highest_ignition_y)
fn detect_potential_chains(field: &CoreField) -> (usize, f64) {
    let mut max_chains = 0;
    let mut highest_ignition_y = 0;

    // Use detect_by_drop to find potential chains by complementing with 2-13 puyos
    let prohibits = [false; 8]; // No column restrictions

    let callback = |mut complemented_field: CoreField, cpl: &ColumnPuyoList| {
        // Find the ignition point (highest column where puyos were added)
        let mut ignition_y = 0;
        for x in 1..=6 {
            if cpl.size_on(x) > 0 {
                let h = complemented_field.height(x);
                if h > ignition_y {
                    ignition_y = h;
                }
            }
        }

        // Simulate the chain
        let rensa_result = complemented_field.simulate();

        // Update max_chains and highest_ignition_y
        if (rensa_result.chain, ignition_y) > (max_chains, highest_ignition_y) {
            max_chains = rensa_result.chain;
            highest_ignition_y = ignition_y;
        }
    };

    // Detect potential chains by complementing with 2-13 puyos
    detect_by_drop(
        field,
        &prohibits,
        PurposeForFindingRensa::ForFire,
        2,  // min puyos to complement
        13, // max puyos to complement
        callback,
    );

    (max_chains, highest_ignition_y as f64)
}


/// Calculate the good_chains threshold based on field complexity
/// Formula: min(14, count_color_puyos_connected_from_start(f) / 5 + 4)
fn calculate_good_chains(field: &CoreField) -> usize {
    let connected_count = count_color_puyos_connected_from_start(field);
    min(14, connected_count / 5 + 4)
}

/// Count color puyos connected from the starting position (column 3, row 13)
/// This estimates the complexity/development of the field
fn count_color_puyos_connected_from_start(field: &CoreField) -> usize {
    let mut visited = [[false; 14]; 8];
    let mut count = 0;

    // Start DFS from column 3 (middle), going downward
    for start_y in (1..=13).rev() {
        if !visited[3][start_y] {
            let color = field.color(3, start_y);
            if color.is_normal_color() {
                count += count_connected_for_good_chains(field, 3, start_y, &mut visited);
            }
        }
    }

    count
}

/// Count connected puyos starting from a position (for good_chains calculation)
/// This counts all connected normal-color puyos, not just same color
fn count_connected_for_good_chains(
    field: &CoreField,
    x: usize,
    y: usize,
    visited: &mut [[bool; 14]; 8],
) -> usize {
    if x < 1 || x > 6 || y < 1 || y > 13 {
        return 0;
    }
    if visited[x][y] {
        return 0;
    }

    let color = field.color(x, y);
    if !color.is_normal_color() {
        return 0;
    }

    visited[x][y] = true;
    let mut count = 1;

    // Check 4 directions - count all normal-color puyos regardless of color
    if x > 1 {
        count += count_connected_for_good_chains(field, x - 1, y, visited);
    }
    if x < 6 {
        count += count_connected_for_good_chains(field, x + 1, y, visited);
    }
    if y > 1 {
        count += count_connected_for_good_chains(field, x, y - 1, visited);
    }
    if y < 13 {
        count += count_connected_for_good_chains(field, x, y + 1, visited);
    }

    count
}

/// Count connected puyos of the same color using DFS
fn count_connected(
    field: &CoreField,
    x: usize,
    y: usize,
    color: PuyoColor,
    visited: &mut [[bool; 14]; 8],
) -> usize {
    if x < 1 || x > 6 || y < 1 || y > 13 {
        return 0;
    }
    if visited[x][y] {
        return 0;
    }
    if field.color(x, y) != color {
        return 0;
    }

    visited[x][y] = true;
    let mut count = 1;

    // Check 4 directions
    if x > 1 {
        count += count_connected(field, x - 1, y, color, visited);
    }
    if x < 6 {
        count += count_connected(field, x + 1, y, color, visited);
    }
    if y > 1 {
        count += count_connected(field, x, y - 1, color, visited);
    }
    if y < 13 {
        count += count_connected(field, x, y + 1, color, visited);
    }

    count
}
