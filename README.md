# tml-workshop

tModLoader の Steam Workshop アイテムを取得・解析する Rust ライブラリと CLI。

- **取得**: アイテムの詳細・最新マニフェスト ID・マニフェストのファイル一覧を Steam Workshop API と CDN から取得する
- **バージョン解決**: `.tmod` のヘッダを読んで実際の mod バージョンを解決し、一覧表示とバージョン指定・マニフェスト ID 指定での取得に対応する
- **ダウンロード**: `.tmod` をそのままストリーム保存し、必要なら内容を展開する
- **ローカライズ**: `.tmod` 内の `Localization` 配下のファイルをカルチャー別に抽出し、hjson・json・csv・table で出力する
- **キャッシュ**: CDN から取得したマニフェストとチャンクをディスクにキャッシュし、同じデータの再取得を避ける

## 必要環境

Rust 1.88 以降（edition 2024 と let chains を使用）。Nix を使う場合は `flake.nix` の devShell で同じツールチェーンが揃う。

## インストール

```bash
cargo install --path .

# git から直接
cargo install --git https://github.com/eva828/tml-workshop
```

Nix を使う場合:

```bash
nix build      # ./result/bin/tml-workshop
nix develop    # 開発用シェル
```

## 使い方

`<ITEM>` には Workshop のアイテム ID（数値）かアイテム名（slug、例 `ExternalLocalizer`）を指定する。
数値以外を渡すとアイテム名として検索され、該当が 0 件または複数件ならエラーになる。

```bash
# アイテム情報
tml-workshop info 2986383249
tml-workshop info ExternalLocalizer

# バージョン一覧（新しい順）
tml-workshop versions ExternalLocalizer
tml-workshop versions ExternalLocalizer --limit 5

# .tmod のダウンロード（指定が無ければ最新バージョン）
tml-workshop download ExternalLocalizer
tml-workshop download ExternalLocalizer --version 0.1.14.11
tml-workshop download ExternalLocalizer --manifest-id 6272043491709940234

# 出力先を変える（プレースホルダが使える）
tml-workshop download ExternalLocalizer -o './output/{itemid}/{modversion}.tmod'

# ダウンロードして内容も展開する
tml-workshop download ExternalLocalizer --extract

# ローカライズの抽出
tml-workshop localize ExternalLocalizer                     # hjson（既定）
tml-workshop localize ExternalLocalizer --culture ja-JP
tml-workshop localize ExternalLocalizer --format json
tml-workshop localize ExternalLocalizer --format csv
tml-workshop localize ExternalLocalizer --format table
```

### versions

- `-l/--limit` で取得するバージョン数の上限を指定できる。
- バージョンは `.tmod` のヘッダから解決するため、リストが長い場合は時間がかかる。

### download

- `--version` と `--manifest-id` は同時に指定できない。
- `-o/--output` の既定値は `./output/{modname}.tmod`。使えるプレースホルダは `{modname}` `{modversion}` `{itemid}` `{manifestid}` で、値にパスとして不正な文字が含まれる場合はエラーになる。
- `--extract` を付けると、出力ファイルの親ディレクトリに `<modname>/` を作って全エントリを書き出す。`.tmod` 内のエントリ名が絶対パスや `..` を含む場合はエラーになる。

### localize

- `-f/--format`: `hjson`（既定）/ `json` / `csv` / `table`。
- `--culture`: 抽出するカルチャー（既定 `en-US`）。
  `en-US`, `de-DE`, `it-IT`, `fr-FR`, `zh-Hans`, `es-ES`, `ru-RU`, `pt-BR`, `pl-PL`, `zh-Hant`, `tr-TR`, `uk-UA`, `ko-KR`, `ja-JP`, `vi-VN`, `th-TH`, `pt-PT`, `cs-CZ`, `hu-HU`, `sv-SE`, `nl-NL`, `nb-NO`, `da-DK`, `fi-FI`, `ro-RO`, `el-GR`, `hr-HR`, `sk-SK`, `sr-SR`, `bg-BG`, `ar-SA`, `he-IL`, `id-ID`, `ms-MY` の 34 種類。
