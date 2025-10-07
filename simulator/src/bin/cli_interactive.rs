use std::io::{self, Write};

use cpu::bot::{BeamSearchAI, PlayerState, AI};
use puyoai::{
    color::PuyoColor,
    decision::Decision,
    field::CoreField,
    kumipuyo::Kumipuyo,
};

use ghoti_simulator::haipuyo_detector::*;
use ghoti_simulator::puyop_decoder::PuyopDecoder;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
    ExecutableCommand, QueueableCommand,
};

// Undo機能のための構造体
#[derive(Clone)]
struct GameSnapshot {
    player_state: PlayerState,
    score: usize,
    tumo_index: usize,
}

struct GameHistory {
    snapshots: Vec<GameSnapshot>,
    max_history: usize,
}

impl GameHistory {
    fn new(max_history: usize) -> Self {
        GameHistory {
            snapshots: Vec::with_capacity(max_history),
            max_history,
        }
    }

    fn push(&mut self, snapshot: GameSnapshot) {
        if self.snapshots.len() >= self.max_history {
            self.snapshots.remove(0);
        }
        self.snapshots.push(snapshot);
    }

    fn pop(&mut self) -> Option<GameSnapshot> {
        self.snapshots.pop()
    }
}

// チェインアニメーションのための構造体
#[derive(Clone, Debug)]
struct ChainStep {
    field: CoreField,
    _chain_number: usize,
    step_score: usize,
    description: String,
}

struct ChainAnimation {
    steps: Vec<ChainStep>,
    total_chains: usize,
    total_score: usize,
}

fn main() -> Result<(), std::io::Error> {
    // コマンドライン引数をチェック
    let args: Vec<String> = std::env::args().collect();
    let initial_url = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    // ターミナルをrawモードに設定
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();

    let result = run_game(&mut stdout, initial_url);

    // rawモードを解除
    terminal::disable_raw_mode()?;
    stdout.execute(cursor::Show)?;
    println!();

    result
}

