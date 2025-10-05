/// puyop.com URL のエンコード/デコード往復テスト
///
/// make_puyop_url() -> decode_url() -> 盤面比較

use ghoti_simulator::puyop_decoder::PuyopDecoder;
use puyoai::{
    color::PuyoColor,
    decision::Decision,
    field::CoreField,
    kumipuyo::Kumipuyo,
    puyop::make_puyop_url,
};

fn main() {
    println!("=== puyop.com URL 往復変換テスト ===\n");

    // テスト1: 元のテストケース
    test_original_case();

    // テスト2: 空の盤面
    test_empty_field();

    // テスト3: ツモと操作付き
    test_with_control();

    println!("\n=== すべてのテスト完了 ===");
}

fn test_original_case() {
    println!("【テスト1】元のテストケース");

    // puyoai-core のテストケースと同じ盤面
    let original_field = CoreField::from_str(concat!(
        ".....Y",
        ".G..YY",
        "RGRRBB",
        "RRGRGB",
    ));

    // エンコード
    let url = make_puyop_url(&original_field, &[], &[]);
    println!("  エンコード結果: {}", url);
    println!("  期待値: http://www.puyop.com/s/420Aa9r9hj");

    // デコード
    let decoder = PuyopDecoder::new();
    let (decoded_field, decoded_seq, decoded_decisions) = decoder.decode_url(&url).unwrap();

    println!("  デコードされたツモ数: {}", decoded_seq.len());
    println!("  デコードされた操作数: {}", decoded_decisions.len());

    // 盤面比較
    println!("\n  【盤面比較】");
    print_field_comparison(&original_field, &decoded_field);

    // 完全一致チェック
    let matches = check_field_equality(&original_field, &decoded_field);
    if matches {
        println!("  ✅ 盤面が完全一致しました！");
    } else {
        println!("  ❌ 盤面が一致しません");
    }

    println!();
}

fn test_empty_field() {
    println!("【テスト2】空の盤面");

    let original_field = CoreField::new();

    let url = make_puyop_url(&original_field, &[], &[]);
    println!("  エンコード結果: {}", url);

    let decoder = PuyopDecoder::new();
    let (decoded_field, _, _) = decoder.decode_url(&url).unwrap();

    let matches = check_field_equality(&original_field, &decoded_field);
    if matches {
        println!("  ✅ 空の盤面が一致しました！");
    } else {
        println!("  ❌ 空の盤面が一致しません");
    }

    println!();
}

fn test_with_control() {
    println!("【テスト3】ツモと操作付き");

    let original_field = CoreField::new();
    let seq = vec![
        Kumipuyo::new(PuyoColor::RED, PuyoColor::RED),
        Kumipuyo::new(PuyoColor::BLUE, PuyoColor::YELLOW),
        Kumipuyo::new(PuyoColor::GREEN, PuyoColor::GREEN),
    ];
    let decisions = vec![
        Decision::new(3, 0),  // 3列目に縦置き
        Decision::new(4, 1),  // 4列目に右向き
        Decision::new(2, 2),  // 2列目に下向き
    ];

    let url = make_puyop_url(&original_field, &seq, &decisions);
    println!("  エンコード結果: {}", url);

    let decoder = PuyopDecoder::new();
    let (decoded_field, decoded_seq, decoded_decisions) = decoder.decode_url(&url).unwrap();

    println!("  元のツモ数: {}", seq.len());
    println!("  デコードされたツモ数: {}", decoded_seq.len());
    println!("  元の操作数: {}", decisions.len());
    println!("  デコードされた操作数: {}", decoded_decisions.len());

    // ツモ比較
    println!("\n  【ツモ比較】");
    let tumo_matches = check_tumo_equality(&seq, &decoded_seq);
    if tumo_matches {
        println!("  ✅ ツモが一致しました！");
    } else {
        println!("  ❌ ツモが一致しません");
        for (i, (orig, dec)) in seq.iter().zip(decoded_seq.iter()).enumerate() {
            println!("    {}手目: ({:?},{:?}) vs ({:?},{:?})",
                i + 1,
                orig.axis(), orig.child(),
                dec.axis(), dec.child()
            );
        }
    }

    // 操作比較
    println!("\n  【操作比較】");
    let decision_matches = check_decision_equality(&decisions, &decoded_decisions);
    if decision_matches {
        println!("  ✅ 操作が一致しました！");
    } else {
        println!("  ❌ 操作が一致しません");
        for (i, (orig, dec)) in decisions.iter().zip(decoded_decisions.iter()).enumerate() {
            println!("    {}手目: ({},{}) vs ({},{})",
                i + 1,
                orig.axis_x(), orig.rot(),
                dec.axis_x(), dec.rot()
            );
        }
    }

    println!();
}

fn print_field_comparison(original: &CoreField, decoded: &CoreField) {
    println!("  元の盤面:");
    print_field(original, "    ");

    println!("  デコード後の盤面:");
    print_field(decoded, "    ");
}

fn print_field(field: &CoreField, indent: &str) {
    for y in (1..=13).rev() {
        print!("{}", indent);
        for x in 1..=6 {
            let c = match field.color(x, y) {
                PuyoColor::RED => 'R',
                PuyoColor::GREEN => 'G',
                PuyoColor::BLUE => 'B',
                PuyoColor::YELLOW => 'Y',
                PuyoColor::OJAMA => 'O',
                _ => '.',
            };
            print!("{}", c);
        }
        println!();
    }
}

fn check_field_equality(f1: &CoreField, f2: &CoreField) -> bool {
    for y in 1..=13 {
        for x in 1..=6 {
            if f1.color(x, y) != f2.color(x, y) {
                return false;
            }
        }
    }
    true
}

fn check_tumo_equality(s1: &[Kumipuyo], s2: &[Kumipuyo]) -> bool {
    if s1.len() != s2.len() {
        return false;
    }
    for (k1, k2) in s1.iter().zip(s2.iter()) {
        if k1.axis() != k2.axis() || k1.child() != k2.child() {
            return false;
        }
    }
    true
}

fn check_decision_equality(d1: &[Decision], d2: &[Decision]) -> bool {
    if d1.len() != d2.len() {
        return false;
    }
    for (dec1, dec2) in d1.iter().zip(d2.iter()) {
        if dec1.axis_x() != dec2.axis_x() || dec1.rot() != dec2.rot() {
            return false;
        }
    }
    true
}
