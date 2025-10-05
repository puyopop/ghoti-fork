use clap::Parser;
use cpu::bot::{BeamSearchAI, PlayerState, AI};
use cpu::evaluator::Evaluator;
use ghoti_simulator::puyop_decoder::PuyopDecoder;
use ghoti_simulator::puyop_parser::PuyopParser;
use puyoai::{
    color::PuyoColor,
    decision::Decision,
    field::CoreField,
    kumipuyo::Kumipuyo,
    plan::Plan,
};

#[derive(Parser)]
#[clap(
    name = "Position Analyzer",
    about = "Analyze a specific Puyo Puyo position and show best moves"
)]
struct Opts {
    /// puyop.com形式のURL
    /// 例: "http://www.puyop.com/s/420Aa9r9hj" または "420Aa9r9hj_0a0b"
    #[clap(long)]
    url: Option<String>,

    /// フィールド文字列（78文字、下から上へ）
    /// 例: "........." (6列×13段 = 78文字)
    #[clap(long, conflicts_with = "url")]
    field: Option<String>,

    /// ツモ文字列（カンマ区切り）
    /// 例: "RR,BY,GG,RY,GB"
    #[clap(long, default_value = "RR,BY,GG,RY,GB")]
    tumos: String,

    /// 上位何手を表示するか
    #[clap(long, default_value = "10")]
    top_n: usize,

    /// 評価値の詳細を表示
    #[clap(long)]
    verbose: bool,

    /// AI読み深さ（デフォルトは1手のみ評価）
    #[clap(long, default_value = "1")]
    depth: usize,

    /// 指定手数分最善手を進めてから解析
    #[clap(long, default_value = "0")]
    advance: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::parse();

    // 盤面とツモを取得
    let (mut field, mut tumos) = if let Some(url) = opts.url {
        // puyop.com URLをデコード
        let decoder = PuyopDecoder::new();
        let (f, mut t, _) = decoder.decode_url(&url)?;

        // URLにツモがない場合、コマンドライン引数から取得
        if t.is_empty() {
            t = PuyopParser::parse_tumos(&opts.tumos)?;
        }

        (f, t)
    } else {
        // URLなしの場合、空の盤面とコマンドライン引数のツモ
        let f = CoreField::new();
        let t = PuyopParser::parse_tumos(&opts.tumos)?;
        (f, t)
    };

    // 指定手数分進める
    if opts.advance > 0 {
        println!("=== 最善手を{}手進めます ===\n", opts.advance);

        for move_num in 1..=opts.advance {
            if tumos.is_empty() {
                eprintln!("警告: ツモが不足しています（{}手目で終了）", move_num);
                break;
            }

            // 現在の盤面で最善手を計算
            let ai = BeamSearchAI::new();
            let candidates = analyze_all_moves(&ai, &field, &tumos, 1);

            if candidates.is_empty() {
                eprintln!("警告: 有効な手がありません（{}手目で終了）", move_num);
                break;
            }

            let best = &candidates[0];
            let current_tumo = &tumos[0];

            println!("{}手目: 列{} 回転{} (ツモ: {}, 評価値: {})",
                move_num,
                best.decision.axis_x(),
                format_rotation(best.decision.rot()),
                format_kumipuyo(current_tumo),
                best.eval_score
            );

            if best.chain > 0 {
                println!("    🔥 {}連鎖 ({}点)", best.chain, best.score);
            }

            // 盤面を更新
            field.drop_kumipuyo(&best.decision, current_tumo);
            field.simulate();

            // ツモを消費
            tumos.remove(0);
        }
        println!();
    }

    println!("=== Position Analysis ===\n");
    print_field(&field);
    println!("\nTumos: {}", format_tumos(&tumos));
    println!();

    // 全候補を評価
    let ai = BeamSearchAI::new();
    let candidates = analyze_all_moves(&ai, &field, &tumos, opts.depth);

    // 上位N件を表示
    println!("=== Top {} Moves ===\n", opts.top_n);
    for (i, candidate) in candidates.iter().take(opts.top_n).enumerate() {
        println!(
            "{}位: 列{} 回転{} - 評価値: {}",
            i + 1,
            candidate.decision.axis_x(),
            format_rotation(candidate.decision.rot()),
            candidate.eval_score
        );

        if opts.verbose {
            println!("    連鎖: {}連鎖, 得点: {}", candidate.chain, candidate.score);
            println!(
                "    ツモ: {} → 列{}",
                format_kumipuyo(&tumos[0]),
                candidate.decision.axis_x()
            );
        }

        if candidate.chain > 0 {
            println!("    🔥 発火! {}連鎖 ({}点)", candidate.chain, candidate.score);
        }
        println!();
    }

