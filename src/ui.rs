use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::brand::{self, BLUE, MARK, MUTED, NAVY_RAISED, SLATE};
use crate::engine::Engine;
use crate::model::{parse_chord, Action, Document, Hotkey, Macro, Modifier};
use crate::store;
use crate::tray::{Tray, TrayAction};
use crate::windows_host::Session;

use eframe::egui;

enum UiAction {
    None,
    New,
    Delete,
    Save,
}

pub struct KmosApp {
    path: PathBuf,
    document: Document,
    selected: Option<usize>,
    notice: String,
    host: Session,
    report_until: Instant,
    tray: Option<Tray>,
    tray_checked: bool,
    quitting: bool,
    banner: Option<egui::TextureHandle>,
    mark: Option<egui::TextureHandle>,
}

pub fn run(path: PathBuf, document: Document, notice: String) -> Result<(), String> {
    let engine = Engine::from_document(&document)?;
    let host = Session::spawn(engine)?;
    let selected = if document.macros.is_empty() {
        None
    } else {
        Some(0)
    };
    let app = KmosApp {
        path,
        document,
        selected,
        notice,
        host,
        report_until: Instant::now() + Duration::from_millis(600),
        tray: None,
        tray_checked: false,
        quitting: false,
        banner: None,
        mark: None,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([780.0, 520.0])
            .with_icon(brand::window_icon()),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Keyboard Macros OS",
        options,
        Box::new(move |creation| {
            brand::install_visuals(&creation.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| error.to_string())
}

impl eframe::App for KmosApp {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.tray.is_none() && !self.tray_checked {
            self.tray_checked = true;
            match Tray::install() {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => self.notice = error,
            }
        }
        match self.tray.as_ref().and_then(Tray::poll) {
            Some(TrayAction::Open) => show_window(ctx),
            Some(TrayAction::Quit) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {}
        }
        let close_requested = ctx.input(|input| input.viewport().close_requested());
        if close_requested && !self.quitting && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if Instant::now() < self.report_until {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        let mut action = UiAction::None;
        egui::Panel::left("macros")
            .resizable(true)
            .default_size(280.0)
            .frame(side_frame())
            .show(ui, |ui| {
                self.side_heading(ui);
                ui.add_space(8.0);
                ui.heading("Macros");
                ui.horizontal(|ui| {
                    if ui.button("Nueva").clicked() {
                        action = UiAction::New;
                    }
                    if ui.button("Borrar").clicked() {
                        action = UiAction::Delete;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for index in 0..self.document.macros.len() {
                        let shortcut = shortcut_label(&self.document.macros[index].hotkey);
                        let name = self.document.macros[index].name.clone();
                        let selected = self.selected == Some(index);
                        if ui
                            .selectable_label(selected, format!("{name}    {shortcut}"))
                            .clicked()
                        {
                            self.selected = Some(index);
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            self.banner(ui);
            ui.add_space(6.0);
            ui.colored_label(MUTED, self.path.display().to_string());
            ui.separator();
            self.editor(ui);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("Guardar").fill(BLUE)).clicked() {
                    action = UiAction::Save;
                }
                ui.colored_label(
                    MUTED,
                    "El atajo sigue activo al cerrar la ventana. Salir está en la bandeja.",
                );
            });
            ui.add_space(8.0);
            ui.colored_label(MUTED, self.host.report());
            if !self.notice.is_empty() {
                ui.label(&self.notice);
            }
        });
        self.apply(action);
    }
}

fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

fn side_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(NAVY_RAISED)
        .inner_margin(egui::Margin::same(12))
}

impl KmosApp {
    fn side_heading(&mut self, ui: &mut egui::Ui) {
        let mark = self
            .mark
            .get_or_insert_with(|| brand::texture(ui.ctx(), "brand-mark", MARK))
            .clone();
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&mark).fit_to_exact_size(egui::vec2(36.0, 36.0)));
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Keyboard").strong());
                ui.colored_label(BLUE, egui::RichText::new("Macros OS").strong());
            });
        });
    }

    fn banner(&mut self, ui: &mut egui::Ui) {
        let banner = self
            .banner
            .get_or_insert_with(|| brand::texture(ui.ctx(), "brand-banner", brand::LOGO_DARK))
            .clone();
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, SLATE))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.add(
                    egui::Image::new(&banner)
                        .corner_radius(12.0)
                        .max_width(ui.available_width())
                        .max_height(132.0),
                );
            });
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected else {
            ui.label("Crea una macro con Nueva.");
            return;
        };
        let Some(macro_) = self.document.macros.get_mut(index) else {
            ui.label("Crea una macro con Nueva.");
            return;
        };
        egui::Grid::new("campos")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Id");
                ui.text_edit_singleline(&mut macro_.id);
                ui.end_row();
                ui.label("Nombre");
                ui.text_edit_singleline(&mut macro_.name);
                ui.end_row();
                ui.label("Grupo");
                let mut group = macro_.parent.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut group).changed() {
                    macro_.parent = if group.trim().is_empty() {
                        None
                    } else {
                        Some(group)
                    };
                }
                ui.end_row();
                ui.label("Tecla");
                ui.text_edit_singleline(&mut macro_.hotkey.key);
                ui.end_row();
            });
        ui.horizontal(|ui| {
            toggle(ui, &mut macro_.hotkey, Modifier::Ctrl, "Ctrl");
            toggle(ui, &mut macro_.hotkey, Modifier::Shift, "Shift");
            toggle(ui, &mut macro_.hotkey, Modifier::Alt, "Alt");
            toggle(ui, &mut macro_.hotkey, Modifier::Win, "Win");
        });
        match parse_chord(&macro_.hotkey) {
            Ok(chord) => {
                ui.label(format!("Atajo: {}", chord.label()));
            }
            Err(error) => {
                ui.colored_label(egui::Color32::from_rgb(176, 48, 48), error);
            }
        }
        ui.label("Texto");
        let Action::InsertText { text } = &mut macro_.action;
        ui.add(
            egui::TextEdit::multiline(text)
                .desired_rows(12)
                .desired_width(f32::INFINITY),
        );
    }

    fn apply(&mut self, action: UiAction) {
        match action {
            UiAction::None => {}
            UiAction::New => self.add_macro(),
            UiAction::Delete => self.delete_selected(),
            UiAction::Save => self.save_document(),
        }
    }

    fn add_macro(&mut self) {
        let macro_ = fresh_macro(&self.document);
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Completa la macro y pulsa Guardar.".into();
    }

    fn delete_selected(&mut self) {
        let Some(index) = self.selected else {
            self.notice = "Elige una macro.".into();
            return;
        };
        if index >= self.document.macros.len() {
            self.selected = None;
            return;
        }
        self.document.macros.remove(index);
        self.selected = if self.document.macros.is_empty() {
            None
        } else if index >= self.document.macros.len() {
            Some(self.document.macros.len() - 1)
        } else {
            Some(index)
        };
        self.save_document();
        if self.notice.starts_with("Guardado") {
            self.notice = "Macro borrada.".into();
        }
    }

    fn save_document(&mut self) {
        if let Err(error) = store::check(&self.document) {
            self.notice = error;
            return;
        }
        if let Err(error) = store::save(&self.path, &self.document) {
            self.notice = error;
            return;
        }
        match Engine::from_document(&self.document) {
            Ok(engine) => {
                self.host.reload(engine);
                self.report_until = Instant::now() + Duration::from_millis(600);
                self.notice = "Guardado.".into();
            }
            Err(error) => self.notice = error,
        }
    }
}

