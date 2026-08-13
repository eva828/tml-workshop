use sha1::{Digest, Sha1};
use std::time::Duration;
use steam_vent_cdn::error::{SteamVentError, WorkshopError};
use tml_workshop::{
    Error, ItemId, LocalizationFile, ManifestId, TmodReader, Version, VersionCache, WorkshopClient,
};

const ITEM_ID: u64 = 2986383249;
const MANIFEST_ID: u64 = 6272043491709940234;

/// fetch_item_details の基本フィールド（ID・タイトル・consumer_appid）と
/// 最新マニフェスト ID の取得がエラーなく行えることを検証
#[tokio::test]
async fn fetch_item_details_and_latest_manifest_id() {
    let item_id = ItemId::new(ITEM_ID);
    let expected_title = "External Localizer";
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. fetch_item_details でアイテム詳細を取得、ID が 2986383249 と一致することを検証
    let details = tokio::time::timeout(Duration::from_secs(60), client.fetch_item_details(item_id))
        .await
        .expect("fetch_item_details timed out")
        .expect("fetch_item_details");
    assert_eq!(details.publishedfileid, ITEM_ID);
    // 3. タイトル・consumer_appid が実測値と一致することを検証
    assert_eq!(details.title, expected_title);
    assert_eq!(details.consumer_appid, Some(1281930));

    // 4. fetch_latest_manifest_id がエラーなく最新マニフェスト ID を返すことを検証
    tokio::time::timeout(
        Duration::from_secs(60),
        client.fetch_latest_manifest_id(item_id),
    )
    .await
    .expect("fetch_latest_manifest_id timed out")
    .expect("fetch_latest_manifest_id");
}

/// 固定マニフェスト ID から取得した Manifest のファイル一覧が実測値と完全一致することを検証
#[tokio::test]
async fn fetch_manifest_by_id_returns_fixed_manifest() {
    let manifest_id = ManifestId::new(MANIFEST_ID);
    let expected_files: [&str; 4] = [
        "2025.1/ExternalLocalizer.tmod",
        "2025.3/ExternalLocalizer.tmod",
        "2026.1/ExternalLocalizer.tmod",
        "workshop.json",
    ];
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. 固定マニフェスト ID で Manifest を取得
    let manifest = tokio::time::timeout(Duration::from_secs(60), client.fetch_manifest_by_id(manifest_id))
        .await
        .expect("fetch_manifest_by_id timed out")
        .expect("fetch_manifest_by_id");

    // 3. ファイル一覧（ソート済み）が実測値と完全一致することを検証
    let mut files: Vec<&str> = manifest.files.iter().map(|f| f.filename.as_str()).collect();
    files.sort_unstable();
    assert_eq!(files, expected_files);
}

