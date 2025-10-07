use std::time::Instant;

use puyoai::{
    decision::Decision,
    kumipuyo::kumipuyo_seq::generate_random_puyocolor_sequence,
    plan::Plan,
};

use crate::{bot::*, evaluator::Evaluator};

/// 連鎖重視AI - 大連鎖を狙うことに特化したAI
pub struct ChainFocusedAI {
    evaluator: Evaluator,
}

impl ChainFocusedAI {
    pub fn new_customize(evaluator: Evaluator) -> Self {
        ChainFocusedAI { evaluator }
    }

    fn create_evaluator() -> Evaluator {
        let mut evaluator = Evaluator::default();

        // 連鎖関連のパラメータを大幅に強化
        evaluator.chain *= 3;  // 実際の連鎖を3倍評価
        evaluator.chain_sq *= 2;  // 連鎖の2乗項も強化
        evaluator.chain_score *= 3;  // 連鎖得点を3倍評価
        evaluator.chain_frame *= 2;  // 連鎖時間も考慮

        // 潜在連鎖も重視
        evaluator.potential_main_chain *= 3;
        evaluator.potential_main_chain_sq *= 2;
        evaluator.potential_sub_chain *= 2;

        // 連結も重視（連鎖の種を作りやすくする）
        evaluator.connectivity_3 *= 2;

        // 形の評価を下げる（連鎖優先のため）
        evaluator.valley = (evaluator.valley as f32 * 0.5) as i32;
        evaluator.ridge = (evaluator.ridge as f32 * 0.5) as i32;
        evaluator.third_column_height = (evaluator.third_column_height as f32 * 0.7) as i32;

        evaluator
    }
}

impl AI for ChainFocusedAI {
    fn new() -> Self {
        ChainFocusedAI {
            evaluator: Self::create_evaluator(),
        }
    }

    fn name(&self) -> &'static str {
        "ChainFocusedAI"
    }

    fn think(
        &self,
        player_state_1p: PlayerState,
        player_state_2p: Option<PlayerState>,
        _think_frame: Option<usize>,
    ) -> AIDecision {
        let start = Instant::now();

        let cf = &player_state_1p.field;
        let seq = &player_state_1p.seq;

        // 見えているツモが少ない場合はランダムツモを追加
        let visible_tumos = seq.len();
        let depth = 3.min(visible_tumos);  // 最大3手先まで見る

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

            // 連鎖が発生する手には追加ボーナス
            if plan.chain() > 0 {
                score += plan.chain() as i32 * 5000;  // 連鎖数に応じた大きなボーナス
                score += (plan.score() / 100) as i32;  // 得点にもボーナス
            }

            if score > best_score {
                best_score = score;
                best_plan = Some(plan.clone());
                best_decisions = vec![plan.first_decision().clone()];
            }
        });

        // 最善手が見つからない場合は適当に置く
        if best_decisions.is_empty() {
            best_decisions = vec![Decision::new(3, 0)];
        }

        let log_output = if let Some(plan) = best_plan {
            if plan.chain() > 0 {
                format!("Chain: {} ({}pts) Score: {}", plan.chain(), plan.score(), best_score)
            } else {
                format!("Build Score: {}", best_score)
            }
        } else {
            "No valid move".to_string()
        };

        AIDecision::new(best_decisions, log_output, start.elapsed())
    }
}