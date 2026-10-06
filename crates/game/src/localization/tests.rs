use super::*;
use crate::settings::{ConsoleSettings, GameSettings, SettingsFile, SettingsStore};

#[test]
fn catalogs_use_ids_and_explicit_arguments_without_translating_values() {
    let en: Catalog =
        serde_json::from_str(include_str!("../../../../assets/locales/en.json")).unwrap();
    let ru: Catalog =
        serde_json::from_str(include_str!("../../../../assets/locales/ru.json")).unwrap();
    assert_eq!(
        en.messages.keys().collect::<Vec<_>>(),
        ru.messages.keys().collect::<Vec<_>>()
    );
    let mut localization = Localization::default();
    localization.select("ru").unwrap();
    assert_eq!(localization.text("settings.title"), "Настройки");
    // English text is never a lookup key and arguments are never recursively translated.
    assert_eq!(localization.text("Settings"), "[Settings]");
    assert!(localization.select("missing").is_err());
    {
        let _scope = localization.enter();
        assert_eq!(tr("game.save"), "Сохранить");
        assert_eq!(
            tr_args("settings.listening", &["127.0.0.1".into(), "47655".into()]),
            "Консоль слушает 127.0.0.1:47655"
        );
        assert_eq!(
            tr_args("hardware.install-item", &["Settings".into()]),
            "Установить Settings"
        );
        let error: UiMessage = cloud_provider_sim::SimError::InsufficientFunds {
            needed: 320,
            available: 12,
        }
        .into();
        assert_eq!(
            error.render(),
            "Недостаточно денег: нужно $320, доступно $12"
        );
    }
    assert_eq!(tr("game.save"), "Save");
}

#[test]
fn runtime_catalog_overrides_validate_template_arguments_and_fall_back() {
    let directory =
        std::env::temp_dir().join(format!("game-locale-catalog-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("en.json"),
        r#"{"language":"en","name":"English","messages":{"game.save":"Store"}}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("ru.json"),
        r#"{"language":"ru","name":"Русский","messages":{"game.saved-path":"Файл: {0}"}}"#,
    )
    .unwrap();
    let mut localization = Localization::load(&directory);
    assert_eq!(localization.text("game.save"), "Store");
    localization.select("ru").unwrap();
    assert_eq!(localization.text("settings.title"), "Settings");
    {
        let _scope = localization.enter();
        assert_eq!(
            tr_args("game.saved-path", &["/tmp/game.db".into()]),
            "Файл: /tmp/game.db"
        );
    }
    std::fs::write(
        directory.join("ru.json"),
        r#"{"language":"ru","name":"Русский","messages":{"game.saved-path":"Wrong {1}"}}"#,
    )
    .unwrap();
    let mut fallback = Localization::load(&directory);
    fallback.select("ru").unwrap();
    assert_eq!(fallback.text("settings.title"), "Настройки");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn item_owned_translations_follow_language_and_reload_from_equipment_configs() {
    let directory = std::env::temp_dir().join(format!("game-item-locales-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("equipment")).unwrap();
    std::fs::create_dir_all(directory.join("locales")).unwrap();
    std::fs::write(directory.join("equipment/drives.json"), r#"{"drives":[{"id":"enterprise_ssd_960gb","display_name":{"en":"Custom SSD","ru":"Мой SSD"},"desc":{"en":"Custom description"}}]}"#).unwrap();
    let mut localization = Localization::load(&directory.join("locales"));
    let deferred = UiMessage::new("game.saved-path", vec!["/tmp/Save.db".into()]);
    {
        let _scope = localization.enter();
        assert_eq!(item_name("enterprise_ssd_960gb", "fallback"), "Custom SSD");
        assert_eq!(deferred.render(), "Saved to /tmp/Save.db");
    }
    localization.select("ru").unwrap();
    {
        let _scope = localization.enter();
        assert_eq!(item_name("enterprise_ssd_960gb", "fallback"), "Мой SSD");
        assert_eq!(
            item_description("enterprise_ssd_960gb"),
            "Custom description"
        );
        assert_eq!(item_name("missing", "player name"), "player name");
        assert_eq!(deferred.render(), "Сохранено в /tmp/Save.db");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn language_preference_survives_restart_and_failed_saves_preserve_the_language() {
    let directory =
        std::env::temp_dir().join(format!("game-language-settings-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut settings = GameSettings {
        console: ConsoleSettings::default(),
        language: "en".into(),
        localization: Localization::default(),
        console_error: None,
        store: SettingsStore::new(directory.join("settings.json")),
    };
    settings.select_language("ru").unwrap();
    let loaded = settings.store.load().unwrap();
    assert_eq!(loaded.language, "ru");
    assert_eq!(loaded.host, "127.0.0.1");
    assert_eq!(loaded.port, 47655);
    assert_eq!(loaded.password(), "game");
    settings.store = SettingsStore::new(directory.join("missing/settings.json"));
    assert!(settings.select_language("en").is_err());
    assert_eq!(settings.language, "ru");
    assert_eq!(settings.localization.text("game.save"), "Сохранить");
    let legacy: SettingsFile =
        serde_json::from_str(r#"{"host":"127.0.0.1","port":47655,"password":"game"}"#).unwrap();
    assert_eq!(legacy.language, "en");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn placeholder_validation_preserves_utf8_braces_and_rejects_invalid_fields() {
    assert_eq!(fields("{2} — {0} / {1}").unwrap(), [0, 1, 2].into());
    assert!(fields("{0} {1} {2").is_err());
    assert!(fields("{name}").is_err());
    assert!(fields("{0}}").is_err());
    assert_eq!(render("{{{0}}}", &["é".into()]), "{é}");
}

#[test]
fn invalid_settings_and_default_equipment_labels_use_explicit_messages() {
    let mut localization = Localization::default();
    localization.select("ru").unwrap();
    let _scope = localization.enter();
    let mut state = crate::app::SettingsWindowState {
        port: "invalid".into(),
        ..Default::default()
    };
    assert_eq!(
        state.config().unwrap_err().render(),
        "Порт должен быть от 1 до 65535"
    );
    state.port = "47655".into();
    state.host = "invalid".into();
    assert_eq!(
        state.config().unwrap_err().render(),
        "Хост должен быть адресом IPv4/IPv6 или localhost"
    );
    let sim = cloud_provider_sim::NetworkSim::new();
    assert!(!item_description("enterprise_ssd_960gb").is_empty());
    assert!(
        sim.devices()
            .all(|device| !device_name(device).contains("["))
    );
}