/// pick_latest_tmod による .tmod 選定と parse_header の全フィールド・オフセット累積を実測値と完全一致で検証
#[tokio::test]
async fn pick_latest_tmod_and_parse_header() {
    let manifest_id = ManifestId::new(MANIFEST_ID);
    let expected_tmod_filename = "2026.1/ExternalLocalizer.tmod";
    let expected_mod_name = "ExternalLocalizer";
    let expected_mod_version = "0.1.14.11";
    let expected_tml_version = "2026.1.3.1";
    let expected_hash = "66275a50bf75924230211eb7a665babca37396f5";
    let expected_first_offset = 1275u64;
    let expected_entries: [(&str, u32, u32); 23] = [
        ("ExternalLocalizer.dll", 64512, 29205),
        ("ExternalLocalizer.pdb", 32104, 17926),
        ("Info", 2060, 1052),
        ("ExternalLocalizer.code-workspace", 251, 251),
        ("LICENSE.txt", 1090, 641),
        ("icon.png", 5489, 5489),
        ("README.md", 4398, 1844),
        ("description_workshop.txt", 1940, 1001),
        ("Localization/en-US_Mods.ExternalLocalizer.hjson", 6520, 2106),
        ("changelog.txt", 139, 139),
        (
            "tools/BetaBranchUpdater/obj/Debug/net8.0/BetaBranchUpdater.GeneratedMSBuildEditorConfig.editorconfig",
            848,
            848,
        ),
        ("description_workshop_ja.txt", 3409, 1556),
        ("description.txt", 1660, 849),
        ("Localization/ja-JP.hjson", 89232, 23664),
        (
            "tools/BetaBranchUpdater/obj/Debug/net8.0/BetaBranchUpdater.AssemblyInfoInputs.cache",
            66,
            66,
        ),
        ("Localization/ja-JP_Mods.ExternalLocalizer.hjson", 9173, 2661),
        ("icon_workshop.rawimg", 1048588, 8149),
        ("lib/UtfUnknown.dll", 250880, 105928),
        ("Assets/Fonts/WrathOfTheGods/DraedonText.xnb", 618236, 618236),
        ("Assets/Fonts/TerraTCG/SmallText.xnb", 1259956, 1259956),
        (
            "Assets/Fonts/WrathOfTheGods/SolynFightDialogue.xnb",
            1754648,
            1754648,
        ),
        ("Assets/Fonts/WrathOfTheGods/SolynText.xnb", 11251078, 11251078),
        (
            "Assets/Fonts/WrathOfTheGods/SolynTextItalics.xnb",
            12294266,
            12294266,
        ),
    ];
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. 固定マニフェストを取得
    let manifest = tokio::time::timeout(Duration::from_secs(60), client.fetch_manifest_by_id(manifest_id))
        .await
        .expect("fetch_manifest_by_id timed out")
        .expect("fetch_manifest_by_id");

    // 3. pick_latest_tmod が 2026.1/ExternalLocalizer.tmod を選ぶことを検証
    let entry = tokio::time::timeout(
        Duration::from_secs(60),
        client.pick_latest_tmod(&manifest),
    )
    .await
    .expect("pick_latest_tmod timed out")
    .expect("pick_latest_tmod")
    .expect("no .tmod entry in manifest");
    assert_eq!(entry.filename, expected_tmod_filename);

    // 4. reader_for → parse_header でヘッダを取得
    let mut tmod = client.reader_for(&entry).expect("reader_for");
    let header = tokio::time::timeout(Duration::from_secs(60), tmod.parse_header())
        .await
        .expect("parse_header timed out")
        .expect("parse_header");

    // 5. mod 名・mod バージョン・TML バージョン・ハッシュが実測値と一致することを検証
    assert_eq!(header.mod_name, expected_mod_name);
    assert_eq!(header.mod_version, expected_mod_version);
    assert_eq!(header.tml_version, expected_tml_version);
    let actual_hash: String = header.hash.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(actual_hash, expected_hash);

    // 6. エントリ 23 件の (name, uncompressed, compressed) が実測値と一致することを検証
    assert_eq!(header.entries.len(), expected_entries.len());
    for (actual, expected) in header.entries.iter().zip(expected_entries.iter()) {
        assert_eq!(actual.name, expected.0);
        assert_eq!(actual.uncompressed_size, expected.1);
        assert_eq!(actual.compressed_size, expected.2);
    }

    // 7. 先頭オフセットが 1275（ヘッダ終端）であることと、以降の累積計算を全件検証
    assert_eq!(header.entries[0].offset, expected_first_offset);
    for pair in header.entries.windows(2) {
        assert_eq!(
            pair[1].offset,
            pair[0].offset + pair[0].compressed_size as u64,
            "offset accumulation for {:?}",
            pair[1].name
        );
    }
}