    // 最善手を実行した後の盤面を表示
    if !candidates.is_empty() && opts.verbose {
        let best = &candidates[0];
        let mut after_field = field.clone();
        after_field.drop_kumipuyo(&best.decision, &tumos[0]);
        after_field.simulate();

        println!("=== 最善手実行後の盤面 ===\n");
        print_field(&after_field);
    }

    Ok(())
}

struct Candidate {
    decision: Decision,
    eval_score: i32,
    chain: usize,
    score: usize,
}

fn analyze_all_moves(
    ai: &BeamSearchAI,
    field: &CoreField,
    tumos: &Vec<Kumipuyo>,
    depth: usize,
) -> Vec<Candidate> {
    let evaluator = Evaluator::default();
    let mut candidates = Vec::new();

    if depth > 1 {
        // BeamSearchAIを使う場合：depth手先まで読んで最善手を選択
        let player_state = PlayerState::new(
            0,                  // frame
            field.clone(),      // field
            tumos.clone(),      // seq
            0,                  // score
            0,                  // carry_over
            0,                  // fixed_ojama
            0,                  // pending_ojama
            0,                  // current_chain
            0,                  // tumo_index
            None,               // haipuyo
        );

        // AIに思考させる（ビームサーチでdepth手先まで探索）
        let ai_decision = ai.think(player_state, None, None);

        // AIが推奨する手順（複数手）を取得
        // decisionsには最善の手順が含まれる（最初の1手が次の最善手）
        for decision in ai_decision.decisions.iter().take(1) {
            // この手を実行した結果を評価
            Plan::iterate_available_plans(field, tumos, 1, &mut |plan: &Plan| {
                if plan.first_decision().axis_x() == decision.axis_x()
                    && plan.first_decision().rot() == decision.rot() {
                    let eval_score = evaluator.evaluate(plan);
                    candidates.push(Candidate {
                        decision: decision.clone(),
                        eval_score,
                        chain: plan.chain(),
                        score: plan.score(),
                    });
                }
            });
        }

        // AIの最善手以外の候補も追加（比較のため）
        Plan::iterate_available_plans(field, tumos, 1, &mut |plan: &Plan| {
            let decision = plan.first_decision();
            let is_ai_choice = ai_decision.decisions.first()
                .map(|d| d.axis_x() == decision.axis_x() && d.rot() == decision.rot())
                .unwrap_or(false);

            if !is_ai_choice {
                let eval_score = evaluator.evaluate(plan);
                candidates.push(Candidate {
                    decision: decision.clone(),
                    eval_score,
                    chain: plan.chain(),
                    score: plan.score(),
                });
            }
        });

        // 評価値でソート（AI選択の手は既に先頭に追加済み、残りを評価値順に）
        let ai_candidates_count = 1;
        candidates[ai_candidates_count..].sort_by(|a, b| b.eval_score.cmp(&a.eval_score));

        candidates
    } else {
        // depth=1の場合：全候補を1手先の評価値で評価
        Plan::iterate_available_plans(field, tumos, depth, &mut |plan: &Plan| {
            let decision = plan.first_decision().clone();
            let eval_score = evaluator.evaluate(plan);
            let chain = plan.chain();
            let score = plan.score();

            candidates.push(Candidate {
                decision,
                eval_score,
                chain,
                score,
            });
        });

        // 評価値でソート（降順）
        candidates.sort_by(|a, b| b.eval_score.cmp(&a.eval_score));

        candidates
    }
}

fn print_field(field: &CoreField) {
    println!("  1 2 3 4 5 6");
    for y in (1..=13).rev() {
        print!("{:2}│", y);
        for x in 1..=6 {
            let c = match field.color(x, y) {
                PuyoColor::RED => "🔴",
                PuyoColor::BLUE => "🔵",
                PuyoColor::YELLOW => "🟡",
                PuyoColor::GREEN => "🟢",
                PuyoColor::OJAMA => "⚪",
                _ => "  ",
            };
            print!("{}", c);
        }
        println!("│");
    }
    println!("  └───────────┘");
}

fn format_tumos(tumos: &[Kumipuyo]) -> String {
    tumos
        .iter()
        .map(|k| format_kumipuyo(k))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_kumipuyo(k: &Kumipuyo) -> String {
    format!(
        "{}{}",
        color_to_emoji(k.axis()),
        color_to_emoji(k.child())
    )
}

fn color_to_emoji(color: PuyoColor) -> &'static str {
    match color {
        PuyoColor::RED => "🔴",
        PuyoColor::BLUE => "🔵",
        PuyoColor::YELLOW => "🟡",
        PuyoColor::GREEN => "🟢",
        _ => "⚫",
    }
}

fn format_rotation(rot: usize) -> &'static str {
    match rot {
        0 => "↑(上)",
        1 => "→(右)",
        2 => "↓(下)",
        3 => "←(左)",
        _ => "?",
    }
}
