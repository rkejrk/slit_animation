# スリットアニメーション作成ツール

GIFファイルからスリットアニメーションを作成するWebアプリです。

試してみる→ https://rkejrk.github.io/slit_animation/static/index.html

## 機能

- スリット幅の調整
- フレーム数の設定
- 結合画像とマスク画像の生成
- アニメーション効果のプレビュー

## 必要な環境

- Rust (最新版)
- wasm-pack
- モダンブラウザ (WebAssembly対応)

## セットアップ

1. wasm-packをインストール:
```bash
cargo install wasm-pack
```

2. WebAssemblyをビルド:
```bash
wasm-pack build --target web --out-dir static/pkg
```

3. ブラウザでindex.htmlを開く:
```bash
# 簡易HTTPサーバーを起動（Python3が必要）
python3 -m http.server 8000
```

または、任意のHTTPサーバーを使用してstatic/index.htmlを提供してください。

## 使用方法

1. ブラウザでアプリケーションを開く
2. GIFファイルを選択
3. スリット幅とフレーム数を設定
4. 「処理開始」ボタンをクリック
5. 結果を確認

## 技術仕様

- **フロントエンド**: HTML5, CSS3, JavaScript (ES6+)
- **バックエンド**: Rust + WebAssembly
- **画像処理**: image crate
- **ビルドツール**: wasm-pack

## ファイル構造

```
slit_animation/
├── src/
│   └── lib.rs          # WebAssemblyライブラリ
├── static/
│   ├── index.html      # メインHTMLファイル
│   └── pkg/            # ビルドされたWebAssemblyファイル
├── Cargo.toml          # Rust依存関係
├── build.sh            # ビルドスクリプト
└── README.md           # このファイル
```

## トラブルシューティング

### WebAssemblyが読み込まれない場合
- ブラウザがWebAssemblyをサポートしているか確認
- HTTPサーバー経由でアクセスしているか確認（file://では動作しません）
- ブラウザのコンソールでエラーメッセージを確認

### ビルドエラーが発生する場合
- Rustが最新版か確認: `rustup update`
- wasm-packがインストールされているか確認: `cargo install wasm-pack`

## ライセンス

MIT License 