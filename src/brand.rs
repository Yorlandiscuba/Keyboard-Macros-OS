use eframe::egui;
use eframe::egui::Color32;

pub const LOGO_DARK: &[u8] = include_bytes!("../branding/png/logo-dark-background.png");
pub const MARK: &[u8] = include_bytes!("../branding/png/app-icon-dark-64.png");
pub const WINDOW_ICON: &[u8] = include_bytes!("../branding/png/app-icon-dark-256.png");
pub const TRAY_FOR_DARK_BAR: &[u8] = include_bytes!("../branding/tray/tray-light-32.png");
pub const TRAY_FOR_LIGHT_BAR: &[u8] = include_bytes!("../branding/tray/tray-dark-32.png");

pub const NAVY: Color32 = Color32::from_rgb(0x0F, 0x17, 0x2A);
pub const NAVY_RAISED: Color32 = Color32::from_rgb(0x16, 0x24, 0x3B);
pub const SLATE: Color32 = Color32::from_rgb(0x1E, 0x29, 0x3B);
pub const BLUE: Color32 = Color32::from_rgb(0x2F, 0x80, 0xED);
pub const PAPER: Color32 = Color32::from_rgb(0xF8, 0xFA, 0xFC);
pub const MUTED: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);

pub fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(WINDOW_ICON).expect("icono de la ventana")
}

pub fn rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let icon = eframe::icon_data::from_png_bytes(bytes).expect("png de la marca");
    (icon.width, icon.height, icon.rgba)
}

pub fn texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let (width, height, pixels) = rgba(bytes);
    let image =
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
    ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
}

pub fn install_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = NAVY;
    visuals.window_fill = NAVY;
    visuals.extreme_bg_color = SLATE;
    visuals.faint_bg_color = NAVY_RAISED;
    visuals.code_bg_color = SLATE;
    visuals.hyperlink_color = BLUE;
    visuals.override_text_color = Some(PAPER);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0x2F, 0x80, 0xED, 90);
    visuals.selection.stroke.color = BLUE;
    let line = egui::Stroke::new(1.0, SLATE);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.fg_stroke.color = PAPER;
        widget.bg_stroke = line;
    }
    visuals.widgets.noninteractive.bg_stroke = line;
    visuals.weak_text_color = Some(MUTED);
    let raised = Color32::from_rgb(0x33, 0x45, 0x62);
    visuals.widgets.inactive.bg_fill = SLATE;
    visuals.widgets.inactive.weak_bg_fill = SLATE;
    visuals.widgets.hovered.bg_fill = raised;
    visuals.widgets.hovered.weak_bg_fill = raised;
    visuals.widgets.hovered.bg_stroke.color = BLUE;
    visuals.widgets.active.bg_fill = BLUE;
    visuals.widgets.active.weak_bg_fill = BLUE;
    visuals.widgets.active.fg_stroke.color = PAPER;
    visuals.widgets.open.bg_fill = SLATE;
    visuals.widgets.open.weak_bg_fill = SLATE;
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
}

pub fn taskbar_is_light() -> bool {
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
    };
    use windows::core::PCWSTR;

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let name: Vec<u16> = "SystemUsesLightTheme\0".encode_utf16().collect();
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    status.is_ok() && value != 0
}