fn toggle(ui: &mut egui::Ui, hotkey: &mut Hotkey, modifier: Modifier, label: &str) {
    let mut enabled = hotkey.modifiers.contains(&modifier);
    if ui.checkbox(&mut enabled, label).changed() {
        if enabled {
            if !hotkey.modifiers.contains(&modifier) {
                hotkey.modifiers.push(modifier);
            }
        } else {
            hotkey.modifiers.retain(|item| *item != modifier);
        }
    }
}

fn shortcut_label(hotkey: &Hotkey) -> String {
    parse_chord(hotkey)
        .map(|chord| chord.label())
        .unwrap_or_else(|_| hotkey.key.clone())
}

fn fresh_macro(document: &Document) -> Macro {
    Macro {
        id: unique_id(document),
        name: "Nueva macro".into(),
        parent: None,
        hotkey: unused_hotkey(document),
        action: Action::InsertText { text: String::new() },
    }
}

fn unique_id(document: &Document) -> String {
    let mut id = "nueva".to_string();
    let mut number = 2u32;
    while document.macros.iter().any(|macro_| macro_.id == id) {
        id = format!("nueva-{number}");
        number += 1;
    }
    id
}

fn unused_hotkey(document: &Document) -> Hotkey {
    let used: Vec<_> = document
        .macros
        .iter()
        .filter_map(|macro_| parse_chord(&macro_.hotkey).ok())
        .collect();
    let letters = ('a'..='z').map(|letter| letter.to_string());
    let functions = (1..=12).map(|number| format!("f{number}"));
    for key in letters.chain(functions) {
        let hotkey = Hotkey {
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            key,
        };
        if let Ok(chord) = parse_chord(&hotkey) {
            if !used.contains(&chord) {
                return hotkey;
            }
        }
    }
    Hotkey {
        modifiers: vec![Modifier::Ctrl, Modifier::Shift],
        key: "a".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SCHEMA_VERSION;

    #[test]
    fn new_macro_gets_a_free_chord() {
        let document = Document {
            version: SCHEMA_VERSION,
            macros: vec![fresh_macro(&Document {
                version: SCHEMA_VERSION,
                macros: Vec::new(),
            })],
        };
        let another = fresh_macro(&document);
        let first = parse_chord(&document.macros[0].hotkey).unwrap();
        let second = parse_chord(&another.hotkey).unwrap();
        assert_ne!(document.macros[0].id, another.id);
        assert_ne!(first, second);
    }
}
