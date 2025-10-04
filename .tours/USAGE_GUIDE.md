# シミュレーター使用ガイド

## 基本的な使い方

### 1人プレイ（とこぷよ）

```sh
# デフォルト設定で実行
cargo run --release -p ghoti-simulator --bin cli_1p

# 手数を指定
cargo run --release -p ghoti-simulator --bin cli_1p -- --max_tumos 50

# AI読み手数を変更（デフォルト2手）
cargo run --release -p ghoti-simulator --bin cli_1p -- --visible_tumos 5

# 特定の配ぷよで実行（0-16777215）
cargo run --release -p ghoti-simulator --bin cli_1p -- --haipuyo_margin 12345

# 目標得点を設定（この得点以上で終了）
cargo run --release -p ghoti-simulator --bin cli_1p -- --required_chain_score 50000

# AIを変更
cargo run --release -p ghoti-simulator --bin cli_1p -- --ai RandomAI

# 複数回実行
cargo run --release -p ghoti-simulator --bin cli_1p -- --trial 10
```

### 2人対戦

```sh
# デフォルト（BeamSearchAI vs BeamSearchAI）
cargo run --release -p ghoti-simulator --bin cli_2p

# 勝利条件を変更（デフォルト1本先取）
cargo run --release -p ghoti-simulator --bin cli_2p -- --win_goal 3

# AIを指定
cargo run --release -p ghoti-simulator --bin cli_2p -- \
  --ai_1p BeamSearchAI \
  --ai_2p RandomAI
```

### リプレイ再生

```sh
# Next.jsサーバーを起動
cd nextjs
yarn dev

# ブラウザで http://localhost:3000 にアクセス
# kifus/ ディレクトリのJSONファイルが表示されます
```

## ログの見方

```
  1. RR (3, 0) [ 123 ms]      70 (+    70) | eval:   1234
  2. YG (4, 1) [ 145 ms]     140 (+    70) | eval:   2345
  3. BB (2, 0) [  98 ms]   4680 (+  4540) | fire:   4540
```

各列の意味：
- `1.`: ツモ番号
- `RR`: ツモ（軸色+子色）
- `(3, 0)`: 配置（x座標, 回転）
- `[ 123 ms]`: 思考時間
- `70`: 累計スコア
- `(+70)`: このツモで獲得した得点
- `eval: 1234` または `fire: 4540`: AIのログ出力
  - `eval`: 評価値
  - `fire`: 発火（連鎖を打った）

## Rust特有の注意点

### nightlyツールチェインが必須

```sh
# プロジェクトディレクトリで自動的に nightly-2023-10-01 が使われます
rustup show

# 手動でインストールする場合
rustup install nightly-2023-10-01
```

### AVX2/BMI2命令セットの確認

このプロジェクトは高速化のためAVX2とBMI2を使用します。

```sh
# CPUの機能を確認（Linux/Mac）
lscpu | grep avx2
lscpu | grep bmi2

# Macの場合
sysctl -a | grep machdep.cpu.features
```

AVX2/BMI2がない場合、一部の機能が無効化されますが、基本的な動作は可能です。

### ビルドが遅い場合

```sh
# インクリメンタルビルドを有効化（デフォルトで有効）
export CARGO_INCREMENTAL=1

# 並列ビルドジョブ数を増やす
cargo build --release -j 8

# 依存関係の再ビルドを避ける
cargo build --release --frozen
```

### メモリ使用量について

BeamSearchAIは並列探索（デフォルト20スレッド）を行うため、メモリを多く使います：
- 1スレッドあたり約50-100MB
- 合計で約1-2GB程度

メモリが不足する場合、`beam_search_ai.rs` の `parallel` 変数を小さくしてください。

## デバッグのヒント

### ログファイルの場所

```
simulator/logs/cli_1p/BeamSearchAI/YYYYMMDD_HHMMSS_FFFFFF.log
```

### puyop.comでリプレイ確認

シミュレーション結果にpuyop.comのURLが出力されます：
```
https://puyop.com/s/...
```

このURLをブラウザで開くと、視覚的にリプレイを確認できます。

### 配ぷよの固定

同じ配ぷよで何度も試す場合：
```sh
cargo run --release -p ghoti-simulator --bin cli_1p -- --haipuyo_margin 0
```

`--haipuyo_margin` を固定すれば、再現可能なテストができます。

## 評価関数のチューニング

遺伝的アルゴリズムで評価関数を最適化できます：

```sh
# 1Pモード用
cargo run --release -p ghoti-optimizer --bin ga_tuning_1p

# 2Pモード用
cargo run --release -p ghoti-optimizer --bin ga_tuning_2p
```

チューニング結果は自動的に保存され、次回のビルドで反映されます。

## トラブルシューティング

### エラー: `feature 'let_chains' is unstable`

→ nightly toolchainを使用してください。`rust-toolchain` ファイルで自動的に選択されるはずです。

### エラー: `target feature 'avx2' is not available`

→ CPUがAVX2に対応していません。コードを修正してAVX2機能を無効化する必要があります。

### ビルドエラー: リンカエラー

→ 開発ツールが不足している可能性：
```sh
# Ubuntu/Debian
sudo apt install build-essential

# Mac
xcode-select --install
```

### 実行が遅い

→ `--release` フラグを必ず付けてください。デバッグビルドは10倍以上遅いです。

```sh
# ❌ 遅い
cargo run -p ghoti-simulator --bin cli_1p

# ✅ 速い
cargo run --release -p ghoti-simulator --bin cli_1p
```
