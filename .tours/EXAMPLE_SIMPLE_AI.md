# シンプルなAIの実装例

このファイルはCodeTourの補足として、実際にオリジナルAIを作る例を示します。

## 例1: 常に3列目に置くAI

```rust
// cpu/src/bot/simple_ai.rs
use std::time::Instant;
use crate::bot::*;
use puyoai::decision::Decision;

pub struct SimpleAI;

impl AI for SimpleAI {
    fn new() -> Self {
        SimpleAI
    }

    fn name(&self) -> &'static str {
        "SimpleAI"
    }

    fn think(
        &self,
        player_state_1p: PlayerState,
        _player_state_2p: Option<PlayerState>,
        _think_frame: Option<usize>,
    ) -> AIDecision {
        let start = Instant::now();

        // 常に3列目に縦置き
        let decision = Decision::new(3, 0);

        AIDecision::from_decision(
            &decision,
            "always column 3".to_string(),
            start.elapsed(),
        )
    }
}
```

## 例2: 3列目の高さを避けるAI

```rust
use puyoai::field::CoreField;

pub struct AvoidHighAI;

impl AI for AvoidHighAI {
    fn new() -> Self {
        AvoidHighAI
    }

    fn name(&self) -> &'static str {
        "AvoidHighAI"
    }

    fn think(
        &self,
        player_state_1p: PlayerState,
        _player_state_2p: Option<PlayerState>,
        _think_frame: Option<usize>,
    ) -> AIDecision {
        let start = Instant::now();
        let field = &player_state_1p.field;

        // 最も低い列を探す
        let mut best_x = 1;
        let mut min_height = field.height(1);

        for x in 2..=6 {
            let h = field.height(x);
            if h < min_height {
                min_height = h;
                best_x = x;
            }
        }

        // 最も低い列に縦置き
        let decision = Decision::new(best_x, 0);

        AIDecision::from_decision(
            &decision,
            format!("column {} (height {})", best_x, min_height),
            start.elapsed(),
        )
    }
}
```

## 例3: BeamSearchAIをカスタマイズ

既存のBeamSearchAIの評価関数だけを変更する例：

```rust
use cpu::evaluator::Evaluator;
use cpu::bot::BeamSearchAI;

// カスタム評価関数を作成
let mut custom_evaluator = Evaluator::default();
custom_evaluator.valley = -500;  // 谷をより嫌う
custom_evaluator.connectivity_3 = 500;  // 3連結を重視

// カスタム評価関数でAIを初期化
let custom_ai = BeamSearchAI::new_customize(custom_evaluator);
```

## 登録方法

1. `cpu/src/bot.rs` に追加:
```rust
pub mod simple_ai;
pub use simple_ai::*;
```

2. `simulator/src/bin/cli_1p.rs` の `ais` ベクトルに追加:
```rust
let ais: Vec<Box<dyn AI>> = vec![
    Box::new(BeamSearchAI::new()),
    Box::new(RandomAI::new()),
    Box::new(SimpleAI::new()),  // 追加
];
```

3. 実行:
```sh
cargo run --release -p ghoti-simulator --bin cli_1p -- --ai SimpleAI
```

## よく使うメソッド

### CoreFieldのメソッド
```rust
field.height(x)                    // x列の高さ
field.color(x, y)                  // (x,y)のぷよの色
field.is_empty(x, y)               // 空か？
field.is_dead()                    // 死んでいるか？
field.simulate()                   // 連鎖シミュレーション
field.count_connected(x, y)        // (x,y)から連結している個数
field.drop_kumipuyo(decision, tumo) // ぷよを置く
```

### Decisionの生成
```rust
Decision::new(x, rot)              // 手を作成
Decision::all_valid_decisions()    // 全合法手（22通り）
decision.axis_x()                  // 軸ぷよのx座標
decision.rot()                     // 回転（0=上, 1=右, 2=下, 3=左）
```

### Plan（手の計画）
```rust
Plan::iterate_available_plans(field, seq, depth, |plan| {
    // 各合法手に対する処理
    let chain = plan.chain();      // 連鎖数
    let score = plan.score();      // 得点
    let field_after = plan.field(); // 手を打った後の盤面
});
```
