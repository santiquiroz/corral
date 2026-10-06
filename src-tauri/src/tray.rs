use crate::snapshot::{OllamaState, Snapshot, TrayStatus};
use crate::state::AppState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "corral";
const ICON_SIZE: u32 = 32;
const TOOLTIP_MAX: usize = 127;

pub fn status_color(status: TrayStatus) -> [u8; 3] {
    match status {
        TrayStatus::Running => [0x2e, 0xcc, 0x71],
        TrayStatus::Spilling => [0xf5, 0xa6, 0x23],
        TrayStatus::Paused => [0x8a, 0x93, 0x9b],
        TrayStatus::Down => [0xe7, 0x4c, 0x3c],
    }
}

pub fn icon_rgba(status: TrayStatus, size: u32) -> Vec<u8> {
    let [r, g, b] = status_color(status);
    let center = size as f32 / 2.0;
    let radius = center - 2.0;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let distance = ((x as f32 + 0.5 - center).powi(2) + (y as f32 + 0.5 - center).powi(2)).sqrt();
            let alpha = ((radius + 0.5 - distance).clamp(0.0, 1.0) * 255.0) as u8;
            pixels.extend_from_slice(&[r, g, b, alpha]);
        }
    }
    pixels
}

pub fn model_lines(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .loaded
        .as_ok()
        .map(|models| models.iter().map(|m| format!("{} · {:.1} GB", m.name, m.vram_mb as f64 / 1024.0)).collect())
        .unwrap_or_default()
}

pub fn tooltip(snapshot: &Snapshot) -> String {
    let state = match snapshot.status {
        TrayStatus::Running => "corriendo",
        TrayStatus::Spilling => "desbordando VRAM",
        TrayStatus::Paused => "en pausa",
        TrayStatus::Down => "caído",
    };
    let mut text = format!("Corral · {state}");
    for line in model_lines(snapshot) {
        text.push('\n');
        text.push_str(&line);
    }
    text.chars().take(TOOLTIP_MAX).collect()
}

pub fn toggle_label(state: OllamaState) -> &'static str {
    match state {
        OllamaState::Running => "Pausar Ollama",
        OllamaState::Paused { .. } | OllamaState::Down => "Reanudar Ollama",
    }
}

fn icon_image(status: TrayStatus) -> Image<'static> {
    Image::new_owned(icon_rgba(status, ICON_SIZE), ICON_SIZE, ICON_SIZE)
}

fn build_menu(app: &AppHandle, snapshot: Option<&Snapshot>) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    for line in snapshot.map(model_lines).unwrap_or_default() {
        menu.append(&MenuItem::new(app, line, false, None::<&str>)?)?;
    }
    if snapshot.is_some_and(|s| !model_lines(s).is_empty()) {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    let state = snapshot.map(|s| s.state).unwrap_or(OllamaState::Down);
    menu.append(&MenuItem::with_id(app, "toggle", toggle_label(state), true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "open", "Abrir panel", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?)?;
    Ok(menu)
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon_image(TrayStatus::Down))
        .tooltip("Corral")
        .menu(&build_menu(app, None)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_panel(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn update(app: &AppHandle, snapshot: &Snapshot) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let _ = tray.set_icon(Some(icon_image(snapshot.status)));
    let _ = tray.set_tooltip(Some(tooltip(snapshot)));
    if let Ok(menu) = build_menu(app, Some(snapshot)) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_panel(app),
        "quit" => app.exit(0),
        "toggle" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let running = state.latest_state() == Some(OllamaState::Running);
                let _ = if running { crate::commands::do_pause(&state).await.map(|_| ()) } else { crate::commands::do_resume(&state).await.map(|_| ()) };
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Field, LoadedModel, OllamaState, PausedBy, Snapshot, TrayStatus};

    fn snap(state: OllamaState, status: TrayStatus, models: Vec<(&str, u64)>) -> Snapshot {
        let loaded = models.into_iter().map(|(name, vram)| LoadedModel {
            name: name.into(), digest: String::new(), size_mb: vram, vram_mb: vram, context_length: 32768, expires_at: String::new(),
        }).collect();
        Snapshot { taken_at_ms: 0, state, status, version: Some("0.35.1".into()), adapters: Field::Ok(vec![]), runners: Field::Ok(vec![]), loaded: Field::Ok(loaded) }
    }

    #[test]
    fn icon_is_square_rgba_with_status_color_in_the_center() {
        let rgba = icon_rgba(TrayStatus::Spilling, 32);
        assert_eq!(rgba.len(), 32 * 32 * 4);
        let center = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(&rgba[center..center + 3], &status_color(TrayStatus::Spilling));
        assert_eq!(rgba[3], 0, "la esquina debe ser transparente");
    }

    #[test]
    fn statuses_have_distinct_colors() {
        let colors = [TrayStatus::Running, TrayStatus::Spilling, TrayStatus::Paused, TrayStatus::Down].map(status_color);
        for i in 0..colors.len() {
            for j in i + 1..colors.len() {
                assert_ne!(colors[i], colors[j]);
            }
        }
    }

    #[test]
    fn tooltip_and_menu_lines_describe_loaded_models() {
        let s = snap(OllamaState::Running, TrayStatus::Running, vec![("qwen3.5-mem:latest", 6279)]);
        assert_eq!(model_lines(&s), vec!["qwen3.5-mem:latest · 6.1 GB".to_string()]);
        assert!(tooltip(&s).starts_with("Corral · corriendo"));
        let many = snap(OllamaState::Running, TrayStatus::Running, (0..20).map(|_| ("modelo-con-un-nombre-largo:latest", 1000)).collect());
        assert!(tooltip(&many).chars().count() <= 127);
    }

    #[test]
    fn toggle_label_follows_state() {
        assert_eq!(toggle_label(OllamaState::Running), "Pausar Ollama");
        assert_eq!(toggle_label(OllamaState::Paused { by: PausedBy::User }), "Reanudar Ollama");
        assert_eq!(toggle_label(OllamaState::Down), "Reanudar Ollama");
    }
}
