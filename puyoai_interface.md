# puyoai-core ライブラリ インターフェースガイド

このドキュメントは、`puyoai-core`ライブラリの主要なインターフェースをまとめたものです。

## 1. 色（Color）関連

### PuyoColor 列挙型
```rust
pub enum PuyoColor {
    EMPTY = 0,
    OJAMA = 1,
    WALL = 2,
    IRON = 3,
    RED = 4,
    BLUE = 5,
    YELLOW = 6,
    GREEN = 7
}
```

**主要メソッド:**
- `from_byte(b: u8) -> PuyoColor` - バイトから色を生成
- `to_char() -> char` - 文字表現に変換（'R', 'B', 'Y', 'G' など）
- `is_normal_color() -> bool` - 通常色（RED/BLUE/YELLOW/GREEN）かチェック

**使用例:**
```rust
let color = PuyoColor::RED;
assert!(color.is_normal_color());
```

## 2. フィールド関連

### 定数
```rust
pub const WIDTH: usize = 6;       // ゲームフィールドの幅
pub const HEIGHT: usize = 12;     // ゲームフィールドの高さ
pub const MAP_WIDTH: usize = 8;   // 内部表現の幅（壁含む）
pub const MAP_HEIGHT: usize = 16; // 内部表現の高さ
```

### CoreField 構造体
最も基本的なフィールド型。内部で`BitField`を使用。

**主要メソッド:**
- `new() -> CoreField` - 空のフィールドを生成
- `from_str(s: &str) -> CoreField` - 文字列からフィールドを生成
- `color(x: usize, y: usize) -> PuyoColor` - 指定位置の色を取得（yは1-based）
- `height(x: usize) -> usize` - 指定列の高さを取得（xは1-based）
- `is_empty(x: usize, y: usize) -> bool` - 指定位置が空か
- `is_dead() -> bool` - ゲームオーバーか（12段目以上にぷよがある）
- `simulate() -> RensaResult` - 連鎖シミュレーション実行

**単一ぷよを配置する方法:**
```rust
// CoreFieldでは直接単一ぷよを置くメソッドはない
// PlainFieldを経由する必要がある
```

### PlainField 構造体
2次元配列によるシンプルなフィールド表現。単一ぷよの配置が可能。

```rust
use puyoai::field::plain_field::PlainField;
use puyoai::color::PuyoColor;

// 新規作成
let mut pf = PlainField::<PuyoColor>::new();

// 単一ぷよを配置
pf.set_color(x, y, PuyoColor::RED);  // x: 1-6, y: 1-12

// 高さを計算（内部的なメソッド）
let mut heights = [0u16; MAP_WIDTH];
pf.calculate_height(&mut heights);
let height = heights[x] as usize;

// CoreFieldに変換
// 注意: 直接変換メソッドは提供されていない
// BitField経由で変換する
```

### フィールド間の変換

```rust
// PlainField -> BitField -> CoreField の変換
let mut pf = PlainField::<PuyoColor>::new();
// ... pf にぷよを配置 ...

// PlainField -> BitField
let bf = BitField::from_plain_field(&pf);

// BitField -> CoreField
let cf = CoreField::from_bit_field(&bf);

// CoreField -> PlainField (直接変換)
// 各座標を手動でコピーする必要がある
let mut pf2 = PlainField::<PuyoColor>::new();
for x in 1..=6 {
    for y in 1..=cf.height(x) {
        pf2.set_color(x, y, cf.color(x, y as i16));
    }
}
```

## 3. 組ぷよと配置

### Kumipuyo（組ぷよ）
```rust
use puyoai::kumipuyo::Kumipuyo;

let kumipuyo = Kumipuyo::new(PuyoColor::RED, PuyoColor::BLUE);
assert_eq!(kumipuyo.axis(), PuyoColor::RED);   // 軸ぷよ
assert_eq!(kumipuyo.child(), PuyoColor::BLUE); // 子ぷよ
```

### Decision（配置決定）
```rust
use puyoai::decision::Decision;

// x=3, 回転0（子ぷよが上）
let decision = Decision::new(3, 0);

// 回転の意味:
// 0: 子ぷよが上
// 1: 子ぷよが右
// 2: 子ぷよが下
// 3: 子ぷよが左

// 全有効配置（22通り）
let all_decisions = Decision::all_valid_decisions();
```

