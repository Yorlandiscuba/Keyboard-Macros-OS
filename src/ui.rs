use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::brand::{self, BLUE, MARK, MUTED, NAVY, NAVY_RAISED, PAPER, SLATE};
use crate::engine::Engine;
use crate::import_hk;
use crate::model::{
    parse_chord, Action, Condition, Delay, Document, Hotkey, InputEvent, Macro, MediaOp, Modifier,
    PowerOp, Repeat, ReplaceMatch, ReplaceRule, ShotDest, ShotSource, Trigger, VolumeOp, WindowOp,
};
use crate::store;
use crate::tray::{Tray, TrayAction};
use crate::windows_host::{Capture, Session};

use eframe::egui;

enum UiAction {
    None,
    New,
    Delete,
    Save,
    Import,
    CaptureKeys,
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
    startup: bool,
    capture: Option<Capture>,
    creator: Option<CreatorDraft>,
}

pub fn run(path: PathBuf, document: Document, notice: String) -> Result<(), String> {
    let engine = Engine::from_document(&document)?;
    let host = Session::spawn(engine, path.clone(), document.variables.clone())?;
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
        startup: startup_enabled(),
        capture: None,
        creator: None,
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
        if let Some(capture) = self.capture.as_ref() {
            match capture.poll() {
                Some(Ok(events)) => {
                    self.capture = None;
                    self.keep_recording(events);
                }
                Some(Err(error)) => {
                    self.capture = None;
                    self.notice = error;
                    self.restore_hotkeys();
                }
                None => ctx.request_repaint_after(Duration::from_millis(50)),
            }
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
                    if ui.button("Importar").clicked() {
                        action = UiAction::Import;
                    }
                });
                ui.separator();
                if self.document.macros.is_empty() {
                    ui.add_space(8.0);
                    ui.colored_label(MUTED, "Pulsa Nueva para elegir el tipo.");
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for index in 0..self.document.macros.len() {
                        let (title, _) = describe_action(&self.document.macros[index].action);
                        let shortcut = shortcut_label(self.document.macros[index].hotkey.as_ref());
                        let name = self.document.macros[index].name.clone();
                        let selected = self.selected == Some(index);
                        if ui
                            .selectable_label(selected, format!("{name}\n{title}  ·  {shortcut}"))
                            .clicked()
                        {
                            self.selected = Some(index);
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            self.banner(ui);
            if self.capture.is_some() {
                recording_banner(ui);
            }
            ui.add_space(6.0);
            ui.colored_label(MUTED, self.path.display().to_string());
            ui.separator();
            let editor_height = (ui.available_height() - 118.0).max(80.0);
            egui::ScrollArea::vertical()
                .max_height(editor_height)
                .show(ui, |ui| {
                    self.editor(ui, &mut action);
                });
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
            startup_toggle(ui, self);
            ui.add_space(8.0);
            ui.colored_label(MUTED, self.host.report());
            if !self.notice.is_empty() {
                ui.label(&self.notice);
            }
        });
        self.apply(action);
        self.creation_modal(ui.ctx());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CreatorKind {
    Text,
    Record,
    OpenFile,
    Launch,
    OpenFolder,
    OpenPage,
    Window,
    Volume,
    Replace,
    Power,
    Media,
    Delay,
    Run,
    Sequence,
    Expression,
    Reminder,
    Screenshot,
    SaveVars,
}

struct CreatorDraft {
    name: String,
    kind: CreatorKind,
}

struct KindRow {
    group: &'static str,
    kind: CreatorKind,
    title: &'static str,
    help: &'static str,
}

const KIND_ROWS: &[KindRow] = &[
    KindRow { group: "Texto", kind: CreatorKind::Text, title: "Pegar texto", help: "Inserta un texto plano donde estés escribiendo." },
    KindRow { group: "Texto", kind: CreatorKind::Replace, title: "Reemplazar", help: "Copia la selección, cambia el texto y lo pega." },
    KindRow { group: "Texto", kind: CreatorKind::Record, title: "Grabación", help: "Reproduce teclas, clics y esperas. Escape termina la grabación." },
    KindRow { group: "Abrir", kind: CreatorKind::OpenFile, title: "Archivo", help: "Abre un archivo con su programa, o con otro que elijas." },
    KindRow { group: "Abrir", kind: CreatorKind::Launch, title: "Programa", help: "Inicia un programa. Puede ir minimizado o como administrador." },
    KindRow { group: "Abrir", kind: CreatorKind::OpenFolder, title: "Carpeta", help: "Abre una carpeta en el Explorador." },
    KindRow { group: "Abrir", kind: CreatorKind::OpenPage, title: "Página", help: "Abre una dirección http, https o ftp." },
    KindRow { group: "Equipo", kind: CreatorKind::Window, title: "Ventana", help: "Cierra, mueve o cambia la ventana que está delante." },
    KindRow { group: "Equipo", kind: CreatorKind::Volume, title: "Volumen", help: "Sube, baja, fija o silencia el audio predeterminado." },
    KindRow { group: "Equipo", kind: CreatorKind::Media, title: "Medios", help: "Pausa, para o cambia de pista con las teclas de medio." },
    KindRow { group: "Equipo", kind: CreatorKind::Power, title: "Energía", help: "Bloquea, cierra la sesión, apaga, reinicia o suspende." },
    KindRow { group: "Equipo", kind: CreatorKind::Screenshot, title: "Captura", help: "Guarda la pantalla, la ventana o un rectángulo." },
    KindRow { group: "Equipo", kind: CreatorKind::Reminder, title: "Aviso", help: "Muestra un recordatorio en una ventana aparte." },
    KindRow { group: "Encadenar", kind: CreatorKind::Delay, title: "Espera", help: "Pausa la cadena, o espera a que aparezca una ventana." },
    KindRow { group: "Encadenar", kind: CreatorKind::Run, title: "Otra macro", help: "Ejecuta la macro del id que indiques." },
    KindRow { group: "Encadenar", kind: CreatorKind::Sequence, title: "Cadena", help: "Ejecuta varias macros, una detrás de otra." },
    KindRow { group: "Encadenar", kind: CreatorKind::Expression, title: "Expresión", help: "Calcula un número y puede guardarlo en una variable." },
    KindRow { group: "Encadenar", kind: CreatorKind::SaveVars, title: "Variables", help: "Guarda valores para que sigan ahí al volver a abrir el programa." },
];

fn describe_action(action: &Action) -> (&'static str, &'static str) {
    let kind = match action {
        Action::InsertText { .. } => Some(CreatorKind::Text),
        Action::Play { .. } => Some(CreatorKind::Record),
        Action::OpenFile { .. } => Some(CreatorKind::OpenFile),
        Action::Launch { .. } => Some(CreatorKind::Launch),
        Action::OpenFolder { .. } => Some(CreatorKind::OpenFolder),
        Action::OpenPage { .. } => Some(CreatorKind::OpenPage),
        Action::Window { .. } => Some(CreatorKind::Window),
        Action::Volume { .. } => Some(CreatorKind::Volume),
        Action::Replace { .. } => Some(CreatorKind::Replace),
        Action::Power { .. } => Some(CreatorKind::Power),
        Action::Media { .. } => Some(CreatorKind::Media),
        Action::Delay { .. } => Some(CreatorKind::Delay),
        Action::Run { .. } => Some(CreatorKind::Run),
        Action::Sequence { .. } => Some(CreatorKind::Sequence),
        Action::Expression { .. } => Some(CreatorKind::Expression),
        Action::Reminder { .. } => Some(CreatorKind::Reminder),
        Action::Screenshot { .. } => Some(CreatorKind::Screenshot),
        Action::SaveVars { .. } => Some(CreatorKind::SaveVars),
        Action::Unconverted { .. } => None,
    };
    kind.and_then(|kind| KIND_ROWS.iter().find(|row| row.kind == kind))
        .map(|row| (row.title, row.help))
        .unwrap_or(("Sin convertir", "Este tipo se guardó y todavía no se ejecuta."))
}

fn kind_card(ui: &mut egui::Ui, selected: bool, title: &str, help: &str) -> bool {
    let width = ui.available_width();
    let color = ui.visuals().text_color();
    let title_galley = ui.painter().layout_no_wrap(title.to_owned(), egui::FontId::proportional(14.0), color);
    let help_galley = ui.painter().layout(help.to_owned(), egui::FontId::proportional(12.5), MUTED, (width - 24.0).max(80.0));
    let title_height = title_galley.size().y;
    let height = 8.0 + title_height + 2.0 + help_galley.size().y + 8.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let fill = if selected || response.hovered() { SLATE } else { NAVY };
    let stroke = egui::Stroke::new(if selected { 1.5 } else { 1.0 }, if selected { BLUE } else { SLATE });
    ui.painter().rect(rect, 8.0, fill, stroke, egui::StrokeKind::Inside);
    let origin = rect.min + egui::vec2(12.0, 8.0);
    ui.painter().galley(origin, title_galley, color);
    ui.painter().galley(origin + egui::vec2(0.0, title_height + 2.0), help_galley, MUTED);
    response.clicked()
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

fn recording_banner(ui: &mut egui::Ui) {
    let count = crate::windows_host::captured_event_count();
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(0x7F, 0x1D, 0x1D))
        .corner_radius(8)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("Grabando eventos  ·  {count}"))
                    .strong()
                    .color(PAPER),
            );
            ui.colored_label(PAPER, "El ratón también se graba. Escape termina y guarda.");
        });
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

    fn editor(&mut self, ui: &mut egui::Ui, action: &mut UiAction) {
        let variables = self.host.variable_snapshot();
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
                let mut key = macro_
                    .hotkey
                    .as_ref()
                    .map(|hotkey| hotkey.key.clone())
                    .unwrap_or_default();
                if ui.text_edit_singleline(&mut key).changed() {
                    ensure_hotkey(macro_).key = key;
                }
                ui.end_row();
            });
        ui.horizontal(|ui| {
            toggle(ui, macro_, Modifier::Ctrl, "Ctrl");
            toggle(ui, macro_, Modifier::Shift, "Shift");
            toggle(ui, macro_, Modifier::Alt, "Alt");
            toggle(ui, macro_, Modifier::Win, "Win");
        });
        match macro_.hotkey.as_ref() {
            Some(hotkey) if !hotkey.key.trim().is_empty() => match parse_chord(hotkey) {
                Ok(chord) => {
                    ui.label(format!("Atajo: {}", chord.label()));
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::from_rgb(176, 48, 48), error);
                }
            },
            _ => {
                ui.label("Sin atajo.");
            }
        }
        let (title, help) = describe_action(&macro_.action);
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title).strong());
        ui.colored_label(MUTED, help);
        ui.add_space(4.0);
        match &mut macro_.action {
            Action::InsertText { text } => {
                ui.label("Texto");
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(8)
                        .desired_width(f32::INFINITY),
                );
            }
            Action::Unconverted { note, .. } => {
                ui.add_space(8.0);
                ui.label(note.as_str());
            }
            Action::Play {
                events,
                speed,
                repeat,
            } => play_editor(ui, events, speed, repeat, action),
            Action::OpenFile {
                path,
                program,
                arguments,
            } => open_file_editor(ui, path, program, arguments),
            Action::Launch {
                program,
                arguments,
                directory,
                minimized,
                activate_existing,
                as_admin,
            } => launch_editor(
                ui,
                program,
                arguments,
                directory,
                minimized,
                activate_existing,
                as_admin,
            ),
            Action::OpenFolder { path } => {
                path_row(ui, "Carpeta", path, "Carpeta", None);
                ui.label("El atajo abre esta carpeta en el Explorador.");
            }
            Action::OpenPage {
                url,
                browser,
                new_window,
            } => page_editor(ui, url, browser, new_window),
            Action::Window { op } => window_editor(ui, op),
            Action::Volume { op } => volume_editor(ui, op),
            Action::Replace {
                rules,
                max_iterations,
                max_length_kb,
            } => replace_editor(ui, rules, max_iterations, max_length_kb),
            Action::Power { op, force } => power_editor(ui, op, force),
            Action::Media { op } => media_editor(ui, op),
            Action::Delay { delay } => delay_editor(ui, delay),
            Action::Run { id } => {
                ui.label("Id de la macro");
                ui.add(egui::TextEdit::singleline(id).desired_width(f32::INFINITY));
            }
            Action::Sequence { ids } => sequence_editor(ui, ids),
            Action::Expression { source, variable } => {
                ui.label("Expresión");
                ui.add(egui::TextEdit::multiline(source).desired_rows(3).desired_width(f32::INFINITY));
                ui.label("Guardar en");
                ui.add(egui::TextEdit::singleline(variable).desired_width(f32::INFINITY));
                ui.label("Números, $variable, + - * /, comparaciones, &&, ||, sqrt y abs.");
            }
            Action::Reminder { title, text } => {
                ui.label("Título");
                ui.add(egui::TextEdit::singleline(title).desired_width(f32::INFINITY));
                ui.label("Aviso");
                ui.add(egui::TextEdit::multiline(text).desired_rows(4).desired_width(f32::INFINITY));
            }
            Action::Screenshot { source, dest, overwrite } => shot_editor(ui, source, dest, overwrite),
            Action::SaveVars { names } => names_editor(ui, names, &variables),
        }
        ui.separator();
        trigger_editor(ui, &mut macro_.trigger);
        condition_editor(ui, &mut macro_.condition);
    }

    fn apply(&mut self, action: UiAction) {
        match action {
            UiAction::None => {}
            UiAction::New => {
                self.creator = Some(CreatorDraft {
                    name: String::new(),
                    kind: CreatorKind::Text,
                });
            }
            UiAction::Delete => self.delete_selected(),
            UiAction::Save => self.save_document(),
            UiAction::Import => self.import_file(),
            UiAction::CaptureKeys => self.begin_recording(),
        }
    }

    fn creation_modal(&mut self, ctx: &egui::Context) {
        if self.creator.is_none() {
            return;
        }
        let mut close = false;
        let mut create = false;
        let id = egui::Id::unique("crear-macro");
        let response = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).default_width(540.0))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .inner_margin(16)
                    .corner_radius(12)
                    .fill(NAVY_RAISED),
            )
            .show(ctx, |ui| {
                let draft = self.creator.as_mut().expect("borrador");
                ui.heading("Nueva macro");
                ui.colored_label(
                    MUTED,
                    "Elige qué va a hacer. El atajo y los detalles se ajustan después.",
                );
                ui.add_space(8.0);
                ui.label("Nombre");
                ui.add(egui::TextEdit::singleline(&mut draft.name).hint_text("Opcional").desired_width(f32::INFINITY));
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    let mut group = "";
                    for row in KIND_ROWS {
                        if row.group != group {
                            group = row.group;
                            ui.add_space(6.0);
                            ui.colored_label(BLUE, egui::RichText::new(row.group).strong());
                            ui.add_space(4.0);
                        }
                        if kind_card(ui, draft.kind == row.kind, row.title, row.help) {
                            draft.kind = row.kind;
                        }
                        ui.add_space(4.0);
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancelar").clicked() {
                        close = true;
                    }
                    if ui.add(egui::Button::new("Crear").fill(BLUE)).clicked() {
                        create = true;
                    }
                });
            });
        if response.should_close() {
            close = true;
        }
        if create {
            let draft = self.creator.take().expect("borrador");
            self.create_kind(draft.kind);
            let name = draft.name.trim();
            if !name.is_empty() {
                if let Some(index) = self.selected {
                    self.document.macros[index].name = name.to_string();
                }
            }
        } else if close {
            self.creator = None;
        }
    }

    fn create_kind(&mut self, kind: CreatorKind) {
        match kind {
            CreatorKind::Text => self.add_macro(),
            CreatorKind::Record => self.add_recording(),
            CreatorKind::OpenFile => self.add_open_file(),
            CreatorKind::Launch => self.add_launch(),
            CreatorKind::OpenFolder => self.add_folder(),
            CreatorKind::OpenPage => self.add_page(),
            CreatorKind::Window => self.add_window(),
            CreatorKind::Volume => self.add_volume(),
            CreatorKind::Replace => self.add_replace(),
            CreatorKind::Power => self.add_power(),
            CreatorKind::Media => self.add_media(),
            CreatorKind::Delay => self.add_delay(),
            CreatorKind::Run => self.add_run(),
            CreatorKind::Sequence => self.add_sequence(),
            CreatorKind::Expression => self.add_expression(),
            CreatorKind::Reminder => self.add_reminder(),
            CreatorKind::Screenshot => self.add_screenshot(),
            CreatorKind::SaveVars => self.add_save_vars(),
        }
    }

    fn begin_recording(&mut self) {
        if self.capture.is_some() {
            self.notice = "Ya hay una grabación en curso. Escape la termina.".into();
            return;
        }
        let is_play = matches!(
            self.selected
                .and_then(|index| self.document.macros.get(index))
                .map(|macro_| &macro_.action),
            Some(Action::Play { .. })
        );
        if !is_play {
            self.notice = "Elige una grabación.".into();
            return;
        }
        match crate::windows_host::start_capture() {
            Ok(capture) => {
                self.capture = Some(capture);
                self.pause_hotkeys();
                self.notice = "Grabando. Pulsa Escape para terminar.".into();
            }
            Err(error) => self.notice = error,
        }
    }

    fn pause_hotkeys(&mut self) {
        let parked = Document {
            version: self.document.version,
            macros: Vec::new(),
            variables: Default::default(),
        };
        if let Ok(engine) = Engine::from_document(&parked) {
            self.host.reload(engine);
            self.report_until = Instant::now() + Duration::from_millis(600);
        }
    }

    fn restore_hotkeys(&mut self) {
        if let Ok(engine) = Engine::from_document(&self.document) {
            self.host.reload(engine);
            self.report_until = Instant::now() + Duration::from_millis(600);
        }
    }

    fn add_recording(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Grabación".into();
        macro_.action = Action::Play {
            events: Vec::new(),
            speed: 100,
            repeat: Repeat::Times { count: 1 },
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Pulsa Grabar y, al terminar, Escape.".into();
    }

    fn add_open_file(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Abrir archivo".into();
        macro_.action = Action::OpenFile {
            path: String::new(),
            program: String::new(),
            arguments: String::new(),
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Elige el archivo y pulsa Guardar.".into();
    }

    fn add_launch(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Ejecutar programa".into();
        macro_.action = Action::Launch {
            program: String::new(),
            arguments: String::new(),
            directory: String::new(),
            minimized: false,
            activate_existing: false,
            as_admin: false,
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Elige el programa y pulsa Guardar.".into();
    }

    fn add_folder(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Abrir carpeta".into();
        macro_.action = Action::OpenFolder {
            path: String::new(),
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Elige la carpeta y pulsa Guardar.".into();
    }

    fn add_page(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Abrir página".into();
        macro_.action = Action::OpenPage {
            url: String::new(),
            browser: String::new(),
            new_window: false,
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Escribe la dirección y pulsa Guardar.".into();
    }

    fn add_window(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Ventana".into();
        macro_.action = Action::Window {
            op: WindowOp::Minimize,
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Elige la operación y pulsa Guardar.".into();
    }

    fn add_volume(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Volumen".into();
        macro_.action = Action::Volume {
            op: VolumeOp::Up { percent: 10 },
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Elige el cambio y pulsa Guardar.".into();
    }

    fn add_replace(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Reemplazar".into();
        macro_.action = Action::Replace {
            rules: vec![empty_rule()],
            max_iterations: 50,
            max_length_kb: 64,
        };
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = "Escribe qué buscar y pulsa Guardar.".into();
    }

    fn add_power(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Apagar".into();
        macro_.action = Action::Power { op: PowerOp::Lock, force: false };
        self.push_new(macro_, "Elige la operación y pulsa Guardar.");
    }

    fn add_media(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Medios".into();
        macro_.action = Action::Media { op: MediaOp::PlayPause };
        self.push_new(macro_, "Elige la tecla de medio y pulsa Guardar.");
    }

    fn add_delay(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Espera".into();
        macro_.action = Action::Delay { delay: Delay::Pause { millis: 500 } };
        self.push_new(macro_, "Elige la espera y pulsa Guardar.");
    }

    fn add_run(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Otra macro".into();
        macro_.action = Action::Run { id: String::new() };
        self.push_new(macro_, "Escribe el id y pulsa Guardar.");
    }

    fn add_sequence(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Cadena".into();
        macro_.action = Action::Sequence { ids: vec![String::new()] };
        self.push_new(macro_, "Escribe los id, uno por línea, y pulsa Guardar.");
    }

    fn add_expression(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Expresión".into();
        macro_.action = Action::Expression { source: String::new(), variable: String::new() };
        self.push_new(macro_, "Escribe la expresión y pulsa Guardar.");
    }

    fn add_reminder(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Aviso".into();
        macro_.action = Action::Reminder { title: String::new(), text: String::new() };
        self.push_new(macro_, "Escribe el aviso y pulsa Guardar.");
    }

    fn add_screenshot(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Captura".into();
        macro_.action = Action::Screenshot {
            source: ShotSource::Screen,
            dest: ShotDest::Clipboard,
            overwrite: false,
        };
        self.push_new(macro_, "Elige la captura y pulsa Guardar.");
    }

    fn add_save_vars(&mut self) {
        let mut macro_ = fresh_macro(&self.document);
        macro_.name = "Variables".into();
        macro_.action = Action::SaveVars { names: vec![String::new()] };
        self.push_new(macro_, "Escribe los nombres y pulsa Guardar.");
    }

    fn push_new(&mut self, macro_: Macro, notice: &str) {
        self.document.macros.push(macro_);
        self.selected = Some(self.document.macros.len() - 1);
        self.notice = notice.into();
    }

    fn keep_recording(&mut self, events: Vec<InputEvent>) {
        let count = events.len();
        let Some(index) = self.selected else {
            self.notice = "No había una grabación seleccionada.".into();
            self.restore_hotkeys();
            return;
        };
        let kept = self.document.macros.get(index).and_then(|macro_| match &macro_.action {
            Action::Play { speed, repeat, .. } => Some((*speed, repeat.clone())),
            _ => None,
        });
        let Some((speed, repeat)) = kept else {
            self.notice = "La grabación no tenía dónde guardarse.".into();
            self.restore_hotkeys();
            return;
        };
        self.document.macros[index].action = Action::Play {
            events,
            speed,
            repeat,
        };
        self.save_document();
        if self.notice.starts_with("Guardado") {
            self.notice = format!("Grabación lista: {count} eventos. Escape también detiene una repetición infinita.");
        } else {
            self.restore_hotkeys();
        }
    }

    fn import_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importar Hot Keyboard")
            .add_filter("Hot Keyboard", &["xml", "hk4backup"])
            .pick_file()
        else {
            return;
        };
        match import_hk::import_path(&path, &mut self.document) {
            Ok(report) => {
                self.selected = if self.document.macros.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.save_document();
                if self.notice.starts_with("Guardado") {
                    self.notice = report.summary();
                }
            }
            Err(error) => self.notice = error,
        }
    }

    fn apply_startup(&mut self, enabled: bool) {
        #[cfg(windows)]
        {
            match crate::startup::set_enabled(enabled, &self.path) {
                Ok(()) => {
                    self.startup = enabled;
                    self.notice = if enabled {
                        "Se abrirá al iniciar Windows. El acceso apunta a este ejecutable y a este archivo.".into()
                    } else {
                        "Ya no se abrirá al iniciar Windows.".into()
                    };
                }
                Err(error) => {
                    self.startup = crate::startup::is_enabled().unwrap_or(false);
                    self.notice = error;
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = enabled;
            self.notice = "El arranque con el sistema todavía no está en esta plataforma.".into();
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
        if self.path.exists() {
            if let Ok(disk) = store::load(&self.path) {
                self.document.variables = disk.variables;
            }
        }
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

fn play_editor(
    ui: &mut egui::Ui,
    events: &mut Vec<InputEvent>,
    speed: &mut u32,
    repeat: &mut Repeat,
    action: &mut UiAction,
) {
    let mut percent = *speed as f32;
    if ui
        .add(egui::Slider::new(&mut percent, 10.0..=400.0).text("% velocidad"))
        .changed()
    {
        *speed = percent.round() as u32;
    }
    let mut forever = matches!(repeat, Repeat::Forever);
    if ui.checkbox(&mut forever, "Repetir hasta Escape").changed() {
        *repeat = if forever {
            Repeat::Forever
        } else {
            Repeat::Times { count: 1 }
        };
    }
    if let Repeat::Times { count } = repeat {
        let mut times = *count;
        if ui
            .add(egui::Slider::new(&mut times, 1..=100).text("repeticiones"))
            .changed()
        {
            *count = times;
        }
    }
    if ui.button("Grabar").clicked() {
        *action = UiAction::CaptureKeys;
    }
    ui.label(format!(
        "{} eventos. Grabar sustituye la secuencia. Escape termina la grabación.",
        events.len()
    ));
    let _ = events;
}

fn open_file_editor(
    ui: &mut egui::Ui,
    path: &mut String,
    program: &mut String,
    arguments: &mut String,
) {
    ui.label("Archivo");
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 88.0).max(80.0);
        ui.add(egui::TextEdit::singleline(path).desired_width(width));
        if ui.button("Elegir").clicked() {
            if let Some(picked) = rfd::FileDialog::new().set_title("Archivo a abrir").pick_file() {
                *path = picked.display().to_string();
            }
        }
    });
    ui.label("Programa");
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 88.0).max(80.0);
        ui.add(
            egui::TextEdit::singleline(program)
                .desired_width(width)
                .hint_text("Vacío: el programa asociado"),
        );
        if ui.button("Elegir").clicked() {
            if let Some(picked) = rfd::FileDialog::new()
                .set_title("Programa")
                .add_filter("Programa", &["exe"])
                .pick_file()
            {
                *program = picked.display().to_string();
            }
        }
    });
    ui.label("Argumentos");
    ui.add(
        egui::TextEdit::singleline(arguments)
            .desired_width(f32::INFINITY)
            .hint_text("Se añaden después de la ruta"),
    );
    ui.label("Con el programa vacío, Windows usa la asociación del archivo.");
}

fn launch_editor(
    ui: &mut egui::Ui,
    program: &mut String,
    arguments: &mut String,
    directory: &mut String,
    minimized: &mut bool,
    activate_existing: &mut bool,
    as_admin: &mut bool,
) {
    path_row(ui, "Programa", program, "Programa", Some(&["exe"]));
    ui.label("Argumentos");
    ui.add(egui::TextEdit::singleline(arguments).desired_width(f32::INFINITY));
    path_row(ui, "Carpeta", directory, "Carpeta de inicio", None);
    ui.checkbox(minimized, "Iniciar minimizado");
    ui.checkbox(
        activate_existing,
        "Si ya está abierto, traer esa ventana al frente",
    );
    ui.checkbox(as_admin, "Ejecutar como administrador");
}

fn page_editor(ui: &mut egui::Ui, url: &mut String, browser: &mut String, new_window: &mut bool) {
    ui.label("Dirección");
    ui.add(
        egui::TextEdit::singleline(url)
            .desired_width(f32::INFINITY)
            .hint_text("https://"),
    );
    path_row(ui, "Navegador", browser, "Navegador", Some(&["exe"]));
    ui.checkbox(new_window, "Nueva ventana");
    ui.label("Sin navegador, Windows abre la dirección. La ventana nueva se pide al programa elegido cuando se reconoce.");
}

const WINDOW_LABELS: &[&str] = &[
    "Cerrar",
    "Minimizar",
    "Maximizar",
    "Restaurar",
    "Minimizar todas",
    "Salvapantallas",
    "Enviar atrás",
    "Vaciar la papelera",
    "Siempre visible",
    "Cerrar el proceso",
    "Mover",
    "Maximizar en un monitor",
    "Activar por título",
];

fn window_index(op: &WindowOp) -> usize {
    match op {
        WindowOp::Close => 0,
        WindowOp::Minimize => 1,
        WindowOp::Maximize => 2,
        WindowOp::Restore => 3,
        WindowOp::MinimizeAll => 4,
        WindowOp::Screensaver => 5,
        WindowOp::SendBack => 6,
        WindowOp::EmptyRecycleBin => 7,
        WindowOp::ToggleTopmost => 8,
        WindowOp::EndProcess => 9,
        WindowOp::Move { .. } => 10,
        WindowOp::MaximizeMonitor { .. } => 11,
        WindowOp::Activate { .. } => 12,
    }
}

fn window_from_index(index: usize) -> WindowOp {
    match index {
        0 => WindowOp::Close,
        2 => WindowOp::Maximize,
        3 => WindowOp::Restore,
        4 => WindowOp::MinimizeAll,
        5 => WindowOp::Screensaver,
        6 => WindowOp::SendBack,
        7 => WindowOp::EmptyRecycleBin,
        8 => WindowOp::ToggleTopmost,
        9 => WindowOp::EndProcess,
        10 => WindowOp::Move {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        },
        11 => WindowOp::MaximizeMonitor { monitor: 1 },
        12 => WindowOp::Activate {
            title: String::new(),
        },
        _ => WindowOp::Minimize,
    }
}

fn window_editor(ui: &mut egui::Ui, op: &mut WindowOp) {
    let mut index = window_index(op);
    egui::ComboBox::from_id_salt("operacion-ventana").show_index(
        ui,
        &mut index,
        WINDOW_LABELS.len(),
        |item| WINDOW_LABELS[item],
    );
    if window_index(op) != index {
        *op = window_from_index(index);
    }
    match op {
        WindowOp::Move {
            left,
            top,
            right,
            bottom,
        } => {
            coordinate(ui, "Izquierda", left);
            coordinate(ui, "Arriba", top);
            coordinate(ui, "Derecha", right);
            coordinate(ui, "Abajo", bottom);
        }
        WindowOp::MaximizeMonitor { monitor } => {
            let mut value = *monitor;
            if ui
                .add(egui::Slider::new(&mut value, 1..=16).text("monitor"))
                .changed()
            {
                *monitor = value;
            }
        }
        WindowOp::Activate { title } => {
            ui.label("Título");
            ui.add(egui::TextEdit::singleline(title).desired_width(f32::INFINITY));
            ui.label("Basta con que el título contenga este texto.");
        }
        WindowOp::MinimizeAll | WindowOp::Screensaver | WindowOp::EmptyRecycleBin => {}
        _ => {
            ui.label("Usa la ventana que esté delante.");
        }
    }
}

fn replace_editor(
    ui: &mut egui::Ui,
    rules: &mut Vec<ReplaceRule>,
    max_iterations: &mut u32,
    max_length_kb: &mut u32,
) {
    ui.horizontal(|ui| {
        ui.label("Vueltas");
        ui.add(egui::DragValue::new(max_iterations).range(1..=500));
        ui.label("KB");
        ui.add(egui::DragValue::new(max_length_kb).range(1..=256));
    });
    if rules.is_empty() {
        rules.push(empty_rule());
    }
    let count = rules.len();
    let mut remove = None;
    for (index, rule) in rules.iter_mut().enumerate() {
        ui.separator();
        ui.label(format!("Regla {}", index + 1));
        ui.label("Buscar");
        ui.add(
            egui::TextEdit::multiline(&mut rule.search)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        ui.label("Reemplazar por");
        ui.add(
            egui::TextEdit::multiline(&mut rule.replace)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        ui.checkbox(&mut rule.regex, "Expresión regular");
        ui.checkbox(&mut rule.ignore_case, "Ignorar mayúsculas");
        ui.checkbox(&mut rule.replace_all, "Todas las coincidencias");
        let labels = ["Parar", "Siguiente", "Desde el inicio"];
        let mut chosen = match rule.on_match {
            ReplaceMatch::Stop => 0,
            ReplaceMatch::Next => 1,
            ReplaceMatch::Restart => 2,
        };
        egui::ComboBox::from_id_salt(("si-coincide", index)).show_index(
            ui,
            &mut chosen,
            labels.len(),
            |item| labels[item],
        );
        rule.on_match = match chosen {
            0 => ReplaceMatch::Stop,
            2 => ReplaceMatch::Restart,
            _ => ReplaceMatch::Next,
        };
        if count > 1 && ui.push_id(index, |ui| ui.button("Quitar").clicked()).inner {
            remove = Some(index);
        }
    }
    if let Some(index) = remove {
        rules.remove(index);
    }
    if rules.len() < 20 && ui.button("Añadir regla").clicked() {
        rules.push(empty_rule());
    }
    ui.label("Copia el texto seleccionado, lo cambia y lo pega.");
}

fn empty_rule() -> ReplaceRule {
    ReplaceRule {
        search: String::new(),
        replace: String::new(),
        regex: false,
        ignore_case: false,
        replace_all: false,
        on_match: ReplaceMatch::Next,
    }
}

fn power_editor(ui: &mut egui::Ui, op: &mut PowerOp, force: &mut bool) {
    let labels = ["Bloquear", "Cerrar sesión", "Apagar", "Reiniciar", "Suspender", "Hibernar"];
    let mut index = match op {
        PowerOp::Lock => 0,
        PowerOp::Logoff => 1,
        PowerOp::Shutdown => 2,
        PowerOp::Restart => 3,
        PowerOp::Sleep => 4,
        PowerOp::Hibernate => 5,
    };
    egui::ComboBox::from_id_salt("operacion-energia").show_index(ui, &mut index, labels.len(), |item| labels[item]);
    *op = match index {
        1 => PowerOp::Logoff,
        2 => PowerOp::Shutdown,
        3 => PowerOp::Restart,
        4 => PowerOp::Sleep,
        5 => PowerOp::Hibernate,
        _ => PowerOp::Lock,
    };
    if matches!(op, PowerOp::Logoff | PowerOp::Shutdown | PowerOp::Restart) {
        ui.checkbox(force, "Forzar el cierre de los programas");
    }
}

fn media_editor(ui: &mut egui::Ui, op: &mut MediaOp) {
    let labels = ["Reproducir o pausar", "Parar", "Siguiente", "Anterior"];
    let mut index = match op {
        MediaOp::PlayPause => 0,
        MediaOp::Stop => 1,
        MediaOp::Next => 2,
        MediaOp::Previous => 3,
    };
    egui::ComboBox::from_id_salt("operacion-medios").show_index(ui, &mut index, labels.len(), |item| labels[item]);
    *op = match index {
        1 => MediaOp::Stop,
        2 => MediaOp::Next,
        3 => MediaOp::Previous,
        _ => MediaOp::PlayPause,
    };
    ui.label("Usa las teclas de medio de Windows.");
}

fn delay_editor(ui: &mut egui::Ui, delay: &mut Delay) {
    let mut timed = matches!(delay, Delay::Pause { .. });
    let mut index = if timed { 0 } else { 1 };
    egui::ComboBox::from_id_salt("tipo-espera").show_index(ui, &mut index, 2, |item| ["Tiempo", "Ventana"][item]);
    if index == 0 && !timed {
        *delay = Delay::Pause { millis: 500 };
        timed = true;
    }
    if index == 1 && timed {
        *delay = Delay::UntilWindow { title: String::new(), millis: 10_000 };
    }
    match delay {
        Delay::Pause { millis } => millis_row(ui, millis),
        Delay::UntilWindow { title, millis } => {
            ui.label("Título");
            ui.add(egui::TextEdit::singleline(title).desired_width(f32::INFINITY));
            millis_row(ui, millis);
        }
    }
}

fn millis_row(ui: &mut egui::Ui, millis: &mut u32) {
    let mut value = *millis;
    ui.horizontal(|ui| {
        ui.label("Milisegundos");
        if ui.add(egui::DragValue::new(&mut value).range(0..=600_000)).changed() {
            *millis = value;
        }
    });
}

fn sequence_editor(ui: &mut egui::Ui, ids: &mut Vec<String>) {
    let mut text = ids.join("\n");
    ui.label("Ids, uno por línea");
    if ui.add(egui::TextEdit::multiline(&mut text).desired_rows(4).desired_width(f32::INFINITY)).changed() {
        *ids = text.lines().map(|line| line.trim().to_string()).collect();
        if ids.is_empty() {
            ids.push(String::new());
        }
    }
}

fn names_editor(ui: &mut egui::Ui, names: &mut Vec<String>, variables: &std::collections::BTreeMap<String, String>) {
    let mut text = names.join("\n");
    ui.label("Nombres, uno por línea");
    if ui.add(egui::TextEdit::multiline(&mut text).desired_rows(4).desired_width(f32::INFINITY)).changed() {
        *names = text.lines().map(|line| line.trim().to_string()).collect();
        if names.is_empty() {
            names.push(String::new());
        }
    }
    ui.label("Al ejecutarse, esos valores quedan en el archivo para el próximo arranque.");
    if variables.is_empty() {
        ui.label("Todavía no hay variables en esta sesión.");
    } else {
        for (name, value) in variables {
            ui.label(format!("{name} = {value}"));
        }
    }
}

fn shot_editor(ui: &mut egui::Ui, source: &mut ShotSource, dest: &mut ShotDest, overwrite: &mut bool) {
    let mut source_index = match source {
        ShotSource::Screen => 0,
        ShotSource::Window => 1,
        ShotSource::Region { .. } => 2,
    };
    egui::ComboBox::from_id_salt("origen-captura").show_index(ui, &mut source_index, 3, |item| ["Pantalla", "Ventana", "Rectángulo"][item]);
    if source_index != match source {
        ShotSource::Screen => 0,
        ShotSource::Window => 1,
        ShotSource::Region { .. } => 2,
    } {
        *source = match source_index {
            1 => ShotSource::Window,
            2 => ShotSource::Region { left: 0, top: 0, right: 800, bottom: 600 },
            _ => ShotSource::Screen,
        };
    }
    if let ShotSource::Region { left, top, right, bottom } = source {
        coordinate(ui, "Izquierda", left);
        coordinate(ui, "Arriba", top);
        coordinate(ui, "Derecha", right);
        coordinate(ui, "Abajo", bottom);
    }
    let mut file = matches!(dest, ShotDest::File { .. });
    if ui.checkbox(&mut file, "Guardar en un archivo BMP").changed() {
        *dest = if file {
            ShotDest::File { path: String::new() }
        } else {
            ShotDest::Clipboard
        };
    }
    if let ShotDest::File { path } = dest {
        path_row(ui, "Ruta", path, "Captura", Some(&["bmp"]));
        ui.checkbox(overwrite, "Sustituir si ya existe");
    }
}

fn trigger_editor(ui: &mut egui::Ui, trigger: &mut Option<Trigger>) {
    let labels = ["Ninguno", "Texto", "Gesto", "Pulsaciones", "Agenda", "Arranque", "Ventana"];
    let mut index = match trigger {
        None => 0,
        Some(Trigger::Text { .. }) => 1,
        Some(Trigger::Gesture { .. }) => 2,
        Some(Trigger::Multitap { .. }) => 3,
        Some(Trigger::Schedule { .. }) => 4,
        Some(Trigger::Startup) => 5,
        Some(Trigger::Window { .. }) => 6,
    };
    ui.label("Además del atajo");
    egui::ComboBox::from_id_salt("disparador").show_index(ui, &mut index, labels.len(), |item| labels[item]);
    if trigger_index(trigger) != index {
        *trigger = match index {
            1 => Some(Trigger::Text { phrase: String::new() }),
            2 => Some(Trigger::Gesture { directions: "R".into() }),
            3 => Some(Trigger::Multitap { key: "a".into(), count: 3 }),
            4 => Some(Trigger::Schedule { hour: 9, minute: 0 }),
            5 => Some(Trigger::Startup),
            6 => Some(Trigger::Window { title: String::new() }),
            _ => None,
        };
    }
    match trigger {
        Some(Trigger::Text { phrase }) => {
            ui.add(egui::TextEdit::singleline(phrase).hint_text("frase").desired_width(f32::INFINITY));
        }
        Some(Trigger::Gesture { directions }) => {
            ui.add(egui::TextEdit::singleline(directions).hint_text("UDLR").desired_width(f32::INFINITY));
            ui.label("U arriba, D abajo, L izquierda, R derecha, con el botón izquierdo.");
        }
        Some(Trigger::Multitap { key, count }) => {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(key).desired_width(80.0));
                let mut value = *count;
                if ui.add(egui::DragValue::new(&mut value).range(2..=8)).changed() {
                    *count = value;
                }
            });
        }
        Some(Trigger::Schedule { hour, minute }) => {
            ui.horizontal(|ui| {
                let mut hour_value = *hour;
                let mut minute_value = *minute;
                if ui.add(egui::DragValue::new(&mut hour_value).range(0..=23)).changed() {
                    *hour = hour_value;
                }
                ui.label(":");
                if ui.add(egui::DragValue::new(&mut minute_value).range(0..=59)).changed() {
                    *minute = minute_value;
                }
            });
        }
        Some(Trigger::Window { title }) => {
            ui.add(egui::TextEdit::singleline(title).desired_width(f32::INFINITY));
        }
        Some(Trigger::Startup) => {
            ui.label("Se ejecuta al abrir el programa, no al iniciar Windows.");
        }
        None => {}
    }
}

fn trigger_index(trigger: &Option<Trigger>) -> usize {
    match trigger {
        None => 0,
        Some(Trigger::Text { .. }) => 1,
        Some(Trigger::Gesture { .. }) => 2,
        Some(Trigger::Multitap { .. }) => 3,
        Some(Trigger::Schedule { .. }) => 4,
        Some(Trigger::Startup) => 5,
        Some(Trigger::Window { .. }) => 6,
    }
}

fn condition_editor(ui: &mut egui::Ui, condition: &mut Option<Condition>) {
    let labels = ["Ninguna", "Título", "Clase", "Proceso", "Expresión"];
    let mut index = match condition {
        None => 0,
        Some(Condition::Title { .. }) => 1,
        Some(Condition::Class { .. }) => 2,
        Some(Condition::Process { .. }) => 3,
        Some(Condition::Expression { .. }) => 4,
    };
    ui.label("Condición");
    egui::ComboBox::from_id_salt("condicion").show_index(ui, &mut index, labels.len(), |item| labels[item]);
    if condition_index(condition) != index {
        *condition = match index {
            1 => Some(Condition::Title { text: String::new(), invert: false }),
            2 => Some(Condition::Class { text: String::new(), invert: false }),
            3 => Some(Condition::Process { text: String::new(), invert: false }),
            4 => Some(Condition::Expression { source: String::new(), invert: false }),
            _ => None,
        };
    }
    match condition {
        Some(Condition::Title { text, invert })
        | Some(Condition::Class { text, invert })
        | Some(Condition::Process { text, invert }) => {
            ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
            ui.checkbox(invert, "Invertir");
            ui.label("Varias opciones se separan con punto y coma.");
        }
        Some(Condition::Expression { source, invert }) => {
            ui.add(egui::TextEdit::singleline(source).desired_width(f32::INFINITY));
            ui.checkbox(invert, "Invertir");
        }
        None => {}
    }
}

fn condition_index(condition: &Option<Condition>) -> usize {
    match condition {
        None => 0,
        Some(Condition::Title { .. }) => 1,
        Some(Condition::Class { .. }) => 2,
        Some(Condition::Process { .. }) => 3,
        Some(Condition::Expression { .. }) => 4,
    }
}

fn volume_editor(ui: &mut egui::Ui, op: &mut VolumeOp) {
    let labels = ["Subir", "Bajar", "Fijar", "Silencio"];
    let mut index = match op {
        VolumeOp::Up { .. } => 0,
        VolumeOp::Down { .. } => 1,
        VolumeOp::Set { .. } => 2,
        VolumeOp::ToggleMute => 3,
    };
    egui::ComboBox::from_id_salt("operacion-volumen").show_index(
        ui,
        &mut index,
        labels.len(),
        |item| labels[item],
    );
    if volume_index(op) != index {
        *op = match index {
            1 => VolumeOp::Down { percent: 10 },
            2 => VolumeOp::Set { percent: 50 },
            3 => VolumeOp::ToggleMute,
            _ => VolumeOp::Up { percent: 10 },
        };
    }
    match op {
        VolumeOp::ToggleMute => {
            ui.label("Alterna el silencio.");
        }
        VolumeOp::Set { percent } => volume_percent(ui, percent, 0),
        VolumeOp::Up { percent } | VolumeOp::Down { percent } => volume_percent(ui, percent, 1),
    }
    ui.label("Usa el dispositivo de reproducción predeterminado.");
}

fn volume_index(op: &VolumeOp) -> usize {
    match op {
        VolumeOp::Up { .. } => 0,
        VolumeOp::Down { .. } => 1,
        VolumeOp::Set { .. } => 2,
        VolumeOp::ToggleMute => 3,
    }
}

fn volume_percent(ui: &mut egui::Ui, percent: &mut u32, minimum: u32) {
    let mut value = *percent;
    if ui
        .add(egui::Slider::new(&mut value, minimum..=100).text("%"))
        .changed()
    {
        *percent = value;
    }
}

fn coordinate(ui: &mut egui::Ui, label: &str, value: &mut i32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(-32_000..=32_000));
    });
}

