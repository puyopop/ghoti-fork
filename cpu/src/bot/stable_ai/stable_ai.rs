use std::time::Instant;

use puyoai::{
    decision::Decision,
    field::CoreField,
    kumipuyo::kumipuyo_seq::generate_random_puyocolor_sequence,
    plan::Plan,
};

use crate::{bot::*, evaluator::Evaluator};

/// 安定重視AI - 盤面の形を整えることを重視し、安定した戦いを目指すAI
pub struct StableAI {
    evaluator: Evaluator,
}

impl StableAI {
    pub fn new_customize(evaluator: Evaluator) -> Self {
        StableAI { evaluator }
    }

    fn create_evaluator() -> Evaluator {
        let mut evaluator = Evaluator::default();

        // 形の評価を大幅に強化
        evaluator.valley *= 3;  // 谷を作らないように
        evaluator.ridge *= 3;   // 尾根を作らないように
        evaluator.ideal_height_diff *= 2;  // 理想的な高さを維持
        evaluator.ideal_height_diff_sq *= 2;
        evaluator.third_column_height *= 2;  // 3列目の高さを適切に
        evaluator.unreachable_space *= 3;  // 到達不可能な空間を作らない

        // 連結も重視（安定した形を作る）
        evaluator.connectivity_2 *= 2;
        evaluator.connectivity_3 *= 2;

        // 連鎖は控えめに評価
        evaluator.chain = (evaluator.chain as f32 * 0.7) as i32;
        evaluator.chain_score = (evaluator.chain_score as f32 * 0.7) as i32;

        // 操作の効率も重視
        evaluator.chigiri *= 2;  // ちぎりは避ける
        evaluator.move_frame *= 2;  // 操作時間も考慮

        // GTR系のパターンは維持（安定した形として）
        evaluator.gtr_base_1 *= 2;
        evaluator.gtr_base_2 *= 2;
        evaluator.gtr_base_3 *= 2;

        evaluator
    }

    fn evaluate_stability(cf: &CoreField) -> i32 {
        let mut score = 0;

        // 各列の高さのばらつきを評価（小さいほど良い）
        let heights: Vec<i16> = (1..=6).map(|x| cf.height(x) as i16).collect();
        let avg_height: f32 = heights.iter().sum::<i16>() as f32 / 6.0;
        let variance: f32 = heights
            .iter()
            .map(|&h| {
                let diff = h as f32 - avg_height;
                diff * diff
            })
            .sum::<f32>() / 6.0;

        score -= (variance * 100.0) as i32;

        // 隣接列の高さの差を評価（小さいほど良い）
        for i in 1..6 {
            let diff = (heights[i] - heights[i - 1]).abs();
            if diff > 3 {
                score -= diff as i32 * 50;  // 大きな段差にペナルティ
            }
        }

        // 3列目が高すぎる場合のペナルティ
        if cf.height(3) > 10 {
            score -= (cf.height(3) - 10) as i32 * 100;
        }

        score
    }
}

impl AI for StableAI {
    fn new() -> Self {
        StableAI {
            evaluator: Self::create_evaluator(),
        }
    }

    fn name(&self) -> &'static str {
        "StableAI"
    }

    fn think(
        &self,
        player_state_1p: PlayerState,
        _player_state_2p: Option<PlayerState>,
        _think_frame: Option<usize>,
    ) -> AIDecision {
        let start = Instant::now();

        let cf = &player_state_1p.field;
        let seq = &player_state_1p.seq;

        // 見えているツモが少ない場合はランダムツモを追加
        let visible_tumos = seq.len();
        let depth = 2.min(visible_tumos);  // 2手先まで見る（安定重視なので浅めで良い）

        let seq: Vec<_> = seq
            .iter()
            .cloned()
            .chain(generate_random_puyocolor_sequence(
                if depth > visible_tumos {
                    depth - visible_tumos
                } else {
                    0
                },
            ))
            .collect();

        // 全ての可能な手を評価
        let mut best_plan: Option<Plan> = None;
        let mut best_score = i32::MIN;
        let mut best_decisions = vec![];

        Plan::iterate_available_plans(&cf, &seq, depth, &mut |plan: &Plan| {
            let mut score = self.evaluator.evaluate(plan);

            // 安定性の追加評価
            score += Self::evaluate_stability(plan.field());

            // 死んでしまう手は大きくペナルティ
            if plan.field().is_dead() {
                score -= 100000;
            }

            // 13段目に置く手はペナルティ
            for x in 1..=6 {
                if !plan.field().is_empty(x, 13) {
                    score -= 5000;
                }
            }

            if score > best_score {
                best_score = score;
                best_plan = Some(plan.clone());
                best_decisions = vec![plan.first_decision().clone()];
            }
        });

        // 最善手が見つからない場合は中央に置く
        if best_decisions.is_empty() {
            best_decisions = vec![Decision::new(3, 0)];
        }

        let log_output = if let Some(plan) = best_plan {
            let avg_height: f32 = (1..=6)
                .map(|x| plan.field().height(x) as i16)
                .sum::<i16>() as f32 / 6.0;
            format!("Stable (h:{:.1}) Score: {}", avg_height, best_score)
        } else {
            "Emergency placement".to_string()
        };

        AIDecision::new(best_decisions, log_output, start.elapsed())
    }
}