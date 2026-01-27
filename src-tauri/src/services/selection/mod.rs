
pub mod manager;
pub mod menu_panel;
pub mod content_panel;
pub mod settings_panel;
pub mod settings_disable_panel;

pub use manager::setup_selection;
pub use menu_panel::show_panel as show_menu_panel;
pub use menu_panel::hide_panel as hide_menu_panel;
pub use content_panel::show_panel as show_content_panel;
pub use content_panel::hide_panel as hide_content_panel;
pub use content_panel::set_panel_pinned as set_content_panel_pinned;
pub use content_panel::is_panel_pinned as is_content_panel_pinned;
pub use settings_panel::show_panel as show_settings_panel;
pub use settings_panel::hide_panel as hide_settings_panel;
pub use settings_panel::is_mouse_inside as is_settings_panel_mouse_inside;
pub use settings_disable_panel::show_panel as show_settings_disable_panel;
pub use settings_disable_panel::hide_panel as hide_settings_disable_panel;
pub use settings_disable_panel::is_mouse_inside as is_settings_disable_panel_mouse_inside;