fn path_row(ui: &mut egui::Ui, label: &str, value: &mut String, title: &str, filter: Option<&[&str]>) {
    ui.label(label);
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 88.0).max(80.0);
        ui.add(egui::TextEdit::singleline(value).desired_width(width));
        if ui.button("Elegir").clicked() {
            let mut dialog = rfd::FileDialog::new().set_title(title);
            let picked = if let Some(extensions) = filter {
                dialog = dialog.add_filter(title, extensions);
                dialog.pick_file()
            } else {
                dialog.pick_folder()
            };
            if let Some(picked) = picked {
                *value = picked.display().to_string();
            }
        }
    });
}

fn toggle(ui: &mut egui::Ui, macro_: &mut Macro, modifier: Modifier, label: &str) {
    let mut enabled = macro_
        .hotkey
        .as_ref()
        .is_some_and(|hotkey| hotkey.modifiers.contains(&modifier));
    if ui.checkbox(&mut enabled, label).changed() {
        let hotkey = ensure_hotkey(macro_);
        if enabled {
            if !hotkey.modifiers.contains(&modifier) {
                hotkey.modifiers.push(modifier);
            }
        } else {
            hotkey.modifiers.retain(|item| *item != modifier);
        }
    }
}

fn ensure_hotkey(macro_: &mut Macro) -> &mut Hotkey {
    if macro_.hotkey.is_none() {
        macro_.hotkey = Some(Hotkey {
            modifiers: Vec::new(),
            key: String::new(),
        });
    }
    macro_.hotkey.as_mut().expect("atajo")
}

