use voxcode::config::AppConfig;

#[test]
fn default_config_has_monitor_zero() {
    let config = AppConfig::default();
    assert_eq!(config.selected_monitor, 0);
}

#[test]
fn config_round_trip_serialization() {
    let config = AppConfig {
        selected_monitor: 2,
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.selected_monitor, 2);
}

#[test]
fn config_deserialize_missing_fields_uses_default() {
    let json = "{}";
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.selected_monitor, 0);
}

#[test]
fn config_deserialize_invalid_json_returns_default() {
    let result: Result<AppConfig, _> = serde_json::from_str("not json");
    assert!(result.is_err());
    // In the app, load() falls back to default on error
    let config = result.unwrap_or_default();
    assert_eq!(config.selected_monitor, 0);
}
