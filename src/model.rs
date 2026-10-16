use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use crate::logic::{
    any_part_matches, check_condition, check_delay, check_expression_action, check_names,
    check_reminder, check_sequence, check_shot, check_target_id, check_trigger, check_variable_name,
    eval_expression, format_value, media_virtual_key, power_flags, Condition, Delay, MediaOp, PowerOp,
    ShotDest, ShotSource, Trigger, CHAIN_LIMIT,
};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub macros: Vec<Macro>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<Hotkey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Trigger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Win,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    InsertText { text: String },
    /// Cuerpo guardado de otro programa que este motor todavía no ejecuta.
    Unconverted {
        kind: String,
        note: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blob: Option<String>,
    },
    /// Secuencia propia de teclado y ratón. No es el binario de otro programa.
    Play {
        events: Vec<InputEvent>,
        #[serde(default = "default_speed")]
        speed: u32,
        #[serde(default)]
        repeat: Repeat,
    },
    /// Abre un archivo con el programa asociado, o con uno indicado.
    OpenFile {
        path: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        program: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        arguments: String,
    },
    /// Inicia un programa. No abre un documento con su asociación.
    Launch {
        program: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        arguments: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        directory: String,
        #[serde(default, skip_serializing_if = "is_false")]
        minimized: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        activate_existing: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        as_admin: bool,
    },
    /// Abre una carpeta en el explorador de archivos.
    OpenFolder { path: String },
    /// Abre una dirección en el navegador asociado, o en uno indicado.
    OpenPage {
        url: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        browser: String,
        #[serde(default, skip_serializing_if = "is_false")]
        new_window: bool,
    },
    /// Cambia la ventana activa, o una ventana elegida por su título.
    Window {
        #[serde(flatten)]
        op: WindowOp,
    },
    /// Cambia el volumen del dispositivo de reproducción predeterminado.
    Volume {
        #[serde(flatten)]
        op: VolumeOp,
    },
    /// Copia la selección, aplica reglas y pega el resultado.
    Replace {
        rules: Vec<ReplaceRule>,
        #[serde(default = "default_iterations", skip_serializing_if = "is_default_iterations")]
        max_iterations: u32,
        #[serde(default = "default_length_kb", skip_serializing_if = "is_default_length")]
        max_length_kb: u32,
    },
    /// Pide a Windows apagar, reiniciar, cerrar la sesión, bloquear, suspender o hibernar.
    Power {
        op: PowerOp,
        #[serde(default, skip_serializing_if = "is_false")]
        force: bool,
    },
    /// Envía una tecla de medio del sistema.
    Media {
        op: MediaOp,
    },
    /// Espera un tiempo, o a que aparezca una ventana.
    Delay {
        #[serde(flatten)]
        delay: Delay,
    },
    /// Ejecuta otra macro. La cadena se corta al llegar al límite.
    Run { id: String },
    /// Ejecuta varias macros en orden.
    Sequence { ids: Vec<String> },
    /// Calcula una expresión y puede guardarla en una variable.
    Expression {
        source: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        variable: String,
    },
    /// Muestra un aviso.
    Reminder {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        title: String,
        text: String,
    },
    /// Copia la pantalla, la ventana activa o un rectángulo.
    Screenshot {
        source: ShotSource,
        dest: ShotDest,
        #[serde(default, skip_serializing_if = "is_false")]
        overwrite: bool,
    },
    /// Guarda variables para el próximo arranque.
    SaveVars { names: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WindowOp {
    Close,
    Minimize,
    Maximize,
    Restore,
    MinimizeAll,
    Screensaver,
    SendBack,
    EmptyRecycleBin,
    ToggleTopmost,
    EndProcess,
    Move {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
    MaximizeMonitor { monitor: u32 },
    Activate { title: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VolumeOp {
    Up { percent: u32 },
    Down { percent: u32 },
    Set { percent: u32 },
    ToggleMute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceRule {
    pub search: String,
    pub replace: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub regex: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_case: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub replace_all: bool,
    #[serde(default, skip_serializing_if = "is_next")]
    pub on_match: ReplaceMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplaceMatch {
    Stop,
    Next,
    Restart,
}

impl Default for ReplaceMatch {
    fn default() -> Self {
        ReplaceMatch::Next
    }
}

fn default_iterations() -> u32 {
    50
}

fn default_length_kb() -> u32 {
    64
}

fn is_default_iterations(value: &u32) -> bool {
    *value == default_iterations()
}

fn is_default_length(value: &u32) -> bool {
    *value == default_length_kb()
}

fn is_next(value: &ReplaceMatch) -> bool {
    *value == ReplaceMatch::Next
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn default_speed() -> u32 {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Repeat {
    Times { count: u32 },
    Forever,
}

impl Default for Repeat {
    fn default() -> Self {
        Repeat::Times { count: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    Key { key: String, down: bool },
    Button { button: MouseButton, down: bool },
    Move { x: i32, y: i32 },
    Wait { millis: u32 },
}

impl Action {
    pub fn runs_now(&self) -> bool {
        matches!(
            self,
            Action::InsertText { .. }
                | Action::Play { .. }
                | Action::OpenFile { .. }
                | Action::Launch { .. }
                | Action::OpenFolder { .. }
                | Action::OpenPage { .. }
                | Action::Window { .. }
                | Action::Volume { .. }
                | Action::Replace { .. }
                | Action::Power { .. }
                | Action::Media { .. }
                | Action::Delay { .. }
                | Action::Run { .. }
                | Action::Sequence { .. }
                | Action::Expression { .. }
                | Action::Reminder { .. }
                | Action::Screenshot { .. }
                | Action::SaveVars { .. }
        )
    }
}

pub fn check_open_file(path: &str, program: &str, arguments: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("falta la ruta del archivo".into());
    }
    check_open_field("la ruta", path, false)?;
    check_open_field("el programa", program.trim(), false)?;
    check_open_field("los argumentos", arguments.trim(), true)?;
    Ok(())
}

fn check_open_field(label: &str, value: &str, allow_quotes: bool) -> Result<(), String> {
    check_text_field(label, value, allow_quotes, 1024)
}

fn check_text_field(
    label: &str,
    value: &str,
    allow_quotes: bool,
    limit: usize,
) -> Result<(), String> {
    if value.chars().count() > limit {
        return Err(format!("{label} es demasiado largo"));
    }
    let rejected = value.chars().any(|character| {
        character == '\0' || character == '\n' || character == '\r' || (!allow_quotes && character == '"')
    });
    if rejected {
        return Err(format!(
            "{label} tiene un carácter que no se puede pasar al sistema"
        ));
    }
    Ok(())
}

pub fn check_launch(program: &str, arguments: &str, directory: &str) -> Result<(), String> {
    let program = program.trim();
    if program.is_empty() {
        return Err("falta el programa".into());
    }
    check_open_field("el programa", program, false)?;
    check_open_field("los argumentos", arguments.trim(), true)?;
    check_open_field("la carpeta", directory.trim(), false)?;
    Ok(())
}

pub fn check_folder(path: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("falta la carpeta".into());
    }
    check_open_field("la carpeta", path, false)
}

pub fn check_page(url: &str, browser: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("falta la dirección".into());
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://"))
    {
        return Err("la dirección tiene que empezar por http://, https:// o ftp://".into());
    }
    check_text_field("la dirección", url, false, 2048)?;
    check_open_field("el navegador", browser.trim(), false)
}

pub fn check_window(op: &WindowOp) -> Result<(), String> {
    match op {
        WindowOp::Move {
            left,
            top,
            right,
            bottom,
        } => window_box(*left, *top, *right, *bottom).map(|_| ()),
        WindowOp::MaximizeMonitor { monitor } => {
            if !(1..=16).contains(monitor) {
                Err("el monitor tiene que estar entre 1 y 16".into())
            } else {
                Ok(())
            }
        }
        WindowOp::Activate { title } => {
            let title = title.trim();
            if title.is_empty() {
                return Err("falta el título".into());
            }
            check_text_field("el título", title, true, 256)
        }
        WindowOp::Close
        | WindowOp::Minimize
        | WindowOp::Maximize
        | WindowOp::Restore
        | WindowOp::MinimizeAll
        | WindowOp::Screensaver
        | WindowOp::SendBack
        | WindowOp::EmptyRecycleBin
        | WindowOp::ToggleTopmost
        | WindowOp::EndProcess => Ok(()),
    }
}

pub fn window_box(left: i32, top: i32, right: i32, bottom: i32) -> Result<(i32, i32, i32, i32), String> {
    for value in [left, top, right, bottom] {
        if !(-32_000..=32_000).contains(&value) {
            return Err("la posición se sale del rango".into());
        }
    }
    if right <= left || bottom <= top {
        return Err("el rectángulo está vacío".into());
    }
    Ok((left, top, right - left, bottom - top))
}

pub fn check_volume(op: &VolumeOp) -> Result<(), String> {
    match op {
        VolumeOp::Up { percent } | VolumeOp::Down { percent } => {
            if !(1..=100).contains(percent) {
                Err("el cambio tiene que estar entre 1 y 100".into())
            } else {
                Ok(())
            }
        }
        VolumeOp::Set { percent } => {
            if !(0..=100).contains(percent) {
                Err("el volumen tiene que estar entre 0 y 100".into())
            } else {
                Ok(())
            }
        }
        VolumeOp::ToggleMute => Ok(()),
    }
}

pub fn adjusted_volume(current: u32, op: &VolumeOp) -> Option<u32> {
    let current = current.min(100);
    match op {
        VolumeOp::Up { percent } => Some(current.saturating_add(*percent).min(100)),
        VolumeOp::Down { percent } => Some(current.saturating_sub(*percent)),
        VolumeOp::Set { percent } => Some((*percent).min(100)),
        VolumeOp::ToggleMute => None,
    }
}

pub fn check_replace(rules: &[ReplaceRule], max_iterations: u32, max_length_kb: u32) -> Result<(), String> {
    if rules.is_empty() {
        return Err("falta una regla".into());
    }
    if rules.len() > 20 {
        return Err("como mucho 20 reglas".into());
    }
    if !(1..=500).contains(&max_iterations) {
        return Err("las vueltas tienen que estar entre 1 y 500".into());
    }
    if !(1..=256).contains(&max_length_kb) {
        return Err("el límite de longitud tiene que estar entre 1 y 256 KB".into());
    }
    for rule in rules {
        if rule.search.is_empty() {
            return Err("falta el texto a buscar".into());
        }
        check_replace_text("la búsqueda", &rule.search)?;
        check_replace_text("el reemplazo", &rule.replace)?;
        if rule.regex {
            regex::RegexBuilder::new(&rule.search)
                .case_insensitive(rule.ignore_case)
                .build()
                .map_err(|error| format!("la expresión no es válida: {error}"))?;
        }
    }
    Ok(())
}

fn check_replace_text(label: &str, value: &str) -> Result<(), String> {
    if value.chars().count() > 4_096 {
        return Err(format!("{label} supera las 4096 letras"));
    }
    if value.chars().any(|character| character == '\0') {
        return Err(format!("{label} tiene un carácter que no se puede guardar"));
    }
    Ok(())
}

pub fn apply_replace(
    text: &str,
    rules: &[ReplaceRule],
    max_iterations: u32,
    max_length_kb: u32,
) -> Result<String, String> {
    let max_bytes = usize::try_from(max_length_kb)
        .unwrap_or(256)
        .saturating_mul(1024);
    let mut current = text.to_string();
    let mut index = 0usize;
    let mut steps = 0u32;
    while index < rules.len() {
        match apply_rule(&current, &rules[index])? {
            None => index += 1,
            Some(next) => {
                steps += 1;
                if steps > max_iterations {
                    return Err("se pasó el límite de vueltas".into());
                }
                if next.len() > max_bytes {
                    return Err("el texto supera el límite de longitud".into());
                }
                let on_match = rules[index].on_match;
                current = next;
                match on_match {
                    ReplaceMatch::Stop => break,
                    ReplaceMatch::Next => index += 1,
                    ReplaceMatch::Restart => index = 0,
                }
            }
        }
    }
    Ok(current)
}

fn apply_rule(text: &str, rule: &ReplaceRule) -> Result<Option<String>, String> {
    if rule.regex || rule.ignore_case {
        let pattern = if rule.regex {
            rule.search.clone()
        } else {
            regex::escape(&rule.search)
        };
        let expression = regex::RegexBuilder::new(&pattern)
            .case_insensitive(rule.ignore_case)
            .build()
            .map_err(|error| format!("la expresión no es válida: {error}"))?;
        if !expression.is_match(text) {
            return Ok(None);
        }
        let limit = if rule.replace_all { 0 } else { 1 };
        let next = if rule.regex {
            expression.replacen(text, limit, |caps: &regex::Captures| {
                expand_replacement(caps, &rule.replace)
            })
        } else {
            expression.replacen(text, limit, |_caps: &regex::Captures| rule.replace.clone())
        };
        return Ok(Some(next.into_owned()));
    }
    if !text.contains(&rule.search) {
        return Ok(None);
    }
    let next = if rule.replace_all {
        text.replace(&rule.search, &rule.replace)
    } else {
        let index = text.find(&rule.search).expect("la búsqueda ya coincidió");
        let mut next = String::with_capacity(
            text.len().saturating_sub(rule.search.len()) + rule.replace.len(),
        );
        next.push_str(&text[..index]);
        next.push_str(&rule.replace);
        next.push_str(&text[index + rule.search.len()..]);
        next
    };
    Ok(Some(next))
}

fn expand_replacement(caps: &regex::Captures, template: &str) -> String {
    let mut output = String::new();
    let mut characters = template.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' {
            output.push(characters.next().unwrap_or('\\'));
            continue;
        }
        if character != '$' {
            output.push(character);
            continue;
        }
        if characters.peek() == Some(&'{') {
            characters.next();
            let number = take_digits(&mut characters);
            let case = match characters.peek() {
                Some('U') => {
                    characters.next();
                    Some(true)
                }
                Some('L') => {
                    characters.next();
                    Some(false)
                }
                _ => None,
            };
            if characters.peek() == Some(&'}') && !number.is_empty() {
                characters.next();
                push_group(&mut output, caps, &number, case);
            } else {
                output.push('$');
                output.push('{');
                output.push_str(&number);
                if let Some(upper) = case {
                    output.push(if upper { 'U' } else { 'L' });
                }
            }
            continue;
        }
        let number = take_digits(&mut characters);
        if number.is_empty() {
            output.push('$');
            continue;
        }
        push_group(&mut output, caps, &number, None);
    }
    output
}

fn take_digits(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut number = String::new();
    while matches!(characters.peek(), Some(character) if character.is_ascii_digit()) {
        number.push(characters.next().expect("dígito"));
    }
    number
}

fn push_group(output: &mut String, caps: &regex::Captures, number: &str, case: Option<bool>) {
    let Ok(index) = number.parse::<usize>() else {
        return;
    };
    let Some(matched) = caps.get(index) else {
        return;
    };
    match case {
        Some(true) => output.push_str(&matched.as_str().to_uppercase()),
        Some(false) => output.push_str(&matched.as_str().to_lowercase()),
        None => output.push_str(matched.as_str()),
    }
}

pub fn wait_millis(millis: u32, speed: u32) -> u64 {
    let speed = u64::from(speed.clamp(10, 400));
    u64::from(millis) * 100 / speed
}

/// Atajo ya comprobado. El nombre de la tecla no depende de Windows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub key: Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Letter(char),
    Digit(char),
    Function(u8),
    Space,
    Enter,
    Tab,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Numpad(u8),
    NumpadAdd,
    NumpadSub,
    NumpadMul,
    NumpadDiv,
    NumpadDec,
    Ctrl,
    Shift,
    Alt,
    Win,
}

impl Chord {
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("ctrl");
        }
        if self.shift {
            parts.push("shift");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.win {
            parts.push("win");
        }
        parts.push(self.key.label());
        parts.join("+")
    }
}

impl Key {
    pub fn label(&self) -> &'static str {
        match self {
            Key::Letter(c) => letter_label(*c),
            Key::Digit(c) => digit_label(*c),
            Key::Function(1) => "f1",
            Key::Function(2) => "f2",
            Key::Function(3) => "f3",
            Key::Function(4) => "f4",
            Key::Function(5) => "f5",
            Key::Function(6) => "f6",
            Key::Function(7) => "f7",
            Key::Function(8) => "f8",
            Key::Function(9) => "f9",
            Key::Function(10) => "f10",
            Key::Function(11) => "f11",
            Key::Function(12) => "f12",
            Key::Function(13) => "f13",
            Key::Function(14) => "f14",
            Key::Function(15) => "f15",
            Key::Function(16) => "f16",
            Key::Function(17) => "f17",
            Key::Function(18) => "f18",
            Key::Function(19) => "f19",
            Key::Function(20) => "f20",
            Key::Function(21) => "f21",
            Key::Function(22) => "f22",
            Key::Function(23) => "f23",
            Key::Function(24) => "f24",
            Key::Function(_) => "f?",
            Key::Space => "space",
            Key::Enter => "enter",
            Key::Tab => "tab",
            Key::Escape => "escape",
            Key::Left => "left",
            Key::Right => "right",
            Key::Up => "up",
            Key::Down => "down",
            Key::Numpad(0) => "num0",
            Key::Numpad(1) => "num1",
            Key::Numpad(2) => "num2",
            Key::Numpad(3) => "num3",
            Key::Numpad(4) => "num4",
            Key::Numpad(5) => "num5",
            Key::Numpad(6) => "num6",
            Key::Numpad(7) => "num7",
            Key::Numpad(8) => "num8",
            Key::Numpad(9) => "num9",
            Key::Numpad(_) => "num?",
            Key::NumpadAdd => "num_add",
            Key::NumpadSub => "num_sub",
            Key::NumpadMul => "num_mul",
            Key::NumpadDiv => "num_div",
            Key::NumpadDec => "num_dec",
            Key::Ctrl => "ctrl",
            Key::Shift => "shift",
            Key::Alt => "alt",
            Key::Win => "win",
        }
    }
}

fn letter_label(c: char) -> &'static str {
    match c {
        'a' => "a",
        'b' => "b",
        'c' => "c",
        'd' => "d",
        'e' => "e",
        'f' => "f",
        'g' => "g",
        'h' => "h",
        'i' => "i",
        'j' => "j",
        'k' => "k",
        'l' => "l",
        'm' => "m",
        'n' => "n",
        'o' => "o",
        'p' => "p",
        'q' => "q",
        'r' => "r",
        's' => "s",
        't' => "t",
        'u' => "u",
        'v' => "v",
        'w' => "w",
        'x' => "x",
        'y' => "y",
        'z' => "z",
        _ => "?",
    }
}

fn digit_label(c: char) -> &'static str {
    match c {
        '0' => "0",
        '1' => "1",
        '2' => "2",
        '3' => "3",
        '4' => "4",
        '5' => "5",
        '6' => "6",
        '7' => "7",
        '8' => "8",
        '9' => "9",
        _ => "?",
    }
}

pub fn modifiers_from_mask(mask: u32) -> Vec<Modifier> {
    let mut modifiers = Vec::new();
    if mask & 0x02 != 0 {
        modifiers.push(Modifier::Ctrl);
    }
    if mask & 0x04 != 0 {
        modifiers.push(Modifier::Shift);
    }
    if mask & 0x01 != 0 {
        modifiers.push(Modifier::Alt);
    }
    if mask & 0x08 != 0 {
        modifiers.push(Modifier::Win);
    }
    modifiers
}

pub fn key_from_virtual_code(code: u32) -> Option<Key> {
    Some(match code {
        0x30..=0x39 => Key::Digit(char::from(code as u8)),
        0x41..=0x5A => Key::Letter(char::from(code as u8).to_ascii_lowercase()),
        0x70..=0x87 => Key::Function((code - 0x6F) as u8),
        0x60..=0x69 => Key::Numpad((code - 0x60) as u8),
        0x6B => Key::NumpadAdd,
        0x6D => Key::NumpadSub,
        0x6A => Key::NumpadMul,
        0x6F => Key::NumpadDiv,
        0x6E => Key::NumpadDec,
        0x20 => Key::Space,
        0x0D => Key::Enter,
        0x09 => Key::Tab,
        0x1B => Key::Escape,
        0x25 => Key::Left,
        0x27 => Key::Right,
        0x26 => Key::Up,
        0x28 => Key::Down,
        0x10 | 0xA0 | 0xA1 => Key::Shift,
        0x11 | 0xA2 | 0xA3 => Key::Ctrl,
        0x12 | 0xA4 | 0xA5 => Key::Alt,
        0x5B | 0x5C => Key::Win,
        _ => return None,
    })
}

pub fn parse_chord(hotkey: &Hotkey) -> Result<Chord, String> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    for modifier in &hotkey.modifiers {
        match modifier {
            Modifier::Ctrl => ctrl = true,
            Modifier::Shift => shift = true,
            Modifier::Alt => alt = true,
            Modifier::Win => win = true,
        }
    }
    Ok(Chord {
        ctrl,
        shift,
        alt,
        win,
        key: parse_key(&hotkey.key)?,
    })
}