fn shortcut_label(hotkey: Option<&Hotkey>) -> String {
    let Some(hotkey) = hotkey else {
        return "sin atajo".into();
    };
    parse_chord(hotkey)
        .map(|chord| chord.label())
        .unwrap_or_else(|_| hotkey.key.clone())
}

fn startup_toggle(ui: &mut egui::Ui, app: &mut KmosApp) {
    let mut enabled = app.startup;
    if ui.checkbox(&mut enabled, "Iniciar con Windows").changed() {
        app.apply_startup(enabled);
    }
}

fn startup_enabled() -> bool {
    #[cfg(windows)]
    {
        crate::startup::is_enabled().unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn fresh_macro(document: &Document) -> Macro {
    Macro {
        id: unique_id(document),
        name: "Nueva macro".into(),
        parent: None,
        hotkey: Some(unused_hotkey(document)),
        trigger: None,
        condition: None,
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
        .filter_map(|macro_| macro_.hotkey.as_ref())
        .filter_map(|hotkey| parse_chord(hotkey).ok())
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
                variables: Default::default(),
            })],
            variables: Default::default(),
        };
        let another = fresh_macro(&document);
        let first = parse_chord(document.macros[0].hotkey.as_ref().unwrap()).unwrap();
        let second = parse_chord(another.hotkey.as_ref().unwrap()).unwrap();
        assert_ne!(document.macros[0].id, another.id);
        assert_ne!(first, second);
    }
}
