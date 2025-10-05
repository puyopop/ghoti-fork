# puyop.com URL デコーダー

## 概要

`PuyopDecoder` は puyop.com の URL から盤面とツモ情報をデコードし、`analyze_position` で解析できるようにします。

## 使い方

### 基本的な使い方

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --url "http://www.puyop.com/s/420Aa9r9hj" \
  --top-n 10
```

### URLプレフィックスなし

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --url "420Aa9r9hj" \
  --top-n 5
```

### 操作履歴付きURL

```sh
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --url "420Aa9r9hj_0a0b0c" \
  --verbose
```

## URL形式

puyop.comのURL構造：

```
http://www.puyop.com/s/{field}_{control}
                        ^^^^^^  ^^^^^^^^
                        盤面     ツモ+操作
```

### フィールド部分

- 3列ペア (1-2, 3-4, 5-6) を1文字にエンコード
- 上から下へ (y=13 → y=1)
- エンコーディング: `d = color(x) * 8 + color(x+1)`
- 合計: 13段 × 3ペア = 最大39文字

色のマッピング：
```rust
0 => EMPTY
1 => RED
2 => GREEN
3 => BLUE
4 => YELLOW
6 => OJAMA
```

### コントロール部分

- 各手は2文字
- 1手目: `(tsumo_axis * 5 + tsumo_child) | ((axis_x << 2 | rot) << 7)`
- 1文字目: 下位6ビット
- 2文字目: 上位6ビット

ツモ色のマッピング：
```rust
0 => RED
1 => GREEN
2 => BLUE
3 => YELLOW
```

### エンコーディング文字セット

64文字（base64風）：
```
0-9, a-z, A-Z, [, ]
```

## 実装詳細

### デコーダーの構造

```rust
use ghoti_simulator::puyop_decoder::PuyopDecoder;

let decoder = PuyopDecoder::new();
let (field, tumos, decisions) = decoder.decode_url(url)?;

// field: CoreField - デコードされた盤面
// tumos: Vec<Kumipuyo> - ツモ列
// decisions: Vec<Decision> - 操作履歴
```

### テストケース

元のエンコーダーのテストケース：

```rust
// URL: http://www.puyop.com/s/420Aa9r9hj
// 期待される盤面:
.....Y
.G..YY
RGRRBB
RRGRGB
```

デコード結果：
```sh
$ cargo run --release -p ghoti-simulator --bin analyze_position -- \
    --url "420Aa9r9hj"

=== Position Analysis ===

  1 2 3 4 5 6
...
 4│🔵🔵🔴🟢🔴🔴│
 3│🔵🔵🔴🟢🔴🔴│
 2│🟢🔵🔴🔴🟢🔴│
 1│🟢🔵🔴🔴🟢🔴│
```

## 制約と注意点

### 1. 盤面構築の制約

`CoreField` には直接ぷよを設定するAPIがないため、`drop_kumipuyo` を使って構築しています。

これにより以下の制約があります：
- 空中に浮いたぷよは正しく配置できない
- おじゃまぷよの配置は未対応

### 2. 完全な互換性

元の `make_puyop_url()` との完全な往復変換はテストが必要です：

```rust
// エンコード
let url1 = make_puyop_url(&field, &seq, &decisions);

// デコード
let (field2, seq2, decisions2) = decoder.decode_url(&url1)?;

// 再エンコード
let url2 = make_puyop_url(&field2, &seq2, &decisions2);

// url1 == url2 であるべき（理想）
```

### 3. 空の盤面

盤面が空の場合、フィールド部分は空文字列になります：

```
http://www.puyop.com/s/_0a0b0c
                        ^ 空
```

## エンコーディング詳細

### 例：1手分のエンコード

ツモ: 赤赤（axis=RED, child=RED）
操作: 3列目に縦置き (axis_x=3, rot=0)

```rust
// ツモ部分
tsumo_axis_id = 0  // RED
tsumo_child_id = 0 // RED
tsumo_data = 0 * 5 + 0 = 0

// 操作部分
h = (3 << 2) | 0 = 12

// 結合
d = 0 | (12 << 7) = 1536

// エンコード
c0 = d & 0x3F = 0        -> '0'
c1 = (d >> 6) & 0x3F = 24 -> 'o'

// 結果: "0o"
```

### デコード検証

```rust
c0 = '0' -> 0
c1 = 'o' -> 24
d = 0 | (24 << 6) = 1536

tsumo_data = 1536 & 0x7F = 0
  axis_id = 0 / 5 = 0 (RED)
  child_id = 0 % 5 = 0 (RED)

h = 1536 >> 7 = 12
  axis_x = 12 >> 2 = 3
  rot = 12 & 0x3 = 0
```

## トラブルシューティング

### Q: 盤面が正しくデコードされない

A: 以下を確認してください：
1. URLが正しいか（`http://www.puyop.com/s/` で始まるか）
2. エンコード文字が64文字セット内か
3. 盤面部分の長さが妥当か（最大39文字）

### Q: ツモが表示されない

A: URL に `_` 区切りのコントロール部分がない場合、ツモ情報はありません。`--tumos` オプションで指定してください。

### Q: 空中に浮いたぷよが再現できない

A: 現在の実装では `drop_kumipuyo` を使っているため、物理法則に従います。空中に浮いたぷよの完全再現には、puyoai-coreの内部APIアクセスが必要です。

## 応用例

### 1. puyop.comのリプレイを解析

```sh
# puyop.comで対局後、URLをコピー
URL="http://www.puyop.com/s/xxx_yyy"

# 初期盤面から最善手を確認
cargo run --release -p ghoti-simulator --bin analyze_position -- \
  --url "$URL" \
  --top-n 5 \
  --verbose
```

### 2. 特定局面の研究

```sh
# puyop.comで局面を作成
# URLの盤面部分のみコピー
FIELD="420Aa9r9hj"

# 様々なツモで解析
for tumo in "RR,BY" "GG,YY" "BR,RG"; do
  echo "=== Tumo: $tumo ==="
  cargo run --release -p ghoti-simulator --bin analyze_position -- \
    --url "$FIELD" \
    --tumos "$tumo" \
    --top-n 3
done
```

### 3. エンコード/デコードの検証

```rust
// テストコード
#[test]
fn test_roundtrip() {
    let original_field = CoreField::from_str("...");
    let original_seq = vec![...];
    let original_decisions = vec![...];

    // エンコード
    let url = make_puyop_url(&original_field, &original_seq, &original_decisions);

    // デコード
    let decoder = PuyopDecoder::new();
    let (decoded_field, decoded_seq, decoded_decisions) =
        decoder.decode_url(&url).unwrap();

    // 検証
    assert_eq!(original_field, decoded_field);
    assert_eq!(original_seq, decoded_seq);
    assert_eq!(original_decisions, decoded_decisions);
}
```

## まとめ

`PuyopDecoder` により、**puyop.comのURLから直接盤面を読み込んで解析**できるようになりました！

主な用途：
- ✅ puyop.comのリプレイ解析
- ✅ 特定局面の最善手研究
- ✅ URL共有による局面の再現

制約：
- ⚠️ 空中に浮いたぷよは再現できない
- ⚠️ おじゃまぷよの配置は簡易的

これで、ご質問いただいた「puyop.comのURLから盤面とツモを生成」が実現できました！
