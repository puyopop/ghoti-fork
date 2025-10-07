use std::{sync::mpsc, thread, time::Instant};

use puyoai::{
    color::PuyoColor,
    decision::Decision,
    es_field::EsCoreField,
    field::CoreField,
    kumipuyo::{kumipuyo_seq::generate_random_puyocolor_sequence, Kumipuyo},
    plan::Plan,
};

use crate::{bot::*, evaluator::Evaluator, opening_matcher::OpeningMatcher};

pub struct ChainPotentialAI {
    evaluator: Evaluator,
    opening_matcher: OpeningMatcher,
}

impl AI for ChainPotentialAI {
    fn new() -> Self {
        ChainPotentialAI {
            evaluator: Evaluator::default(),
            opening_matcher: OpeningMatcher::new("opening_vis2.json").unwrap(),
        }
    }

    fn name(&self) -> &'static str {
        "ChainPotentialAI"
    }

    fn think(
        &self,
        player_state_1p: PlayerState,
        player_state_2p: Option<PlayerState>,
        think_frame: Option<usize>,
    ) -> AIDecision {
        let think_frame = think_frame.unwrap_or(0);
        let (depth, width) = if think_frame <= 2 {
            (20, 100)
        } else if think_frame <= 8 {
            (30, 200)
        } else {
            (40, 400)  // ビーム幅を400に拡大
        };

        // モンテカルロシミュレーション10回
        self.think_with_monte_carlo(player_state_1p, player_state_2p, depth, width, 10)
    }
}

impl ChainPotentialAI {
    fn think_with_monte_carlo(
        &self,
        player_state_1p: PlayerState,
        player_state_2p: Option<PlayerState>,
        depth: usize,
        width: usize,
        parallel: usize,
    ) -> AIDecision {
        let start = Instant::now();

        // 最序盤のみテンプレを使う
        if player_state_1p.tumo_index < 5 {
            if let Some(decision) = self.opening_matcher.find_opening(
                player_state_1p.tumo_index,
                &player_state_1p.field,
                &player_state_1p.seq,
            ) {
                return AIDecision::from_decision(
                    &decision,
                    format!("OpeningMatcher"),
                    start.elapsed(),
                );
            }
        }

        // ツモが十分に渡されてたら、モンテカルロをする必要がない
        let parallel = if player_state_1p.seq.len() < depth {
            parallel
        } else {
            1
        };

        // 各スレッドの結果をまとめる
        let (tx, rx): (mpsc::Sender<AIDecision>, mpsc::Receiver<AIDecision>) = mpsc::channel();

        for _ in 0..parallel {
            let depth_c = depth;
            let width_c = width;
            let tx_c = tx.clone();
            let player_state_1p_c = player_state_1p.clone();
            let player_state_2p_c = player_state_2p.clone();
            let evaluator_c = self.evaluator.clone();
            let opening_matcher_c = self.opening_matcher.clone();

            thread::spawn(move || {
                let ai = ChainPotentialAI {
                    evaluator: evaluator_c,
                    opening_matcher: opening_matcher_c,
                };
                tx_c.send(ai.think_single_thread(
                    &player_state_1p_c,
                    &player_state_2p_c,
                    depth_c,
                    width_c,
                ))
                .ok();
            });
        }

        // scores[x][r] := 解として選ばれた回数
        let mut scores = [[0_i32; 4]; 7];
        let mut ai_decisions = Vec::with_capacity(parallel);

        for _ in 0..parallel {
            if let Ok(ai_decision) = rx.recv() {
                // 発火判定があったらすぐにそれを打つ
                if ai_decision.log_output.contains("FIRE") {
                    return AIDecision::new(
                        ai_decision.decisions.clone(),
                        ai_decision.log_output.clone(),
                        start.elapsed(),
                    );
                }

                if !ai_decision.decisions.is_empty() {
                    let first_decision = &ai_decision.decisions[0];
                    let x = first_decision.axis_x();
                    let r = first_decision.rot();
                    scores[x][r] += 1;
                    ai_decisions.push(ai_decision);
                }
            } else {
                break;
            }
        }

        // 最も多く選ばれた手を選択
        let best_decision = Decision::all_valid_decisions()
            .iter()
            .max_by(|d1, d2| scores[d1.axis_x()][d1.rot()].cmp(&scores[d2.axis_x()][d2.rot()]))
            .unwrap();

        if let Some(ai_decision) = ai_decisions
            .iter()
            .find(|&ai_decision| &ai_decision.decisions[0] == best_decision)
        {
            return AIDecision::new(
                ai_decision.decisions.clone(),
                format!("{} (votes: {}/{})",
                    ai_decision.log_output,
                    scores[best_decision.axis_x()][best_decision.rot()],
                    parallel),
                start.elapsed(),
            );
        }

        // どうしようもないので自殺
        AIDecision::new(
            vec![Decision::new(3, 0)],
            format!("muri..."),
            start.elapsed(),
        )
    }

