# tml-workshop CLI 実装計画

## 目的
`tml-workshop` ライブラリに同パッケージのバイナリを追加し、Steam Workshop の tModLoader 向け CLI を提供する。

## コマンド仕様
アイテム指定は数値 → ID、非数値 → slug を自動判定（`ItemSpec`）。

| コマンド | 引数/フラグ | 挙動 | 使用API |
|---|---|---|---|
| `info <ITEM>` | - | アイテム詳細と最新マニフェストIDを key:value 表示 | 既存のみ |
| `versions <ITEM>` | `--limit N` | バージョン一覧のみ表示 | 既存 `list_versions` |
| `download <ITEM>` | `--version V` \| `--manifest-id N`, `--output TMPL`, `--extract` | .tmodを保存し、任意で内容を展開 | 新規 `save_tmod` のみ |
| `localize <ITEM>` | `--version V` \| `--manifest-id N`, `--format hjson\|json\|csv\|table`, `--culture C` | Localization hjsonを抽出し、hjsonはプレフィクス補正して1つにマージして出力 | 既存のみ |

## localize の hjson 出力仕様
- `--culture` はデフォルト `en-US`。未指定時は en-US のファイルのみ対象（複数カルチャー混在はしない）。
- 該当ファイルが1つの場合は原文をそのまま出力。複数の場合は `# === Localization/xxx ===` 区切りではなく、各ファイルを `CulturePrefix` のプレフィクスパス下にマージした1つの hjson として出力。
- マージはノード単位で行い、**各ファイルの値のクォートスタイル（未クォート/ダブルクォート/ブロック文字列）を保持**する。インデント・空白はフォーマッタで整形（コメントは現状保持しない）。
- マージ時のキー衝突は後勝ちで上書き（tModLoader の実行時マージに準拠）。オブジェクト同士は深くマージ。prefix セグメントがオブジェクトでない等の構造的衝突はエラー。
- マージは hjson-edit の `HjsonDocument::merge_from`（`high_level/ops/merge.rs`）を利用。

## 新規ライブラリAPI（1つのみ、既存APIは無変更）
`WorkshopClient::save_tmod(entry: &ManifestEntry, dest: impl AsyncWrite + Unpin) -> Result<u64, Error>`
- `create_reader` → `tokio::io::copy` で全バイトをストリーム。戻り値はコピー総バイト数。

## `--output` プレースホルダ（download）
- 既定テンプレート: `./output/{modname}.tmod`
- 利用可: `{modname}` `{modversion}` `{itemid}` `{manifestid}`（ヘッダ由来はヘッダ解析後に解決）
- 未知プレースホルダはエラー。置換値に Windows 不正文字（`\ / : * ? " < > |` と制御文字）を含む場合はエラー（フォールバックなし）。

## `--extract`
- 展開先: 置換後 `--output` パスの親ディレクトリ配下に `<modname>/` サブフォルダを作成して展開。
- エントリ名が絶対パス・`..` を含む場合はエラー拒否（Zip Slip 対策）。

## ファイル構成 / 依存
- `src/main.rs`, `src/cli/{mod,args,info,versions,download,localize}.rs`
- 追加依存: `clap 4.6`(derive), `tabled 0.21`, `csv 1.4`, `indicatif 0.18`, `tracing-subscriber`(env-filter)
- JSON 出力は既存 `serde_json` を利用。

## 検証
- 既存の統合テスト（tests/client.rs）は無変更で維持。
- `save_tmod` のテストを追加（マジック `TMOD`・サイズ・mod名整合）。
- `cargo build` / `cargo test` / `cargo fmt --check` / `cargo clippy` で確認。