fn run_game(stdout: &mut io::Stdout, initial_url: Option<String>) -> Result<(), std::io::Error> {
    stdout.execute(terminal::Clear(ClearType::All))?;
    stdout.execute(cursor::MoveTo(0, 0))?;

    println!("=== Puyo Puyo Interactive Mode ===\r");
    println!("Controls:\r");
    println!("  a/d       : Move left/right\r");
    println!("  s/Space   : Hard drop\r");
    println!("  j/k       : Rotate left/right\r");
    println!("  h         : Show AI suggestions\r");
    println!("  u         : Undo last move\r");
    println!("  q         : Exit game\r");
    println!("\r");

    // 初期URLから盤面を読み込む場合の表示
    if let Some(ref url) = initial_url {
        println!("Loading field from: {}\r", url);
        println!("\r");
    }

    println!("Press any key to start...\r");
    stdout.flush()?;

    // キー待ち
    loop {
        if let Event::Key(_) = event::read()? {
            break;
        }
    }

    let ai = BeamSearchAI::new();
    let visible_tumos = 3; // 現在手・次手・次々手
    let decoder = PuyopDecoder::new();

    // 初期状態を設定
    let seq = HaipuyoDetector::random_haipuyo();
    let mut player_state = PlayerState::initial_state(vec![], Some(seq.clone()));

    // URLが指定されている場合は盤面を読み込む
    if let Some(url) = initial_url {
        match decoder.decode_url(&url) {
            Ok((field, _, _)) => {
                player_state.field = field;
            }
            Err(e) => {
                println!("Failed to decode URL: {}\r", e);
                println!("Using empty field instead.\r");
                println!("Press any key to continue...\r");
                stdout.flush()?;
                event::read()?;
            }
        }
    }

    let mut score = player_state.score;
    let mut tumo_index = player_state.tumo_index;

    // Undo履歴を初期化
    let mut history = GameHistory::new(50);

    // サジェストのキャッシュを初期化
    let mut suggestions_cache: Option<(usize, Vec<(Decision, i32, String)>)> = None;

    loop {
        // ツモを設定
        player_state.set_seq(visible_tumos);

        // 初期位置とローテーション
        let mut x = 3; // 3列目
        let mut r = 0; // 縦上向き

        loop {
            // 画面クリアして再描画
            stdout.execute(terminal::Clear(ClearType::All))?;
            stdout.execute(cursor::MoveTo(0, 0))?;

            // 盤面とカーソル位置を表示（AIサジェスト付き）
            display_game_state_with_cursor_and_suggestions(&ai, &player_state, score, tumo_index, x, r, &mut suggestions_cache);
            stdout.flush()?;

            // キー入力を待つ
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') => {
                        println!("\r\nGame ended. Final score: {}\r", score);
                        return Ok(());
                    }
                    KeyCode::Char('h') => {
                        // AIのサジェストを表示
                        stdout.execute(terminal::Clear(ClearType::All))?;
                        stdout.execute(cursor::MoveTo(0, 0))?;
                        show_ai_suggestions(&ai, &player_state, tumo_index, &mut suggestions_cache);
                        println!("\r\nPress any key to continue...\r");
                        stdout.flush()?;
                        event::read()?;
                        continue;
                    }
                    KeyCode::Char('u') => {
                        // Undo機能
                        if let Some(snapshot) = history.pop() {
                            player_state = snapshot.player_state;
                            score = snapshot.score;
                            tumo_index = snapshot.tumo_index;
                            // undoした場合はキャッシュをクリア（tumo_indexが変わるため）
                            suggestions_cache = None;
                            break; // 内側のループから抜けて即座に再描画
                        } else {
                            // 履歴がない場合は何もしない（画面を維持）
                            continue;
                        }
                    }
                    KeyCode::Char('a') => {
                        // 左に移動
                        if x > 1 {
                            x -= 1;
                        }
                    }
                    KeyCode::Char('d') => {
                        // 右に移動
                        if x < 6 {
                            x += 1;
                        }
                    }
                    KeyCode::Char('j') => {
                        // 左回転
                        r = (r + 3) % 4;
                    }
                    KeyCode::Char('k') => {
                        // 右回転
                        r = (r + 1) % 4;
                    }
                    KeyCode::Char('s') | KeyCode::Char(' ') => {
                        // ハードドロップ
                        let decision = Decision::new(x, r);

                        // 合法手かチェック
                        if !is_valid_decision(&player_state.field, &player_state.seq[0], &decision)
                        {
                            // 不正な手の場合は何もせず、そのまま操作を続ける
                            continue;
                        }

                        // 現在の状態を履歴に保存
                        history.push(GameSnapshot {
                            player_state: player_state.clone(),
                            score,
                            tumo_index,
                        });

                        // ぷよを落とす
                        player_state.drop_kumipuyo(&decision);

                        // 連鎖アニメーションをチェック
                        let mut test_field = player_state.field.clone();
                        let rensa_result = test_field.simulate();

                        if rensa_result.chain > 0 {
                            // 連鎖が発生する場合、アニメーションを表示
                            let animation = create_chain_animation(&player_state.field);
                            display_chain_animation(stdout, &animation)?;

                            // 実際のシミュレーションを適用
                            player_state.field.simulate();
                            score += rensa_result.score;
                        } else {
                            // 連鎖がない場合は通常通り更新
                            player_state.field = test_field;
                            score += rensa_result.score;
                        }

                        // 死んだかチェック
                        if player_state.field.is_dead() {
                            stdout.execute(terminal::Clear(ClearType::All))?;
                            stdout.execute(cursor::MoveTo(0, 0))?;
                            display_field(&player_state.field);
                            println!("\r\n💀 Game Over! Final score: {}\r", score);
                            println!("\r\nPress any key to exit...\r");
                            stdout.flush()?;
                            event::read()?;
                            return Ok(());
                        }

                        tumo_index += 1;
                        player_state.tumo_index = tumo_index;

                        if tumo_index >= 100 {
                            stdout.execute(terminal::Clear(ClearType::All))?;
                            stdout.execute(cursor::MoveTo(0, 0))?;
                            display_field(&player_state.field);
                            println!("\r\n🏁 Reached max turns! Final score: {}\r", score);
                            println!("\r\nPress any key to exit...\r");
                            stdout.flush()?;
                            event::read()?;
                            return Ok(());
                        }

                        break; // 次のツモへ（即座に次の画面へ）
                    }
                    _ => {}
                }
            }
        }
    }
}