    fn think_single_thread(
        &self,
        player_state_1p: &PlayerState,
        _player_state_2p: &Option<PlayerState>,
        depth: usize,
        width: usize,
    ) -> AIDecision {
        let start = Instant::now();

        let cf = &player_state_1p.field;
        let seq = &player_state_1p.seq;

        // ツモを伸ばす
        let visible_tumos = seq.len();
        let seq: Vec<Kumipuyo> = seq
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

        let mut state_v: Vec<State> = vec![State::from_field(cf)];
        let mut fired_states: Vec<State> = Vec::new();

        for cur_depth in 0..depth.min(seq.len()) {
            // ビーム内の初手がすべて同じなら終わり
            if cur_depth > 0
                && state_v
                    .iter()
                    .all(|state| state.first_decision() == state_v[0].first_decision())
            {
                break;
            }

            // 次の状態を列挙
            let mut next_state_v: Vec<State> = Vec::with_capacity(width * 22);
            for cur_state in &state_v {
                self.generate_next_states(
                    &cur_state,
                    &mut next_state_v,
                    &mut fired_states,
                    &seq[cur_depth],
                    cur_depth < visible_tumos,
                );
            }

            // 8万点以上の発火可能な手があれば即座に選択
            if let Some(fire_state) = fired_states.iter()
                .filter(|s| s.chain_score >= 80000)
                .max_by_key(|s| s.chain_score)
            {
                return AIDecision::new(
                    fire_state.decisions.clone(),
                    format!("FIRE: {} points, {} chain!", fire_state.chain_score, fire_state.chain_count),
                    start.elapsed(),
                );
            }

            if next_state_v.is_empty() {
                break;
            }

            // 良い方からビーム幅分だけ残す
            next_state_v.sort_by(|a, b| b.eval_score.cmp(&a.eval_score));
            if next_state_v.len() > width {
                next_state_v.truncate(width);
            }
            state_v = next_state_v;
        }

        if state_v[0].first_decision().is_some() {
            return AIDecision::new(
                state_v[0].decisions.clone(),
                format!("eval: {}, potential: {}", state_v[0].eval_score, state_v[0].chain_potential),
                start.elapsed(),
            );
        }

        // どうしようもないので自殺
        AIDecision::new(
            vec![Decision::new(3, 0)],
            format!("muri..."),
            start.elapsed(),
        )
    }

    fn generate_next_states(
        &self,
        cur_state: &State,
        next_states: &mut Vec<State>,
        fired_states: &mut Vec<State>,
        kumipuyo: &Kumipuyo,
        track_fired: bool,
    ) {
        let seq = vec![kumipuyo.clone()];

        Plan::iterate_available_plans(&cur_state.field, &seq, 1, &mut |plan: &Plan| {
            let mut decisions = cur_state.decisions.clone();
            decisions.push(plan.first_decision().clone());

            // 連鎖が発火する場合
            if track_fired && plan.chain() > 0 {
                fired_states.push(State {
                    field: plan.field().clone(),
                    decisions: decisions.clone(),
                    eval_score: plan.score() as i32,
                    chain_potential: 0,
                    chain_score: plan.score(),
                    chain_count: plan.chain(),
                });
            }

            // 基本評価値（谷・山の関係）
            let base_eval = self.evaluator.evaluate(plan);

            // 連鎖ポテンシャルを計算
            let chain_potential = self.calculate_chain_potential(plan.field());

            // 最終評価値 = 基本評価値 + 連鎖ポテンシャル * 重み
            let eval_score = base_eval + (chain_potential as i32 * 20);

            next_states.push(State {
                field: plan.field().clone(),
                decisions,
                eval_score,
                chain_potential,
                chain_score: 0,
                chain_count: 0,
            });
        });
    }

    fn calculate_chain_potential(&self, field: &CoreField) -> i32 {
        use puyoai::field::{plain_field::PlainField, bit_field::BitField};

        let colors = [PuyoColor::RED, PuyoColor::BLUE, PuyoColor::YELLOW, PuyoColor::GREEN];
        let mut max_potential = 0i32;

        // 1個目のぷよを仮想的に落とす（24通り）
        for x1 in 1..=6 {
            if field.height(x1) >= 12 {
                continue; // 列がほぼ埋まっている
            }

            for &color1 in &colors {
                // PlainFieldに現在のフィールドをコピー
                let mut pf1 = PlainField::<PuyoColor>::new();
                for x in 1..=6 {
                    for y in 1..=field.height(x) {
                        pf1.set_color(x, y, field.color(x, y));
                    }
                }

                // 1個目のぷよを配置
                let height1 = field.height(x1);
                pf1.set_color(x1, height1 + 1, color1);

                // BitField経由でCoreFieldに変換してシミュレーション
                let bf1 = BitField::from_plain_field(pf1.clone());
                let mut field1 = CoreField::from_bit_field(&bf1);
                let result1 = field1.es_simulate();

                if result1.score > 0 {
                    max_potential = max_potential.max(result1.score as i32);
                } else {
                    // 連鎖が発生しない場合、2個目を試す
                    for x2 in 1..=6 {
                        // pf1の高さを再計算
                        let mut heights = [0u16; 8];  // MAP_WIDTH = 8
                        pf1.calculate_height(&mut heights);
                        if heights[x2] >= 12 {
                            continue;
                        }

                        for &color2 in &colors {
                            let mut pf2 = pf1.clone();
                            let height2 = heights[x2] as usize;
                            pf2.set_color(x2, height2 + 1, color2);

                            // BitField経由でCoreFieldに変換
                            let bf2 = BitField::from_plain_field(pf2);
                            let mut field2 = CoreField::from_bit_field(&bf2);
                            let result2 = field2.es_simulate();

                            if result2.score > 0 {
                                max_potential = max_potential.max(result2.score as i32);
                            }
                        }
                    }
                }
            }
        }

        max_potential
    }
}

#[derive(Clone)]
struct State {
    field: CoreField,
    decisions: Vec<Decision>,
    eval_score: i32,
    chain_potential: i32,
    chain_score: usize,
    chain_count: usize,
}

impl State {
    fn from_field(field: &CoreField) -> Self {
        State {
            field: field.clone(),
            decisions: vec![],
            eval_score: 0,
            chain_potential: 0,
            chain_score: 0,
            chain_count: 0,
        }
    }

    fn first_decision(&self) -> Option<&Decision> {
        self.decisions.first()
    }
}