# 盤面解析ツールの使い方

## 概要

`analyze_position` は特定の盤面から最善手・次善手を解析するCLIツールです。

## インストール

```sh
cargo build --release -p ghoti-simulator --bin analyze_position
```

## 基本的な使い方

### 1. ツモのみ指定（空の盤面から）

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --tumos "RR,BY,GG,RY,GB" \
  --top-n 10
```

出力例：
```
=== Top 10 Moves ===

1位: 列3 回転↓(下) - 評価値: 572
2位: 列3 回転→(右) - 評価値: 441
3位: 列2 回転←(左) - 評価値: 124
...
```

### 2. URLから解析（簡易版）

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --url "?tumos=RR,BY,GG,RY,GB&ops=3-0,4-1" \
  --top-n 5
```

### 3. 詳細表示モード

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --tumos "RR,BY,GG" \
  --top-n 3 \
  --verbose
```

出力に連鎖情報が追加されます：
```
1位: 列3 回転↑(上) - 評価値: 572
    連鎖: 0連鎖, 得点: 0
    ツモ: 🔴🔴 → 列3
```

### 4. 読み深さを変更

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --tumos "RR,BY,GG,RY" \
  --depth 2 \
  --top-n 5
```

## オプション一覧

| オプション | 説明 | デフォルト値 | 例 |
|-----------|------|------------|-----|
| `--tumos` | ツモ文字列（カンマ区切り） | `RR,BY,GG,RY,GB` | `--tumos "RR,GG,BY"` |
| `--url` | puyop.com形式URL（簡易版） | なし | `--url "?tumos=RR,BY"` |
| `--field` | フィールド文字列（78文字） | 空 | （未実装） |
| `--top-n` | 上位何手を表示するか | `10` | `--top-n 5` |
| `--depth` | AI読み深さ | `1` | `--depth 3` |
| `--verbose` | 詳細表示 | なし | `--verbose` |

## URL形式

簡易版のURL形式は以下の通りです：

```
?tumos=RR,BY,GG&ops=3-0,4-1
```

パラメータ：
- `tumos`: ツモ（例：`RR,BY,GG`）
- `ops`: 操作履歴（例：`3-0,4-1` = 3列目に縦置き、4列目に右向き）
- `field`: 盤面（現在未実装）

## 実装の制約

### ⚠️ 盤面指定の制約

現在、CoreFieldに直接ぷよを配置するpublic APIが存在しないため、**盤面の指定は未実装**です。

以下の方法で回避可能：

#### 方法1: コードで直接構築

`analyze_position.rs` を編集して盤面を構築：

```rust
// analyze_position.rs の main() 内
let mut field = CoreField::new();

// ぷよを1つずつ配置
use puyoai::color::PuyoColor;
use puyoai::kumipuyo::Kumipuyo;
use puyoai::decision::Decision;

field.drop_kumipuyo(
    &Decision::new(1, 0),  // 1列目に縦置き
    &Kumipuyo::new(PuyoColor::RED, PuyoColor::RED)
);
field.drop_kumipuyo(
    &Decision::new(1, 0),
    &Kumipuyo::new(PuyoColor::BLUE, PuyoColor::BLUE)
);
// ...
```

#### 方法2: シミュレーターから保存

既存のシミュレーターで対局し、特定の局面をJSON出力してから、そこから盤面を読み込む。

#### 方法3: puyoai-coreの内部APIを使う

`BitField::from_str()` を使って構築し、unsafe等で変換する（上級者向け）。

## 出力の読み方

### 回転の意味

- `↑(上)`: rot=0（子ぷよが上）
- `→(右)`: rot=1（子ぷよが右）
- `↓(下)`: rot=2（子ぷよが下）
- `←(左)`: rot=3（子ぷよが左）

### 評価値

評価値が大きいほど良い盤面です。評価関数（`Evaluator`）が以下を考慮：

- 盤面の形（谷・尾根・高さ）
- 連結数（2連結・3連結）
- 連鎖ポテンシャル（本線・副砲）
- パターンマッチ（GTR等）
- フレーム効率

## puyop.com連携（将来実装予定）

本来のpuyop.comのURLは複雑なエンコーディングを使用しているため、現在は簡易版のみ対応。

将来的には以下のような実装が必要：
1. puyop.comのURL形式をデコード
2. 盤面情報を抽出
3. CoreFieldに変換

参考: `puyoai-core` の `puyop::make_puyop_url()` の逆変換を実装

## トラブルシューティング

### Q: 盤面を指定したい

A: 現在は未実装です。上記「実装の制約」を参照してください。

### Q: 評価値がマイナスになる

A: 評価関数の仕様です。悪い形（谷・尾根が多い等）はマイナス評価になります。

### Q: 発火候補が表示されない

A: ツモ列が短い場合、連鎖が組めません。`--depth` を増やすか、ツモを追加してください。

### Q: URLパースに失敗する

A: 現在の実装は簡易版です。`?tumos=...` 形式のみ対応しています。

## 応用例

### 1. 特定局面の全候補を比較

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --tumos "RY,GB,RR,BY,GG,YY,BR" \
  --depth 2 \
  --top-n 22 \  # 全22候補
  --verbose > analysis.txt
```

### 2. スクリプト化

```sh
#!/bin/bash
for tumo in "RR,BY" "GG,RY" "BR,YG"; do
  echo "=== Tumo: $tumo ==="
  cargo run --release -p ghoti-simulator --bin analyze_position -- \
    --tumos "$tumo" --top-n 3
  echo ""
done
```

### 3. 評価関数のテスト

```sh
# 同じツモで異なる深さを比較
for depth in 1 2 3 4; do
  echo "=== Depth: $depth ==="
  cargo run --release -p ghoti-simulator --bin analyze_position -- \
    --tumos "RR,BY,GG" --depth $depth --top-n 1
done
```

## まとめ

現時点では**ツモ列からの最善手解析**が主な用途です。

盤面指定機能は将来の拡張として、以下が必要：
1. `puyoai-core` のpublic API拡張
2. puyop.com URL完全対応
3. 独自の盤面フォーマット定義

それまでは、空の盤面からのツモ解析や、コード内で盤面を構築する方法をお使いください。