fn _display_game_state(player_state: &PlayerState, score: usize, tumo_index: usize) {
    println!("\n{}", "=".repeat(40));
    println!("Turn: {}  Score: {}", tumo_index + 1, score);
    println!("{}", "=".repeat(40));

    // 盤面を表示
    display_field(&player_state.field);

    // 現在のツモを表示
    println!("\nNext puyos:");
    for (i, kumipuyo) in player_state.seq.iter().enumerate() {
        let pos_name = match i {
            0 => "Current",
            1 => "Next   ",
            2 => "2nd    ",
            _ => "       ",
        };
        println!(
            "  {}: {}{}",
            pos_name,
            color_to_char(kumipuyo.axis()),
            color_to_char(kumipuyo.child())
        );
    }
}


fn display_game_state_with_cursor_and_suggestions(
    ai: &BeamSearchAI,
    player_state: &PlayerState,
    score: usize,
    tumo_index: usize,
    cursor_x: usize,
    rotation: usize,
    suggestions_cache: &mut Option<(usize, Vec<(Decision, i32, String)>)>,
) {
    println!("\r\n{}\r", "=".repeat(60));

    // 現在の手でゲームオーバーになるかチェック
    let decision = Decision::new(cursor_x, rotation);
    let will_die = will_die_after_drop(&player_state.field, &player_state.seq[0], &decision);

    if will_die {
        println!("Turn: {}  Score: {}  ⚠️  GAME OVER if placed here!\r", tumo_index + 1, score);
    } else {
        println!("Turn: {}  Score: {}\r", tumo_index + 1, score);
    }

    println!("{}\r", "=".repeat(60));

    // 現在の盤面のpuyop.com URLを生成
    let decoder = PuyopDecoder::new();
    let puyop_url = decoder.field_to_puyop_url(&player_state.field);
    println!("📋 Puyop URL: {}\r", puyop_url);
    println!("{}\r", "=".repeat(60));

    // AIサジェストをキャッシュから取得または計算
    let suggestions = if let Some((cached_tumo_index, cached_suggestions)) = suggestions_cache {
        if *cached_tumo_index == tumo_index {
            // キャッシュが有効
            cached_suggestions.clone()
        } else {
            // 新しいツモなので再計算
            let new_suggestions = ai.get_suggestions(player_state.clone());
            *suggestions_cache = Some((tumo_index, new_suggestions.clone()));
            new_suggestions
        }
    } else {
        // 初回計算
        let new_suggestions = ai.get_suggestions(player_state.clone());
        *suggestions_cache = Some((tumo_index, new_suggestions.clone()));
        new_suggestions
    };

    // フィールドとAIサジェストを横並びで表示
    display_field_and_suggestions_side_by_side(
        &player_state.field,
        &player_state.seq[0],
        cursor_x,
        rotation,
        &suggestions,
    );

    // 次のツモを表示
    let mut stdout = io::stdout();
    println!("\r\nNext puyos:\r");
    for (i, kumipuyo) in player_state.seq.iter().skip(1).enumerate() {
        let pos_name = match i {
            0 => "Next   ",
            1 => "2nd    ",
            _ => "       ",
        };
        print!("  {}: ", pos_name);

        // 軸ぷよ
        if let Some(term_color) = puyo_color_to_term_color(kumipuyo.axis()) {
            stdout.queue(SetForegroundColor(term_color)).ok();
        }
        print!("{}", color_to_char(kumipuyo.axis()));
        stdout.queue(ResetColor).ok();

        // 子ぷよ
        if let Some(term_color) = puyo_color_to_term_color(kumipuyo.child()) {
            stdout.queue(SetForegroundColor(term_color)).ok();
        }
        print!("{}", color_to_char(kumipuyo.child()));
        stdout.queue(ResetColor).ok();

        println!("\r");
    }

    println!(
        "\r\nPosition: Column {}, Rotation {} ({})\r",
        cursor_x,
        rotation,
        rotation_description(rotation)
    );
}