- `hjson` では、カルチャーに一致するファイルをファイル名の prefix（例 `Mods.ExternalLocalizer`）配下に 1 つの文書へマージして出力する。ファイルが 1 つだけなら中身をそのまま出力する。
- 指定したカルチャーのファイルが 1 つも無い場合はエラーになる。

### グローバルオプション

| オプション | 説明 |
| --- | --- |
| `-v`, `--verbose` | stderr に debug ログを出す（`RUST_LOG` が設定されていればそちらを優先） |
| `--no-cache` | グローバルキャッシュを使わない |
| `-h`, `--help` | ヘルプを表示 |
| `-V`, `--version` | バージョンを表示（トップレベルのみ） |

コマンドが失敗すると `error: ...` を stderr に出して終了コード 1 で終わる。

## キャッシュ

CDN から取得したマニフェストとチャンクをディスクにキャッシュし、同じデータを再度ダウンロードしない。

- 保存先: `TML_WORKSHOP_CACHE_DIR` があればそのディレクトリ、無ければ OS のキャッシュディレクトリ配下の `tml-workshop`（Linux では通常 `$XDG_CACHE_HOME/tml-workshop`）。
- 無効化: `--no-cache`。

## ライブラリとして使う

CLI と同じ処理をライブラリから呼べる。`WorkshopClient` は接続時に tModLoader（AppID 1281930）のセッションを確立する。

```rust
use tml_workshop::{LocalizationFile, Version, WorkshopClient};

#[tokio::main]
async fn main() -> Result<(), tml_workshop::Error> {
    let client = WorkshopClient::connect(None).await?;

    // アイテム ID は直接指定しても、アイテム名から検索してもよい
    let item_id = client.find_item_by_slug("ExternalLocalizer").await?;
    let details = client.fetch_item_details(item_id).await?;
    println!("{} ({item_id})", details.title);

    // バージョンは .tmod のヘッダから解決される
    for version in client.list_versions(item_id, Some(5)).await? {
        println!("{version}");
    }

    // バージョン → マニフェスト ID → そのマニフェストで最新の .tmod
    let manifest_id = client.find_version(item_id, Version::parse("0.1.14.11")?).await?;
    let manifest = client.fetch_manifest_by_id(manifest_id).await?;
    let entry = client
        .pick_latest_tmod(&manifest)
        .await?
        .expect("manifest has no .tmod entry");

    // ヘッダは CDN から必要な範囲だけ取得してパースされる
    let mut tmod = client.reader_for(&entry)?;
    let header = tmod.parse_header().await?;
    println!(
        "{} {} (tModLoader {})",
        header.mod_name, header.mod_version, header.tml_version
    );

    // 展開せずに .tmod 内のローカライズを直接読む
    let target = "Localization/en-US_Mods.ExternalLocalizer.hjson";
    if let Some(localization) = header.entries.iter().find(|e| e.name == target) {
        let text = tmod.extract_file_as_text(localization).await?;
        let loc = LocalizationFile::parse(&localization.name, &text)?;
        println!("culture: {}", loc.culture_prefix().culture);
        for (key, value) in loc.flatten()? {
            println!("{key} = {value}");
        }
    }

    Ok(())
}
```

- `.tmod` 全体を保存したい場合は `client.save_tmod(&entry, writer)`、内容を書き出したい場合は `TmodReader::extract_file` / `extract_file_as_text` を使う。
- 非公開・削除済みなどでアイテム情報が取れない場合は `Error::is_workshop_item_unavailable()` で判別できる。
- `VersionCache`（マニフェスト ID → 解決済みバージョン）はプロセス内のメモリキャッシュ。`VersionCache::load` / `save` でファイルに読み書きできるが、CLI はプロセス内でのみ使う。

## 開発

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --lib --bins   # ネットワーク不要のユニットテスト
cargo test                # Steam CDN へ接続する統合テスト
```

`tests/client.rs` は実際の Workshop アイテム（AppID 1281930、アイテム 2986383249）に接続するため、実行にはネットワークとアイテムの公開状態が必要になる。
Nix 環境では `nix develop -c cargo test` のように devShell 経由で実行する。

## 備考

このプロジェクトは AI の支援を受けて開発している。

## ライセンス

MIT（`LICENSE` を参照）