## 4. 連鎖シミュレーション

### RensaResult
```rust
pub struct RensaResult {
    pub chain: usize,   // 連鎖数
    pub score: usize,   // 得点
    pub frame: usize,   // フレーム数
    pub quick: bool     // クイック消しか
}
```

### シミュレーション実行
```rust
let mut field = CoreField::from_str(concat!(
    "......",
    "......",
    "RRRR..",  // 4つ揃っている
));

let result = field.simulate();
println!("連鎖数: {}, 得点: {}", result.chain, result.score);
```

### EsCoreField (拡張版)
```rust
use puyoai::es_field::EsCoreField;

// es_simulate()はトレイトメソッドとして提供
let mut field = CoreField::new();
let result = field.es_simulate();
```

## 5. Plan（拡張機能）

```rust
use puyoai::plan::Plan;

// 利用可能なプランを反復処理
Plan::iterate_available_plans(
    &field,           // 現在のフィールド
    &seq,            // Kumipuyoのシーケンス
    max_depth,       // 最大探索深さ
    &mut |plan| {    // 各プランに対する処理
        let decision = plan.first_decision();
        let score = plan.score();
        let chain = plan.chain();
        let field_after = plan.field();
    }
);
```

## 6. 実践例：単一ぷよでの連鎖ポテンシャル評価

```rust
use puyoai::field::plain_field::PlainField;
use puyoai::field::CoreField;
use puyoai::color::PuyoColor;

fn evaluate_single_puyo_potential(field: &CoreField) -> i32 {
    let colors = [PuyoColor::RED, PuyoColor::BLUE,
                  PuyoColor::YELLOW, PuyoColor::GREEN];
    let mut max_score = 0;

    for x in 1..=6 {
        if field.height(x) >= 12 {
            continue;
        }

        for &color in &colors {
            // PlainFieldにコピー
            let mut pf = PlainField::<PuyoColor>::new();
            for cx in 1..=6 {
                for cy in 1..=field.height(cx) {
                    pf.set_color(cx, cy, field.color(cx, cy as i16));
                }
            }

            // 単一ぷよを配置
            let height = field.height(x);
            pf.set_color(x, height + 1, color);

            // BitField経由でCoreFieldに変換
            let bf = BitField::from_plain_field(&pf);
            let mut test_field = CoreField::from_bit_field(&bf);

            // シミュレーション
            let result = test_field.es_simulate();
            if result.score > max_score {
                max_score = result.score as i32;
            }
        }
    }

    max_score
}
```

## 7. 注意事項

### 座標系
- x座標: 1-6（左から右）
- y座標: 1-12（下から上）
- 内部表現は0-basedの場合があるので注意

### パフォーマンス
- `BitField`: ビット演算による高速処理
- `CoreField`: 最も実用的、高さ情報も保持
- `PlainField`: シンプルだが遅い、単一ぷよ配置が可能

### メモリ管理
- `CoreField`と`PlainField`は`Clone`可能
- シミュレーションは`&mut self`を取るので注意

## 8. よくあるパターン

### パターン1: フィールドの文字列表現
```rust
// 上から下への文字列（最上段が最初）
let field = CoreField::from_str(concat!(
    "......",  // 12段目
    "......",  // 11段目
    "RRBYYG",  // 1段目
));
```

### パターン2: 連鎖の事前チェック
```rust
fn will_chain(field: &CoreField, x: usize, color: PuyoColor) -> bool {
    let mut test_field = field.clone();
    // ... ぷよを配置 ...
    let result = test_field.simulate();
    result.chain > 0
}
```

### パターン3: ビームサーチでの使用
```rust
use puyoai::plan::Plan;

let kumipuyo = Kumipuyo::new(PuyoColor::RED, PuyoColor::BLUE);
let seq = vec![kumipuyo];

Plan::iterate_available_plans(&field, &seq, 1, &mut |plan| {
    let eval_score = evaluate(plan);
    // ... 評価値に基づいて処理 ...
});
```