fn display_field(field: &CoreField) {
    let mut stdout = io::stdout();

    println!("\r\n  1 2 3 4 5 6  \r");
    println!(" ┌─────────────┐\r");

    for y in (1..=13).rev() {
        print!(" │");
        for x in 1..=6 {
            let color = field.color(x, y);
            if let Some(term_color) = puyo_color_to_term_color(color) {
                stdout.queue(SetForegroundColor(term_color)).ok();
            }
            print!("{} ", color_to_char(color));
            stdout.queue(ResetColor).ok();
        }
        println!("│\r");
    }
    println!(" └─────────────┘\r");
    stdout.flush().ok();
}

fn display_field_and_suggestions_side_by_side(
    field: &CoreField,
    kumipuyo: &Kumipuyo,
    cursor_x: usize,
    rotation: usize,
    suggestions: &Vec<(Decision, i32, String)>,
) {
    let mut stdout = io::stdout();

    // ツモ表示の3行（上部）
    // 1行目（子ぷよが上の時のみ使用）
    print!("\r\n ");
    for x in 1..=6 {
        let show_puyo = if x == cursor_x && rotation == 0 {
            Some(kumipuyo.child())
        } else {
            None
        };

        if let Some(color) = show_puyo {
            if let Some(term_color) = puyo_color_to_term_color(color) {
                stdout.queue(SetForegroundColor(term_color)).ok();
            }
            print!(" {}", color_to_char(color));
            stdout.queue(ResetColor).ok();
        } else {
            print!("  ");
        }
    }
    println!("        🤖 AI Suggestions:\r");

    // 2行目（軸ぷよは常にここ、横向きの時は子ぷよも）
    print!(" ");
    for x in 1..=6 {
        let show_puyo = if x == cursor_x {
            Some(kumipuyo.axis())
        } else if x == cursor_x + 1 && rotation == 1 {
            Some(kumipuyo.child())
        } else if cursor_x > 1 && x == cursor_x - 1 && rotation == 3 {
            Some(kumipuyo.child())
        } else {
            None
        };

        if let Some(color) = show_puyo {
            if let Some(term_color) = puyo_color_to_term_color(color) {
                stdout.queue(SetForegroundColor(term_color)).ok();
            }
            print!(" {}", color_to_char(color));
            stdout.queue(ResetColor).ok();
        } else {
            print!("  ");
        }
    }
    if !suggestions.is_empty() && suggestions.len() > 0 {
        let (best_decision, best_eval, _) = &suggestions[0];
        println!("        1st: Col {} Rot {} (Eval: {})\r",
                best_decision.axis_x(),
                best_decision.rot(),
                best_eval);
    } else {
        println!("\r");
    }

    // 3行目（子ぷよが下の時のみ使用）
    print!(" ");
    for x in 1..=6 {
        let show_puyo = if x == cursor_x && rotation == 2 {
            Some(kumipuyo.child())
        } else {
            None
        };

        if let Some(color) = show_puyo {
            if let Some(term_color) = puyo_color_to_term_color(color) {
                stdout.queue(SetForegroundColor(term_color)).ok();
            }
            print!(" {}", color_to_char(color));
            stdout.queue(ResetColor).ok();
        } else {
            print!("  ");
        }
    }
    if suggestions.len() > 1 {
        let (second_decision, second_eval, _) = &suggestions[1];
        println!("        2nd: Col {} Rot {} (Eval: {})\r",
                second_decision.axis_x(),
                second_decision.rot(),
                second_eval);
    } else {
        println!("\r");
    }

    // フィールド表示
    println!("  1 2 3 4 5 6  \r");
    print!(" ┌─────────────┐");
    if suggestions.len() > 2 {
        let (third_decision, third_eval, _) = &suggestions[2];
        println!("      3rd: Col {} Rot {} (Eval: {})\r",
                third_decision.axis_x(),
                third_decision.rot(),
                third_eval);
    } else {
        println!("\r");
    }

    for y in (1..=13).rev() {
        print!(" │");
        for x in 1..=6 {
            let color = field.color(x, y);
            if let Some(term_color) = puyo_color_to_term_color(color) {
                stdout.queue(SetForegroundColor(term_color)).ok();
            }
            print!("{} ", color_to_char(color));
            stdout.queue(ResetColor).ok();
        }
        print!("│");

        // 4行目以降のAIサジェストを表示
        let suggestion_index = 13 - y + 3;
        if suggestion_index < suggestions.len() && suggestion_index <= 5 {
            let (decision, eval, _) = &suggestions[suggestion_index];
            println!("      {}th: Col {} Rot {} (Eval: {})\r",
                    suggestion_index + 1,
                    decision.axis_x(),
                    decision.rot(),
                    eval);
        } else {
            println!("\r");
        }
    }
    println!(" └─────────────┘\r");
    stdout.flush().ok();
}