/// 固定マニフェスト内の hjson を抽出し、内容全体の SHA-1・カルチャー・フラットキーを実測値と完全一致で検証
#[tokio::test]
async fn extract_localize_file_and_verify_content_hash() {
    let manifest_id = ManifestId::new(MANIFEST_ID);
    let expected_localize_path = "Localization/en-US_Mods.ExternalLocalizer.hjson";
    let expected_content_sha1 = "67477d3129587f5ce6cff1e283799736f309916d";
    let expected_culture = "en-US";
    let expected_prefix = "Mods.ExternalLocalizer";
    let expected_flat_count = 93usize;
    let expected_keys: [&str; 4] = [
        "Mods.ExternalLocalizer.Configs.ExtractCategoryOption.All.Label",
        "Mods.ExternalLocalizer.Configs.ExtractCategoryOption.Mods.Label",
        "Mods.ExternalLocalizer.Configs.ExtractionConfig.DisplayName",
        "Mods.ExternalLocalizer.Configs.ExtractionConfig.ExtractAtIndividualFiles.Label",
    ];
    let expected_value: (&str, &str) = (
        "Mods.ExternalLocalizer.Configs.ExtractCategoryOption.All.Label",
        "All",
    );
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. 固定マニフェストを取得し、pick_latest_tmod で .tmod を選ぶ
    let manifest = tokio::time::timeout(Duration::from_secs(60), client.fetch_manifest_by_id(manifest_id))
        .await
        .expect("fetch_manifest_by_id timed out")
        .expect("fetch_manifest_by_id");

    let entry = tokio::time::timeout(
        Duration::from_secs(60),
        client.pick_latest_tmod(&manifest),
    )
    .await
    .expect("pick_latest_tmod timed out")
    .expect("pick_latest_tmod")
    .expect("no .tmod entry in manifest");

    // 3. parse_header でエントリ一覧を取得
    let mut tmod = client.reader_for(&entry).expect("reader_for");
    let header = tokio::time::timeout(Duration::from_secs(60), tmod.parse_header())
        .await
        .expect("parse_header timed out")
        .expect("parse_header");

    // 4. 対象 hjson（Localization/en-US_Mods.ExternalLocalizer.hjson）が存在することを検証
    let localize_entry = header
        .entries
        .iter()
        .find(|e| e.name == expected_localize_path)
        .expect("localize entry not found in tmod");

    // 5. extract_file で抽出し、SHA-1 が実測値と一致することを検証
    let bytes = tokio::time::timeout(Duration::from_secs(120), tmod.extract_file(localize_entry))
        .await
        .expect("extract_file timed out")
        .expect("extract_file");

    let digest = Sha1::digest(&bytes);
    let actual_sha1: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(actual_sha1, expected_content_sha1);

    // 6. extract_file_as_text → LocalizationFile::parse でパース
    let text = tmod
        .extract_file_as_text(localize_entry)
        .await
        .expect("extract_file_as_text");

    let loc = LocalizationFile::parse(&localize_entry.name, &text)
        .expect("LocalizationFile::parse");
    // 7. culture・prefix が実測値と一致することを検証
    assert_eq!(loc.culture_prefix().culture, expected_culture);
    assert_eq!(loc.culture_prefix().prefix, expected_prefix);

    // 8. flatten のキー件数・代表キー 4 件・代表値 1 件を完全一致で検証
    let flat = loc.flatten().expect("flatten");
    assert_eq!(flat.len(), expected_flat_count);
    for key in expected_keys {
        assert!(flat.contains_key(key), "missing key: {key}");
    }
    assert_eq!(flat.get(expected_value.0), Some(&expected_value.1.to_string()));
}

/// list_versions が tmod ヘッダから解決したバージョンのリストを、新しい順（意味的降順）で返すことを検証
/// 履歴は増えるため、完全一致ではなく順序と非空のみ検証する
#[tokio::test]
async fn list_versions_returns_resolved_versions() {
    let item_id = ItemId::new(ITEM_ID);
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. list_versions(Some(3)) で最新 3 件のバージョンを取得
    let versions = tokio::time::timeout(Duration::from_secs(120), client.list_versions(item_id, Some(3)))
        .await
        .expect("list_versions timed out")
        .expect("list_versions");

    // 3. リストが非空であることを検証
    assert!(!versions.is_empty());
    // 4. 新しい順（意味的降順）に並んでいることを検証
    for pair in versions.windows(2) {
        assert!(pair[0] > pair[1], "not descending: {:?}", versions);
    }
}

