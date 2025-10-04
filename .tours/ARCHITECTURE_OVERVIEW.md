# アーキテクチャ概要

このファイルはプロジェクト全体の構造を俯瞰します。

## クレート構成

```
ghoti (workspace)
├── puyoai/          # コアライブラリのラッパー + ES拡張
├── cpu/             # AI実装と評価関数
├── simulator/       # ゲームシミュレーション
├── logger/          # ログ出力
└── optimizer/       # 評価関数の最適化（遺伝的アルゴリズム）
```

## データフロー（1Pモード）

```
cli_1p (bin)
    ↓
simulate_1p()
    ↓
┌─────────────────┐
│  メインループ    │
│  (各ツモ処理)    │
└─────────────────┘
    ↓
PlayerState::set_seq() ← HaipuyoDetector (配ぷよ管理)
    ↓
AI::think()
    ↓
┌──────────────────────────┐
│ BeamSearchAI             │
│  ├── OpeningMatcher      │ (序盤5手)
│  └── think_internal()    │
│       ↓                  │
│   並列スレッド × 20       │
│       ↓                  │
│   think_single_thread()  │
│       ↓                  │
│   ビームサーチループ      │
│       ↓                  │
│   generate_next_states() │
│       ↓                  │
│   Plan::iterate_...()    │
│       ↓                  │
│   Evaluator::evaluate()  │
└──────────────────────────┘
    ↓
AIDecision (手を返す)
    ↓
PlayerState::drop_kumipuyo()
    ↓
CoreField::simulate()  ← EsBitField トレイト
    ↓
RensaResult (連鎖結果)
    ↓
ログ出力 & 次ツモへ
    ↓
SimulateResult1P
    ↓
JSON出力 (kifus/)
    ↓
Next.js フロントエンド (視覚化)
```

## 主要な型の関係

```
PlayerState {
    field: CoreField,      // 盤面
    seq: Vec<Kumipuyo>,    // ツモ列
    score: usize,          // 得点
    ...
}

CoreField  (trait)
    ├── BitField        (AVX2最適化実装)
    ├── PlainField      (シンプル実装)
    └── implements EsBitField  (ES拡張)

Kumipuyo {
    axis: PuyoColor,    // 軸ぷよの色
    child: PuyoColor,   // 子ぷよの色
}

Decision {
    axis_x: usize,      // 1-6
    rot: usize,         // 0-3 (上右下左)
}

Plan {
    field: CoreField,      // 手を打った後の盤面
    decisions: Vec<...>,   // ここまでの手順
    rensa_result: ...,     // 連鎖結果
    ...
}
```

## ビームサーチの詳細

```
State {
    field: CoreField,           // 現在の盤面
    decisions: Vec<Decision>,   // ここまでの手順
    eval_score: i32,            // 評価値 ★重要★
    plan: Option<Plan>,
    ...
}

ビームサーチループ:
for depth in 0..20 {
    現在のビーム: Vec<State> (最大width=140個)
    ↓
    各Stateに対して:
        全合法手（22通り）を試す
        ↓
        Plan生成 & Evaluator::evaluate()
        ↓
        新しいStateを作成
    ↓
    全ての新State (最大 140×22 = 3080個)
    ↓
    eval_scoreでソート
    ↓
    上位140個だけ残す ← ビームの枝刈り
    ↓
    次の深さへ
}
```

## 評価関数の構造

```
Evaluator::evaluate(plan: &Plan) -> i32

計算内容:
  score = 0

  // 1. 盤面形状 (10項目以上)
  score += valley × 谷の深さ
  score += ridge × 尾根の高さ
  score += ideal_height_diff × 理想形とのズレ
  ...

  // 2. 連結 (2項目)
  score += connectivity_2 × 2連結の数
  score += connectivity_3 × 3連結の数

  // 3. 連鎖ポテンシャル (8項目)
  score += potential_main_chain × 本線連鎖数
  score += potential_sub_chain × 副砲連鎖数
  ...

  // 4. パターンマッチ (30項目以上)
  score += gtr_base_1 × GTR土台1の検出
  score += gtr_tail_1_1 × GTR尾1-1の検出
  ...

  return score
```

## 並列化の仕組み

```rust
// モンテカルロ的並列化
for _ in 0..20 {
    thread::spawn(|| {
        // 見えないツモをランダム補完
        let extended_seq = seq + random_puyos();

        // ビームサーチ実行
        let result = think_single_thread(...);

        // チャネルで送信
        tx.send(result);
    });
}

// 結果を集計
scores[x][rot] で多数決
   ↓
最頻の手を採用
```

## 発火判定のロジック

```
fire_condition(state, opponent) -> bool

1. 序盤全消し? → true
2. 相手が連鎖中?
   - 割り込める? → true
   - 間に合わない? → false
3. 潰し条件?
   - 相手が平ら
   - 2列以上送れる
   → true
4. おじゃま相殺必要?
   - 3列目が埋まりそう? → 相殺分だけ打つ
   - それ以外 → 全相殺
5. 先打ち?
   - 本線8万点以上
   - 相手より優位
   → true
6. 飽和?
   - 8万点以上 → true

default: false (まだ打たない)
```

## ES (Esports) 仕様の違い

通常のぷよぷよとEsportsモードの主な違い：

1. **フレーム計算**
   - `es_frame::FRAMES_CHAIN[max_drops]`
   - 落下距離に応じたフレーム数テーブル

2. **連鎖ボーナス**
   - 連鎖倍率が異なる
   - 全消しボーナスの計算

3. **おじゃまぷよレート**
   - 70点で1個（`OJAMA_PUYO_RATE = 70`）

## puyoai-core との関係

```
puyoai-core (外部ライブラリ)
    ↓ 依存
puyoai (このプロジェクトのクレート)
    ├── pub use puyoai_core::*  (再エクスポート)
    ├── es_field (ES拡張)
    ├── es_frame (ES拡張)
    └── plan (追加機能)
    ↓ 依存
cpu, simulator, logger, optimizer
```

`puyoai-core` は別プロジェクトで開発されている汎用ライブラリ。
このプロジェクトではそれをラップして、Esports仕様の拡張を追加しています。

## パフォーマンス最適化ポイント

1. **AVX2/BMI2命令**
   - BitFieldでの並列ビット演算
   - 消去判定、落下処理が高速化

2. **ビーム幅の調整**
   - 序盤: width=20 (軽量)
   - 終盤: width=140 (重量)

3. **序盤テンプレート**
   - 最初の5手はパターンマッチで即決
   - 探索コストを削減

4. **並列探索**
   - 20スレッドで同時実行
   - マルチコアCPUを活用

5. **評価関数の軽量化**
   - 線形モデル（重み付き和）
   - O(field_size) で計算可能

## 競技プログラミング的な観点

このプロジェクトで使われているアルゴリズム：

- **ビームサーチ**: 貪欲法 + 幅優先探索
- **評価関数**: 機械学習の線形モデル
- **遺伝的アルゴリズム**: パラメータ最適化
- **優先度付きキュー**: 2P対戦のイベント処理
- **ビット演算**: 盤面の高速化
- **動的計画法的枝刈り**: 状態空間の削減

典型的な探索問題として、AtCoderのAHC（ヒューリスティックコンテスト）と似た構造です。