fn _get_kumipuyo_positions(
    field: &CoreField,
    cursor_x: usize,
    rotation: usize,
) -> (usize, usize, usize, usize) {
    // 軸ぷよの位置を計算
    let axis_y = field.height(cursor_x) as usize + 1;

    // 子ぷよの位置を回転に応じて計算
    let (child_x, child_y) = match rotation {
        0 => (cursor_x, axis_y + 1),      // 縦上
        1 => (cursor_x + 1, axis_y),      // 横右
        2 => (cursor_x, axis_y.saturating_sub(1)), // 縦下
        3 => (cursor_x.saturating_sub(1), axis_y), // 横左
        _ => (cursor_x, axis_y + 1),
    };

    (cursor_x, axis_y, child_x, child_y)
}

fn color_to_char(color: PuyoColor) -> &'static str {
    match color {
        PuyoColor::EMPTY => "·",
        PuyoColor::OJAMA => "○",
        PuyoColor::WALL => "#",
        PuyoColor::IRON => "■",
        PuyoColor::RED => "●",
        PuyoColor::BLUE => "●",
        PuyoColor::YELLOW => "●",
        PuyoColor::GREEN => "●",
    }
}

fn puyo_color_to_term_color(color: PuyoColor) -> Option<Color> {
    match color {
        PuyoColor::RED => Some(Color::Red),
        PuyoColor::BLUE => Some(Color::Blue),
        PuyoColor::YELLOW => Some(Color::Yellow),
        PuyoColor::GREEN => Some(Color::Green),
        PuyoColor::OJAMA => Some(Color::White),
        _ => None,
    }
}

fn show_ai_suggestions(
    ai: &BeamSearchAI,
    player_state: &PlayerState,
    tumo_index: usize,
    suggestions_cache: &mut Option<(usize, Vec<(Decision, i32, String)>)>,
) {
    println!("\r\n🤖 AI Beam Search Suggestions:\r");

    // キャッシュから取得または計算
    let suggestions = if let Some((cached_tumo_index, cached_suggestions)) = suggestions_cache {
        if *cached_tumo_index == tumo_index {
            // キャッシュが有効
            cached_suggestions.clone()
        } else {
            // 新しいツモなので再計算
            let new_suggestions = ai.get_suggestions(player_state.clone());
            *suggestions_cache = Some((tumo_index, new_suggestions.clone()));
            new_suggestions
        }
    } else {
        // 初回計算
        let new_suggestions = ai.get_suggestions(player_state.clone());
        *suggestions_cache = Some((tumo_index, new_suggestions.clone()));
        new_suggestions
    };

    if suggestions.is_empty() {
        println!("No valid moves available!\r");
        return;
    }

    println!("\r\nTop moves by BeamSearch evaluation:\r");
    for (i, (decision, eval, chain_info)) in suggestions.iter().take(10).enumerate() {
        let chain_display = if !chain_info.is_empty() {
            format!(" [{}]", chain_info)
        } else {
            "".to_string()
        };
        println!(
            "  {}. Column {}, Rotation {} ({}) - Eval: {}{}\r",
            i + 1,
            decision.axis_x(),
            decision.rot(),
            rotation_description(decision.rot()),
            eval,
            chain_display
        );
    }

    // AIの最終判断も表示
    let ai_decision = ai.think(player_state.clone(), None, Some(player_state.tumo_index));
    println!("\r\n💡 AI's final recommendation:\r");
    println!(
        "   Column {}, Rotation {} ({}) - {}\r",
        ai_decision.decisions[0].axis_x(),
        ai_decision.decisions[0].rot(),
        rotation_description(ai_decision.decisions[0].rot()),
        ai_decision.log_output
    );
    println!("   (Think time: {} ms)\r", ai_decision.elapsed.as_millis());
}