/// find_version が実測のバージョンから固定マニフェスト ID を返すことと、
/// 存在しないバージョンで VersionNotFound を返すことを検証
#[tokio::test]
async fn find_version_matches_and_reports_missing() {
    let item_id = ItemId::new(ITEM_ID);
    let expected_version = Version::parse("0.1.14.11").expect("Version::parse known version");
    let missing_version = Version::parse("0.0.0").expect("Version::parse missing version");
    let expected_manifest_id = ManifestId::new(MANIFEST_ID);
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. find_version("0.1.14.11") が固定マニフェスト ID と一致することを検証
    let found = tokio::time::timeout(
        Duration::from_secs(120),
        client.find_version(item_id, expected_version),
    )
    .await
    .expect("find_version timed out")
    .expect("find_version");
    assert_eq!(found, expected_manifest_id);

    // 3. find_version("0.0.0") が VersionNotFound エラーになることを検証
    let err = tokio::time::timeout(
        Duration::from_secs(300),
        client.find_version(item_id, missing_version),
    )
    .await
    .expect("find_version (missing) timed out")
    .expect_err("find_version should fail for nonexistent version");
    assert!(
        matches!(
            err,
            Error::Cdn(SteamVentError::Workshop(WorkshopError::VersionNotFound(_)))
        ),
        "unexpected error: {err:?}"
    );
}

/// 解決済みバージョンの VersionCache が save → load で復元できることを検証
#[tokio::test]
async fn version_cache_save_load_roundtrip() {
    let item_id = ItemId::new(ITEM_ID);
    let expected_version = Version::parse("0.1.14.11").expect("Version::parse known version");
    let expected_manifest_id = ManifestId::new(MANIFEST_ID);
    let cache_path = std::env::temp_dir().join(format!(
        "tml-workshop-cache-{}.json",
        std::process::id()
    ));
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. find_version でバージョンを解決し、キャッシュに mid → バージョンが入ることを準備
    tokio::time::timeout(
        Duration::from_secs(120),
        client.find_version(item_id, expected_version),
    )
    .await
    .expect("find_version timed out")
    .expect("find_version");

    // 3. 一時ファイル（PID 付き一意名）に save
    client.cache().save(&cache_path).expect("cache save");

    // 4. load で復元し、get が固定バージョン文字列と一致することを検証
    let loaded = VersionCache::load(&cache_path).expect("cache load");
    assert_eq!(loaded.get(expected_manifest_id), Some(expected_version.to_string()));

    // 5. 一時ファイルを削除
    let _ = std::fs::remove_file(&cache_path);
}

/// save_tmod が .tmod 全体をストリーム保存し、保存ファイルがパース可能であることを検証
#[tokio::test]
async fn save_tmod_writes_valid_tmod_file() {
    let manifest_id = ManifestId::new(MANIFEST_ID);
    let cache_path = std::env::temp_dir().join(format!(
        "tml-workshop-save-tmod-{}.tmod",
        std::process::id()
    ));
    // 1. 接続
    let client = WorkshopClient::connect().await.expect("connect");

    // 2. 固定マニフェストを取得し、最新 .tmod を選定
    let manifest = tokio::time::timeout(
        Duration::from_secs(60),
        client.fetch_manifest_by_id(manifest_id),
    )
    .await
    .expect("fetch_manifest_by_id timed out")
    .expect("fetch_manifest_by_id");
    let entry = tokio::time::timeout(Duration::from_secs(60), client.pick_latest_tmod(&manifest))
        .await
        .expect("pick_latest_tmod timed out")
        .expect("pick_latest_tmod")
        .expect("no .tmod entry in manifest");

    // 3. save_tmod で一時ファイルへ保存
    let file = tokio::fs::File::create(&cache_path)
        .await
        .expect("create file");
    let bytes = tokio::time::timeout(Duration::from_secs(300), client.save_tmod(&entry, file))
        .await
        .expect("save_tmod timed out")
        .expect("save_tmod");

    // 4. 書き込みバイト数がマニフェスト上のサイズと一致することを検証
    assert_eq!(bytes, entry.size.expect("entry size"));

    // 5. 保存ファイルを TmodReader でパースし、mod 名が一致することを検証
    let opened = tokio::fs::File::open(&cache_path).await.expect("open file");
    let mut tmod = TmodReader::new(opened);
    let header = tmod.parse_header().await.expect("parse_header");
    assert_eq!(header.mod_name, "ExternalLocalizer");

    // 6. 一時ファイルを削除
    let _ = tokio::fs::remove_file(&cache_path).await;
}
