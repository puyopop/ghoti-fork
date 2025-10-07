use std::time::Instant;

use puyoai::{
    color::Color,
    decision::Decision,
    field::CoreField,
    kumipuyo::kumipuyo_seq::generate_random_puyocolor_sequence,
    plan::Plan,
};

use crate::{bot::*, evaluator::Evaluator};

/// ハイブリッドAI - 序盤は形重視、中盤から連鎖重視に切り替える適応型AI
pub struct HybridAI {
    stable_evaluator: Evaluator,
    chain_evaluator: Evaluator,
}

impl HybridAI {
    pub fn new_customize(base_evaluator: Evaluator) -> Self {
        HybridAI {
            stable_evaluator: Self::create_stable_evaluator(),
            chain_evaluator: Self::create_chain_evaluator(),
        }
    }

    fn create_stable_evaluator() -> Evaluator {
        let mut evaluator = Evaluator::default();

        // 序盤用：形重視
        evaluator.valley *= 2;
        evaluator.ridge *= 2;
        evaluator.ideal_height_diff *= 2;
        evaluator.connectivity_2 = (evaluator.connectivity_2 as f32 * 1.5) as i32;
        evaluator.connectivity_3 = (evaluator.connectivity_3 as f32 * 1.5) as i32;
        evaluator.third_column_height = (evaluator.third_column_height as f32 * 1.5) as i32;

        // GTRパターンを重視
        evaluator.gtr_base_1 *= 3;
        evaluator.gtr_base_2 *= 3;
        evaluator.gtr_base_3 *= 3;

        evaluator
    }

    fn create_chain_evaluator() -> Evaluator {
        let mut evaluator = Evaluator::default();

        // 中盤以降用：連鎖重視
        evaluator.chain *= 2;
        evaluator.chain_score *= 2;
        evaluator.potential_main_chain *= 2;
        evaluator.potential_main_chain_sq = (evaluator.potential_main_chain_sq as f32 * 1.5) as i32;
        evaluator.potential_sub_chain = (evaluator.potential_sub_chain as f32 * 1.5) as i32;

        // 形は最低限維持
        evaluator.valley = (evaluator.valley as f32 * 0.8) as i32;
        evaluator.ridge = (evaluator.ridge as f32 * 0.8) as i32;

        evaluator
    }

    fn get_phase(cf: &CoreField, tumo_index: usize) -> Phase {
        // 平均の高さを計算
        let avg_height: f32 = (1..=6)
            .map(|x| cf.height(x) as i16)
            .sum::<i16>() as f32 / 6.0;

        // ゲームの進行度を判定
        if tumo_index < 15 || avg_height < 4.0 {
            Phase::Opening  // 序盤
        } else if tumo_index < 35 || avg_height < 7.0 {
            Phase::Middle   // 中盤
        } else {
            Phase::Endgame  // 終盤
        }
    }

    fn select_evaluator(&self, phase: &Phase) -> &Evaluator {
        match phase {
            Phase::Opening => &self.stable_evaluator,
            Phase::Middle | Phase::Endgame => &self.chain_evaluator,
        }
    }
}

#[derive(Debug)]
enum Phase {
    Opening,  // 序盤（形作り）
    Middle,   // 中盤（連鎖準備）
    Endgame,  // 終盤（発火）
}

impl AI for HybridAI {
    fn new() -> Self {
        HybridAI {
            stable_evaluator: Self::create_stable_evaluator(),
            chain_evaluator: Self::create_chain_evaluator(),
        }
    }

    fn name(&self) -> &'static str {
        "HybridAI"
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
        let tumo_index = player_state_1p.tumo_index;

        // 現在のフェーズを判定
        let phase = Self::get_phase(cf, tumo_index);
        let evaluator = self.select_evaluator(&phase);

        // フェーズに応じて先読み深さを調整
        let visible_tumos = seq.len();
        let depth = match phase {
            Phase::Opening => 2.min(visible_tumos),   // 序盤は浅く
            Phase::Middle => 3.min(visible_tumos),    // 中盤は標準
            Phase::Endgame => 4.min(visible_tumos),   // 終盤は深く
        };

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
            let mut score = evaluator.evaluate(plan);

            // フェーズ固有の追加評価
            match phase {
                Phase::Opening => {
                    // 序盤は高く積みすぎないように
                    if plan.field().height(3) > 8 {
                        score -= 1000;
                    }
                }
                Phase::Middle => {
                    // 中盤は連鎖の準備を評価
                    if plan.chain() == 0 {
                        // 連鎖が起きない手でも、連結を評価
                        for x in 1..=6 {
                            for y in 1..=plan.field().height(x) {
                                if plan.field().color(x, y).is_normal_color() {
                                    let connected = plan.field().count_connected(x, y);
                                    if connected == 3 {
                                        score += 100;  // 3連結にボーナス
                                    }
                                }
                            }
                        }
                    }
                }
                Phase::Endgame => {
                    // 終盤は連鎖を積極的に狙う
                    if plan.chain() > 0 {
                        score += plan.chain() as i32 * 3000;
                        score += (plan.score() / 100) as i32;
                    }

                    // 相手の状況を考慮（実装されている場合）
                    if let Some(ref state_2p) = player_state_2p {
                        if state_2p.field.height(3) > 10 {
                            // 相手が高い場合は早めに発火を狙う
                            if plan.chain() > 3 {
                                score += 5000;
                            }
                        }
                    }
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
            let phase_str = match phase {
                Phase::Opening => "Open",
                Phase::Middle => "Mid",
                Phase::Endgame => "End",
            };
            if plan.chain() > 0 {
                format!("[{}] Chain:{} Score:{}", phase_str, plan.chain(), best_score)
            } else {
                format!("[{}] Build Score:{}", phase_str, best_score)
            }
        } else {
            "No valid move".to_string()
        };

        AIDecision::new(best_decisions, log_output, start.elapsed())
    }
}