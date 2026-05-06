use image::ImageDecoder;
use tauri::{
  menu::{Menu, MenuItem, PredefinedMenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
  AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
  build_tray_menu(app)?;
  Ok(())
}

fn load_tray_icon() -> tauri::image::Image<'static> {
  // Load PNG from bytes and convert to RGBA
  let png_data = include_bytes!("../icons/logo-tray-32.png");
  let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(png_data)).unwrap();
  let (width, height) = decoder.dimensions();
  let mut rgba_vec = vec![0u8; (width * height * 4) as usize];

  {
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(png_data)).unwrap();
    ImageDecoder::read_image(decoder, rgba_vec.as_mut_slice()).unwrap();
  }

  tauri::image::Image::new_owned(rgba_vec, width, height)
}

fn build_tray_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
  // Create menu items
  let open_item = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
  let do_something_item =
    MenuItem::with_id(app, "do_something", "Do Something...", true, None::<&str>)?;
  let separator = PredefinedMenuItem::separator(app)?;
  let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

  // Build menu
  let menu = Menu::with_items(
    app,
    &[&open_item, &do_something_item, &separator, &quit_item],
  )?;

  // Load tray icon
  let tray_icon = load_tray_icon();

  // Create tray icon (only created once at startup)
  let _tray = TrayIconBuilder::new()
    .icon(tray_icon)
    .icon_as_template(true)
    .menu(&menu)
    .show_menu_on_left_click(true)
    .on_menu_event(|app, event| match event.id.as_ref() {
      "quit" => {
        app.exit(0);
      },
      "open" => {
        show_main_window(app);
      },
      "do_something" => {
        tracing::info!("do_something clicked");
      },
      _ => {},
    })
    .on_tray_icon_event(|tray, event| {
      if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
      } = event
      {
        let app = tray.app_handle();
        show_main_window(app);
      }
    })
    .build(app)?;

  Ok(())
}

fn show_main_window(app: &AppHandle) {
  if let Some(window) = app.get_webview_window("main") {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
  }
}

// Example: open a specific view by emitting an event to the frontend.
// Uncomment and adapt when you need a menu item to navigate to a page or open a modal.
//
// fn show_main_window_with_view(app: &AppHandle) {
//   if let Some(window) = app.get_webview_window("main") {
//     let _ = window.unminimize();
//     let _ = window.show();
//     let _ = window.set_focus();
//
//     // Emit an event that the frontend listens to, e.g. to open a modal or navigate to a route.
//     let _ = window.emit("open-some-view", ());
//   }
// }
