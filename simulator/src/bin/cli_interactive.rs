use std::io::{self, Write};

use cpu::bot::{BeamSearchAI, PlayerState, AI};
use puyoai::{
    color::PuyoColor,
    decision::Decision,
    field::CoreField,
    kumipuyo::Kumipuyo,
};

use ghoti_simulator::haipuyo_detector::*;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
    ExecutableCommand, QueueableCommand,
};

fn main() -> Result<(), std::io::Error> {
    // ターミナルをrawモードに設定
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();

    let result = run_game(&mut stdout);

    // rawモードを解除
    terminal::disable_raw_mode()?;
    stdout.execute(cursor::Show)?;
    println!();

    result
}

fn run_game(stdout: &mut io::Stdout) -> Result<(), std::io::Error> {
    stdout.execute(terminal::Clear(ClearType::All))?;
    stdout.execute(cursor::MoveTo(0, 0))?;

    println!("=== Puyo Puyo Interactive Mode ===\r");
    println!("Controls:\r");
    println!("  a/d       : Move left/right\r");
    println!("  s/Space   : Hard drop\r");
    println!("  j/k       : Rotate left/right\r");
    println!("  h         : Show AI suggestions\r");
    println!("  q         : Exit game\r");
    println!("\r");
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

    // ランダムな配ぷよを生成
    let seq = HaipuyoDetector::random_haipuyo();
    let mut player_state = PlayerState::initial_state(vec![], Some(seq.clone()));
    let mut score = 0;
    let mut tumo_index = 0;

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

            // 盤面とカーソル位置を表示
            display_game_state_with_cursor(&player_state, score, tumo_index, x, r);
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
                        show_ai_suggestions(&ai, &player_state);
                        println!("\r\nPress any key to continue...\r");
                        stdout.flush()?;
                        event::read()?;
                        continue;
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

                        // ぷよを落とす
                        player_state.drop_kumipuyo(&decision);
                        let rensa_result = player_state.field.simulate();
                        score += rensa_result.score;

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

fn display_game_state(player_state: &PlayerState, score: usize, tumo_index: usize) {
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

fn display_game_state_with_cursor(
    player_state: &PlayerState,
    score: usize,
    tumo_index: usize,
    cursor_x: usize,
    rotation: usize,
) {
    println!("\r\n{}\r", "=".repeat(40));
    println!("Turn: {}  Score: {}\r", tumo_index + 1, score);
    println!("{}\r", "=".repeat(40));

    // 盤面とカーソル位置を表示
    display_field_with_cursor(&player_state.field, &player_state.seq[0], cursor_x, rotation);

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

fn display_field_with_cursor(
    field: &CoreField,
    kumipuyo: &Kumipuyo,
    cursor_x: usize,
    rotation: usize,
) {
    let mut stdout = io::stdout();

    // ツモを最上段（フィールド上部）に表示
    // 軸ぷよは常に2行目の cursor_x 列に固定
    // 子ぷよだけが軸ぷよの周りを回転（上、右、下、左）

    // 1行目（子ぷよが上の時のみ使用）
    print!("\r\n ");
    for x in 1..=6 {
        let show_puyo = if x == cursor_x && rotation == 0 {
            Some(kumipuyo.child()) // 回転0: 子ぷよが上
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
    println!("\r");

    // 2行目（軸ぷよは常にここ、横向きの時は子ぷよも）
    print!(" ");
    for x in 1..=6 {
        let show_puyo = if x == cursor_x {
            Some(kumipuyo.axis()) // 軸ぷよは常にここ
        } else if x == cursor_x + 1 && rotation == 1 {
            Some(kumipuyo.child()) // 回転1: 子ぷよが右
        } else if cursor_x > 1 && x == cursor_x - 1 && rotation == 3 {
            Some(kumipuyo.child()) // 回転3: 子ぷよが左
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
    println!("\r");

    // 3行目（子ぷよが下の時のみ使用）
    print!(" ");
    for x in 1..=6 {
        let show_puyo = if x == cursor_x && rotation == 2 {
            Some(kumipuyo.child()) // 回転2: 子ぷよが下
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
    println!("\r");

    println!("  1 2 3 4 5 6  \r");
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

fn get_kumipuyo_positions(
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

fn show_ai_suggestions(ai: &BeamSearchAI, player_state: &PlayerState) {
    println!("\r\n🤖 AI Beam Search Suggestions:\r");

    // BeamSearchAIで候補手を取得
    let suggestions = ai.get_suggestions(player_state.clone());

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
    !test_field.is_dead() || field.is_dead()
}