pub fn parse_key(raw: &str) -> Result<Key, String> {
    let name = raw.trim().to_ascii_lowercase();
    if name.len() == 1 {
        let c = name.chars().next().unwrap();
        if c.is_ascii_lowercase() {
            return Ok(Key::Letter(c));
        }
        if c.is_ascii_digit() {
            return Ok(Key::Digit(c));
        }
    }
    if let Some(rest) = name.strip_prefix('f') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(Key::Function(n));
            }
        }
    }
    if let Some(rest) = name.strip_prefix("num") {
        if rest.len() == 1 {
            let c = rest.chars().next().unwrap();
            if c.is_ascii_digit() {
                return Ok(Key::Numpad(c.to_digit(10).unwrap() as u8));
            }
        }
    }
    let key = match name.as_str() {
        "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "num_add" | "numpad_add" => Key::NumpadAdd,
        "num_sub" | "numpad_sub" | "num_minus" => Key::NumpadSub,
        "num_mul" | "numpad_mul" => Key::NumpadMul,
        "num_div" | "numpad_div" => Key::NumpadDiv,
        "num_dec" | "numpad_dec" => Key::NumpadDec,
        "ctrl" | "control" => Key::Ctrl,
        "shift" => Key::Shift,
        "alt" => Key::Alt,
        "win" | "windows" => Key::Win,
        _ => return Err(format!("tecla desconocida: {raw}")),
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_letter_and_ignores_modifier_order() {
        let hotkey = Hotkey {
            modifiers: vec![Modifier::Shift, Modifier::Ctrl],
            key: "H".into(),
        };
        let chord = parse_chord(&hotkey).unwrap();
        assert!(chord.ctrl && chord.shift && !chord.alt);
        assert_eq!(chord.key, Key::Letter('h'));
        assert_eq!(chord.label(), "ctrl+shift+h");
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(parse_key("not-a-key").is_err());
    }

    #[test]
    fn double_speed_halves_a_wait() {
        assert_eq!(wait_millis(200, 200), 100);
        assert_eq!(wait_millis(200, 100), 200);
    }

    #[test]
    fn play_roundtrip_keeps_the_sequence() {
        let action = Action::Play {
            events: vec![
                InputEvent::Key {
                    key: "a".into(),
                    down: true,
                },
                InputEvent::Wait { millis: 40 },
                InputEvent::Button {
                    button: MouseButton::Left,
                    down: true,
                },
                InputEvent::Move { x: 10, y: 20 },
            ],
            speed: 100,
            repeat: Repeat::Times { count: 2 },
        };
        let text = serde_json::to_string(&action).unwrap();
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
    }

    #[test]
    fn open_file_roundtrip_omits_an_empty_program() {
        let action = Action::OpenFile {
            path: r"C:\notas\hoy.txt".into(),
            program: String::new(),
            arguments: String::new(),
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("open_file"));
        assert!(!text.contains("program"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
    }

    #[test]
    fn open_file_rejects_a_quote_in_the_path() {
        assert!(check_open_file(r#"C:\notas\"hoy".txt"#, "", "").is_err());
        assert!(check_open_file(r"C:\notas\hoy.txt", r"C:\Windows\notepad.exe", "/a").is_ok());
    }

    #[test]
    fn launch_roundtrip_omits_the_optional_flags() {
        let action = Action::Launch {
            program: r"C:\Windows\notepad.exe".into(),
            arguments: String::new(),
            directory: String::new(),
            minimized: false,
            activate_existing: false,
            as_admin: false,
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("launch"));
        assert!(!text.contains("minimized"));
        assert!(!text.contains("as_admin"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
        assert!(check_launch("", "", "").is_err());
        assert!(check_launch(r"C:\Windows\notepad.exe", "/a", r"C:\notas").is_ok());
    }

    #[test]
    fn folder_roundtrip_keeps_the_path() {
        let action = Action::OpenFolder {
            path: r"C:\notas".into(),
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("open_folder"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
        assert!(check_folder("").is_err());
        assert!(check_folder(r"C:\notas").is_ok());
    }

    #[test]
    fn page_roundtrip_omits_an_empty_browser() {
        let action = Action::OpenPage {
            url: "https://example.com".into(),
            browser: String::new(),
            new_window: false,
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("open_page"));
        assert!(!text.contains("browser"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
        assert!(check_page("nota.txt", "").is_err());
        assert!(check_page("https://example.com/a b", "").is_ok());
    }

    #[test]
    fn window_roundtrip_keeps_the_operation() {
        let action = Action::Window {
            op: WindowOp::Move {
                left: 10,
                top: 20,
                right: 400,
                bottom: 300,
            },
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("window"));
        assert!(text.contains("move"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
        assert_eq!(window_box(10, 20, 400, 300).unwrap(), (10, 20, 390, 280));
        assert!(check_window(&WindowOp::Activate { title: "  ".into() }).is_err());
        assert!(check_window(&WindowOp::Close).is_ok());
    }

    #[test]
    fn volume_change_stays_inside_the_scale() {
        let action = Action::Volume {
            op: VolumeOp::Up { percent: 10 },
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("volume"));
        assert!(text.contains("up"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
        assert_eq!(adjusted_volume(95, &VolumeOp::Up { percent: 10 }), Some(100));
        assert_eq!(adjusted_volume(5, &VolumeOp::Down { percent: 10 }), Some(0));
        assert_eq!(adjusted_volume(40, &VolumeOp::Set { percent: 12 }), Some(12));
        assert!(adjusted_volume(40, &VolumeOp::ToggleMute).is_none());
        assert!(check_volume(&VolumeOp::Up { percent: 0 }).is_err());
    }

    #[test]
    fn replace_rules_keep_the_text_inside_the_limits() {
        let rule = ReplaceRule {
            search: "a".into(),
            replace: "b".into(),
            regex: false,
            ignore_case: false,
            replace_all: false,
            on_match: ReplaceMatch::Next,
        };
        let action = Action::Replace {
            rules: vec![rule.clone()],
            max_iterations: 50,
            max_length_kb: 64,
        };
        let text = serde_json::to_string(&action).unwrap();
        assert!(text.contains("replace"));
        assert!(!text.contains("regex"));
        let back: Action = serde_json::from_str(&text).unwrap();
        assert_eq!(back, action);
        assert_eq!(apply_replace("a-a", &[rule.clone()], 50, 64).unwrap(), "b-a");
        let mut all = rule.clone();
        all.replace_all = true;
        assert_eq!(apply_replace("a-a", &[all], 50, 64).unwrap(), "b-b");
        let ignore = ReplaceRule {
            search: "A".into(),
            replace: "x".into(),
            regex: false,
            ignore_case: true,
            replace_all: false,
            on_match: ReplaceMatch::Next,
        };
        assert_eq!(apply_replace("ab", &[ignore], 50, 64).unwrap(), "xb");
        let expression = ReplaceRule {
            search: "(\\d+)".into(),
            replace: "[$1]".into(),
            regex: true,
            ignore_case: false,
            replace_all: true,
            on_match: ReplaceMatch::Next,
        };
        assert_eq!(apply_replace("ab12", &[expression], 50, 64).unwrap(), "ab[12]");
        let upper = ReplaceRule {
            search: "(ab)".into(),
            replace: "${1U}".into(),
            regex: true,
            ignore_case: false,
            replace_all: false,
            on_match: ReplaceMatch::Next,
        };
        assert_eq!(apply_replace("ab", &[upper], 50, 64).unwrap(), "AB");
        let mut restart = rule.clone();
        restart.on_match = ReplaceMatch::Restart;
        restart.replace = "a".into();
        assert!(apply_replace("a", &[restart], 3, 64).is_err());
        assert!(check_replace(&[rule.clone()], 50, 64).is_ok());
        let mut empty = rule;
        empty.search.clear();
        assert!(check_replace(&[empty], 50, 64).is_err());
    }
}
