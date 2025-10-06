<div align="center">
    <b>Note: このリポジトリは <a href="https://github.com/morioprog/ghoti">morioprog/ghoti</a> のフォークです</b>:bow:
</div>

---

<h1 align="center">
    ghoti
</h1>

[![Rust](https://github.com/puyopop/ghoti-fork/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/puyopop/ghoti-fork/actions/workflows/rust.yml)

```sh
# インタラクティブモード (NEW!)
$ cargo run --release -p ghoti-simulator --bin cli_interactive
# puyop.com URLから盤面を読み込んで起動
$ cargo run --release -p ghoti-simulator --bin cli_interactive "https://puyop.com/s/420Aa9r9hj"

# とこぷよ
$ cargo run --release -p ghoti-simulator --bin cli_1p [-- --help]

# 2人対戦
$ cargo run --release -p ghoti-simulator --bin cli_2p [-- --help]

# 棋譜を見る (WIP)
$ cargo run --release -p ghoti-simulator --bin replay_kifus
```

## 新機能

### 🎮 インタラクティブモード (`cli_interactive`)
ターミナル上でぷよぷよをプレイできる学習ツール：
- **リアルタイムAIサジェスト**: 常に最善手を表示
- **Undo機能**: uキーで1手前に戻る
- **連鎖アニメーション**: 連鎖をステップごとに表示
- **puyop.com連携**: URLで盤面の保存・読み込み

詳細は [CLI_INTERACTIVE_GUIDE.md](./docs/CLI_INTERACTIVE_GUIDE.md) を参照。

<p align="center">
    <a href="https://youtu.be/hr0YxksDlKQ?t=168">
        <img src="http://img.youtube.com/vi/hr0YxksDlKQ/0.jpg" />
        <br />
        YouTube - <i>自作ぷよAI（ghoti）と20先【ぷよぷよeスポーツ】</i> (2022/07/24)
    </a>
</p>