fn rotation_description(rot: usize) -> &'static str {
    match rot {
        0 => "vertical ↑",
        1 => "horizontal →",
        2 => "vertical ↓",
        3 => "horizontal ←",
        _ => "unknown",
    }
}

fn is_valid_decision(field: &CoreField, kumipuyo: &Kumipuyo, decision: &Decision) -> bool {
    let mut test_field = field.clone();
    test_field.drop_kumipuyo(decision, kumipuyo);

    // 連鎖のシミュレーションを実行
    test_field.simulate();

    // 連鎖後も含めて死んでいなければOK
    !test_field.is_dead() || field.is_dead()
}

// 置いた後（連鎖前）にゲームオーバーになるかチェック
fn will_die_after_drop(field: &CoreField, kumipuyo: &Kumipuyo, decision: &Decision) -> bool {
    let mut test_field = field.clone();
    test_field.drop_kumipuyo(decision, kumipuyo);
    // 連鎖前の状態で死ぬかどうか
    test_field.is_dead()
}

// チェインアニメーション関連の関数
fn create_chain_animation(field: &CoreField) -> ChainAnimation {
    let mut steps = Vec::new();
    let mut work_field = field.clone();
    let mut total_score = 0;
    let mut chain_num = 0;

    // Step 0: ぷよ設置直後の状態
    steps.push(ChainStep {
        field: work_field.clone(),
        _chain_number: 0,
        step_score: 0,
        description: "Puyo dropped - checking for chains...".to_string(),
    });

    // 連鎖をシミュレート
    let before_chain = work_field.clone();
    let result = work_field.simulate();

    if result.chain > 0 {
        // 連鎖が発生した場合、前後の状態を記録
        chain_num = result.chain as usize;
        total_score = result.score;

        // 連鎖消去前の状態（連鎖が起きる直前）
        steps.push(ChainStep {
            field: before_chain.clone(),
            _chain_number: 1,
            step_score: 0,
            description: format!("Chain starting... (Total {} chains detected)", chain_num),
        });

        // 連鎖消去後の最終状態
        steps.push(ChainStep {
            field: work_field.clone(),
            _chain_number: chain_num,
            step_score: total_score,
            description: format!("All chains complete! Score: {} pts", total_score),
        });
    }

    ChainAnimation {
        steps,
        total_chains: chain_num,
        total_score,
    }
}

fn display_chain_animation(
    stdout: &mut io::Stdout,
    animation: &ChainAnimation,
) -> Result<(), std::io::Error> {
    for (i, step) in animation.steps.iter().enumerate() {
        stdout.execute(terminal::Clear(ClearType::All))?;
        stdout.execute(cursor::MoveTo(0, 0))?;

        println!("\r\n{}\r", "=".repeat(40));
        println!("{}\r", step.description);
        println!("{}\r", "=".repeat(40));

        display_field(&step.field);

        if step.step_score > 0 {
            println!("\r\n🎯 Chain Score: {} pts\r", step.step_score);
        }

        // 最初のステップか最後のステップでない場合は、次へ進む前に待機
        if i < animation.steps.len() - 1 {
            println!("\r\nPress any key for next step (q to skip animation)...\r");
            stdout.flush()?;

            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if code == KeyCode::Char('q') {
                    // アニメーションをスキップして最終結果を表示
                    break;
                }
            }
        }
    }

    // 最終的なサマリーを表示
    if animation.total_chains > 0 {
        stdout.execute(terminal::Clear(ClearType::All))?;
        stdout.execute(cursor::MoveTo(0, 0))?;
        println!("\r\n{}\r", "=".repeat(40));
        println!("🎊 Chain Complete!\r");
        println!("{}\r", "=".repeat(40));
        println!("Total Chains: {}\r", animation.total_chains);
        println!("Total Score: {} pts\r", animation.total_score);
        println!("\r\nPress any key to continue...\r");
        stdout.flush()?;
        event::read()?;
    }

    Ok(())
}
