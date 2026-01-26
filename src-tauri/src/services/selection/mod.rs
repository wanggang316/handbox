
pub mod manager;
pub mod menu_panel;
pub mod content_panel;

pub use manager::setup_selection;
pub use menu_panel::show_panel as show_menu_panel;
pub use menu_panel::hide_panel as hide_menu_panel;
pub use content_panel::show_panel as show_content_panel;
pub use content_panel::hide_panel as hide_content_panel;
pub use content_panel::set_panel_pinned as set_content_panel_pinned;
pub use content_panel::is_panel_pinned as is_content_panel_pinned;