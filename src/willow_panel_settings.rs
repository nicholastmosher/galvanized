use gpui::Pixels;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::{Settings, VsCodeSettings};

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct WillowPanelSettings {
    /// Default width of the Willow panel in pixels
    pub default_width: Option<Pixels>,
    /// Whether to auto-sync data on startup
    pub auto_sync: Option<bool>,
    /// Maximum number of recent documents to show
    pub max_recent_documents: Option<usize>,
    /// Enable debug logging for Willow operations
    pub debug_logging: Option<bool>,
    /// Default namespace to use for new documents
    pub default_namespace: Option<String>,
    /// Auto-save interval in seconds
    pub auto_save_interval: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WillowPanelDockPosition {
    pub dock: WillowDockSide,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum WillowDockSide {
    Left,
    Right,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WillowPanelSettingsContent {
    /// Default width of the Willow panel
    #[serde(default)]
    pub default_width: Option<f32>,
    /// Whether to automatically sync data when the panel opens
    #[serde(default)]
    pub auto_sync: Option<bool>,
    /// Maximum number of recent documents to display
    #[serde(default)]
    pub max_recent_documents: Option<usize>,
    /// Enable verbose logging for debugging
    #[serde(default)]
    pub debug_logging: Option<bool>,
    /// Default namespace for new documents
    #[serde(default)]
    pub default_namespace: Option<String>,
    /// Auto-save interval in seconds (0 to disable)
    #[serde(default)]
    pub auto_save_interval: Option<u64>,
    /// Panel dock position and visibility
    #[serde(default)]
    pub dock: Option<WillowPanelDockPosition>,
}

// impl Settings for WillowPanelSettings {
//     const KEY: Option<&'static str> = Some("willow_panel");

//     type FileContent = WillowPanelSettingsContent;

//     fn load(sources: SettingsSources<Self::FileContent>, _: &mut gpui::App) -> Result<Self> {
//         sources.json_merge()
//     }

//     fn import_from_vscode(vscode: &VsCodeSettings, current: &mut Self::FileContent) {}
// }

impl Default for WillowPanelSettings {
    fn default() -> Self {
        Self {
            default_width: Some(gpui::px(320.0)),
            auto_sync: Some(false),
            max_recent_documents: Some(10),
            debug_logging: Some(false),
            default_namespace: Some("default".to_string()),
            auto_save_interval: Some(30),
        }
    }
}

impl Default for WillowPanelDockPosition {
    fn default() -> Self {
        Self {
            dock: WillowDockSide::Left,
            visible: true,
        }
    }
}

impl From<WillowPanelSettingsContent> for WillowPanelSettings {
    fn from(content: WillowPanelSettingsContent) -> Self {
        Self {
            default_width: content.default_width.map(gpui::px),
            auto_sync: content.auto_sync,
            max_recent_documents: content.max_recent_documents,
            debug_logging: content.debug_logging,
            default_namespace: content.default_namespace,
            auto_save_interval: content.auto_save_interval,
        }
    }
}

/// Configuration options for Willow data store operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WillowStoreConfig {
    /// Path to the local database directory
    pub db_path: String,
    /// Maximum number of entries to cache in memory
    pub cache_size: usize,
    /// Sync timeout in milliseconds
    pub sync_timeout: u64,
    /// Enable compression for stored data
    pub enable_compression: bool,
    /// Maximum file size for documents in bytes
    pub max_document_size: usize,
}

impl Default for WillowStoreConfig {
    fn default() -> Self {
        Self {
            db_path: "willow_db".to_string(),
            cache_size: 1000,
            sync_timeout: 5000,
            enable_compression: true,
            max_document_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// UI display preferences for the Willow panel
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WillowDisplaySettings {
    /// Show file sizes in document list
    pub show_file_sizes: bool,
    /// Show creation dates
    pub show_dates: bool,
    /// Show author information
    pub show_authors: bool,
    /// Default sort order for documents
    pub sort_order: WillowSortOrder,
    /// Group documents by namespace
    pub group_by_namespace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum WillowSortOrder {
    Name,
    Date,
    Size,
    Author,
}

impl Default for WillowDisplaySettings {
    fn default() -> Self {
        Self {
            show_file_sizes: true,
            show_dates: true,
            show_authors: true,
            sort_order: WillowSortOrder::Date,
            group_by_namespace: false,
        }
    }
}

/// Keyboard shortcuts for Willow panel operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WillowKeyBindings {
    /// Toggle panel visibility
    pub toggle_panel: String,
    /// Create new document
    pub create_document: String,
    /// Create new identity
    pub create_identity: String,
    /// Refresh store
    pub refresh: String,
    /// Start sync
    pub sync: String,
}

impl Default for WillowKeyBindings {
    fn default() -> Self {
        Self {
            toggle_panel: "cmd-shift-w".to_string(),
            create_document: "cmd-shift-n".to_string(),
            create_identity: "cmd-shift-i".to_string(),
            refresh: "cmd-r".to_string(),
            sync: "cmd-shift-s".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = WillowPanelSettings::default();
        assert_eq!(settings.default_width, Some(gpui::px(320.0)));
        assert_eq!(settings.auto_sync, Some(false));
        assert_eq!(settings.max_recent_documents, Some(10));
    }

    #[test]
    fn test_settings_serialization() {
        let content = WillowPanelSettingsContent {
            default_width: Some(400.0),
            auto_sync: Some(true),
            max_recent_documents: Some(20),
            debug_logging: Some(true),
            default_namespace: Some("test".to_string()),
            auto_save_interval: Some(60),
            dock: Some(WillowPanelDockPosition {
                dock: WillowDockSide::Right,
                visible: true,
            }),
        };

        let json = serde_json::to_string(&content).expect("Should serialize");
        let deserialized: WillowPanelSettingsContent =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(content.default_width, deserialized.default_width);
        assert_eq!(content.auto_sync, deserialized.auto_sync);
    }

    #[test]
    fn test_store_config_defaults() {
        let config = WillowStoreConfig::default();
        assert_eq!(config.db_path, "willow_db");
        assert_eq!(config.cache_size, 1000);
        assert_eq!(config.sync_timeout, 5000);
        assert!(config.enable_compression);
    }
}
