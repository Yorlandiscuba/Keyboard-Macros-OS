use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CHAIN_LIMIT: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerOp {
    Lock,
    Logoff,
    Shutdown,
    Restart,
    Sleep,
    Hibernate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaOp {
    PlayPause,
    Stop,
    Next,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Delay {
    Pause { millis: u32 },
    UntilWindow { title: String, millis: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShotSource {
    Screen,
    Window,
    Region {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShotDest {
    Clipboard,
    File { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    Text { phrase: String },
    Gesture { directions: String },
    Multitap { key: String, count: u32 },
    Schedule { hour: u8, minute: u8 },
    Startup,
    Window { title: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    Title { text: String, #[serde(default, skip_serializing_if = "is_false")] invert: bool },
    Class { text: String, #[serde(default, skip_serializing_if = "is_false")] invert: bool },
    Process { text: String, #[serde(default, skip_serializing_if = "is_false")] invert: bool },
    Expression { source: String, #[serde(default, skip_serializing_if = "is_false")] invert: bool },
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn media_virtual_key(op: MediaOp) -> u16 {
    match op {
        MediaOp::Next => 0xb0,
        MediaOp::Previous => 0xb1,
        MediaOp::Stop => 0xb2,
        MediaOp::PlayPause => 0xb3,
    }
}

pub fn power_flags(op: PowerOp, force: bool) -> Option<u32> {
    let base = match op {
        PowerOp::Lock | PowerOp::Sleep | PowerOp::Hibernate => return None,
        PowerOp::Logoff => 0,
        PowerOp::Shutdown => 1 | 8,
        PowerOp::Restart => 2,
    };
    Some(if force { base | 4 } else { base })
}

pub fn check_delay(delay: &Delay) -> Result<(), String> {
    match delay {
        Delay::Pause { millis } => {
            if *millis > 600_000 {
                Err("la espera no puede pasar de 10 minutos".into())
            } else {
                Ok(())
            }
        }
        Delay::UntilWindow { title, millis } => {
            if title.trim().is_empty() {
                return Err("falta el título que hay que esperar".into());
            }
            check_plain("el título", title.trim(), 256)?;
            if !(100..=600_000).contains(millis) {
                Err("la espera de la ventana tiene que estar entre 0,1 s y 10 min".into())
            } else {
                Ok(())
            }
        }
    }
}

pub fn check_target_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("falta la macro que hay que ejecutar".into());
    }
    if id.chars().count() > 128 || id.chars().any(|character| character == '\0' || character == '\n' || character == '\r') {
        return Err("el id de la macro no es válido".into());
    }
    Ok(())
}

pub fn check_sequence(ids: &[String]) -> Result<(), String> {
    if ids.is_empty() {
        return Err("la cadena no tiene macros".into());
    }
    if ids.len() > 20 {
        return Err("la cadena admite como mucho 20 macros".into());
    }
    for id in ids {
        check_target_id(id)?;
    }
    Ok(())
}

pub fn check_variable_name(name: &str) -> Result<(), String> {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return Err("falta el nombre de la variable".into());
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err("el nombre de la variable tiene que empezar por una letra".into());
    }
    if name.len() > 64 || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_') {
        return Err("el nombre de la variable no es válido".into());
    }
    Ok(())
}

pub fn check_names(names: &[String]) -> Result<(), String> {
    if names.is_empty() {
        return Err("falta el nombre de la variable".into());
    }
    if names.len() > 32 {
        return Err("como mucho 32 variables".into());
    }
    for name in names {
        check_variable_name(name.trim())?;
    }
    Ok(())
}

pub fn check_expression_action(source: &str, variable: &str) -> Result<(), String> {
    if source.trim().is_empty() {
        return Err("falta la expresión".into());
    }
    if source.chars().count() > 512 {
        return Err("la expresión es demasiado larga".into());
    }
    check_expression(source)?;
    if !variable.trim().is_empty() {
        check_variable_name(variable.trim())?;
    }
    Ok(())
}

pub fn check_reminder(title: &str, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("falta el aviso".into());
    }
    check_note("el aviso", text, 2_000)?;
    if !title.trim().is_empty() {
        check_note("el título", title, 120)?;
    }
    Ok(())
}

pub fn check_shot(source: &ShotSource, dest: &ShotDest) -> Result<(), String> {
    if let ShotSource::Region { left, top, right, bottom } = source {
        for value in [*left, *top, *right, *bottom] {
            if !(-32_000..=32_000).contains(&value) {
                return Err("la captura se sale del rango".into());
            }
        }
        if *right <= *left || *bottom <= *top {
            return Err("la captura está vacía".into());
        }
    }
    if let ShotDest::File { path } = dest {
        let path = path.trim();
        if path.is_empty() {
            return Err("falta la ruta de la captura".into());
        }
        check_plain("la ruta", path, 1024)?;
        if path.contains('"') {
            return Err("la ruta tiene un carácter que no se puede pasar al sistema".into());
        }
    }
    Ok(())
}

pub fn check_trigger(trigger: &Trigger) -> Result<(), String> {
    match trigger {
        Trigger::Text { phrase } => {
            let phrase = phrase.trim();
            if phrase.is_empty() || phrase.chars().count() > 32 {
                return Err("el atajo de texto tiene que tener entre 1 y 32 caracteres".into());
            }
            if phrase.chars().any(|character| !character.is_ascii_alphanumeric() && character != ' ') {
                return Err("el atajo de texto solo admite letras, números y espacios".into());
            }
            Ok(())
        }
        Trigger::Gesture { directions } => {
            let directions = directions.trim();
            if directions.is_empty() || directions.len() > 16 {
                return Err("el gesto tiene que tener entre 1 y 16 direcciones".into());
            }
            if directions.chars().any(|character| !matches!(character, 'U' | 'D' | 'L' | 'R')) {
                return Err("el gesto solo usa U, D, L y R".into());
            }
            Ok(())
        }
        Trigger::Multitap { key, count } => {
            if key.trim().is_empty() {
                return Err("faltan las pulsaciones".into());
            }
            if !(2..=8).contains(count) {
                return Err("las pulsaciones tienen que estar entre 2 y 8".into());
            }
            Ok(())
        }
        Trigger::Schedule { hour, minute } => {
            if *hour > 23 || *minute > 59 {
                Err("la hora no es válida".into())
            } else {
                Ok(())
            }
        }
        Trigger::Startup => Ok(()),
        Trigger::Window { title } => {
            if title.trim().is_empty() {
                return Err("falta el título que dispara la macro".into());
            }
            check_plain("el título", title.trim(), 256)
        }
    }
}

pub fn check_condition(condition: &Condition) -> Result<(), String> {
    match condition {
        Condition::Title { text, .. } | Condition::Class { text, .. } | Condition::Process { text, .. } => {
            if text.trim().is_empty() {
                return Err("falta la condición".into());
            }
            check_plain("la condición", text.trim(), 256)
        }
        Condition::Expression { source, .. } => check_expression_action(source, ""),
    }
}

pub fn any_part_matches(haystack: &str, pattern: &str) -> bool {
    let haystack = haystack.to_lowercase();
    pattern
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| haystack.contains(&part.to_lowercase()))
}

fn check_plain(label: &str, value: &str, limit: usize) -> Result<(), String> {
    if value.chars().count() > limit {
        return Err(format!("{label} es demasiado largo"));
    }
    if value.chars().any(|character| character == '\0' || character == '\n' || character == '\r') {
        return Err(format!("{label} tiene un carácter que no se puede guardar"));
    }
    Ok(())
}

fn check_note(label: &str, value: &str, limit: usize) -> Result<(), String> {
    if value.chars().count() > limit {
        return Err(format!("{label} es demasiado largo"));
    }
    if value.chars().any(|character| character == '\0') {
        return Err(format!("{label} tiene un carácter que no se puede guardar"));
    }
    Ok(())
}

pub fn check_expression(source: &str) -> Result<(), String> {
    let mut parser = Parser::new(source);
    parser.parse_or()?;
    parser.skip();
    if parser.index != parser.chars.len() {
        return Err("la expresión tiene un trozo de más".into());
    }
    Ok(())
}

pub fn eval_expression(source: &str, variables: &BTreeMap<String, String>) -> Result<f64, String> {
    let mut parser = Parser::new(source);
    let expression = parser.parse_or()?;
    parser.skip();
    if parser.index != parser.chars.len() {
        return Err("la expresión tiene un trozo de más".into());
    }
    eval(&expression, variables)
}

pub fn format_value(value: f64) -> Result<String, String> {
    if !value.is_finite() {
        return Err("el resultado no es un número".into());
    }
    let rounded = value.round();
    if (value - rounded).abs() < 1e-9 && rounded.abs() < 1e15 {
        Ok(format!("{}", rounded as i64))
    } else {
        let text = format!("{value:.6}");
        Ok(text.trim_end_matches('0').trim_end_matches('.').to_string())
    }
}

enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Call(String, Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
}

enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

struct Parser {
    chars: Vec<char>,
    index: usize,
}

impl Parser {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
        }
    }

    fn skip(&mut self) {
        while self.peek().is_some_and(|character| character.is_whitespace()) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn eat(&mut self, expected: &str) -> bool {
        self.skip();
        let rest: String = self.chars[self.index..].iter().collect();
        if rest.starts_with(expected) {
            self.index += expected.chars().count();
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.eat("||") {
            let right = self.parse_and()?;
            left = Expr::Bin(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_cmp()?;
        while self.eat("&&") {
            let right = self.parse_cmp()?;
            left = Expr::Bin(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let left = self.parse_sum()?;
        let op = if self.eat("==") {
            BinOp::Eq
        } else if self.eat("!=") {
            BinOp::Ne
        } else if self.eat(">=") {
            BinOp::Ge
        } else if self.eat("<=") {
            BinOp::Le
        } else if self.eat(">") {
            BinOp::Gt
        } else if self.eat("<") {
            BinOp::Lt
        } else {
            return Ok(left);
        };
        let right = self.parse_sum()?;
        Ok(Expr::Bin(op, Box::new(left), Box::new(right)))
    }

    fn parse_sum(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_product()?;
        loop {
            let op = if self.eat("+") {
                BinOp::Add
            } else if self.eat("-") {
                BinOp::Sub
            } else {
                break;
            };
            let right = self.parse_product()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_product(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.eat("*") {
                BinOp::Mul
            } else if self.eat("/") {
                BinOp::Div
            } else {
                break;
            };
            let right = self.parse_unary()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.eat("!") {
            return Ok(Expr::Not(Box::new(self.parse_unary()?)));
        }
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        self.skip();
        if self.eat("(") {
            let inner = self.parse_or()?;
            if !self.eat(")") {
                return Err("falta el cierre de la expresión".into());
            }
            return Ok(inner);
        }
        if self.eat("$") {
            return Ok(Expr::Var(self.word()?));
        }
        if self.peek().is_some_and(|character| character.is_ascii_digit()) {
            return self.number();
        }
        if self.peek().is_some_and(|character| character.is_ascii_alphabetic()) {
            let name = self.word()?;
            if !self.eat("(") {
                return Err("la función tiene que llevar paréntesis".into());
            }
            let argument = self.parse_or()?;
            if !self.eat(")") {
                return Err("falta el cierre de la función".into());
            }
            return Ok(Expr::Call(name, Box::new(argument)));
        }
        Err("la expresión no se entiende".into())
    }

    fn word(&mut self) -> Result<String, String> {
        self.skip();
        let mut name = String::new();
        if !self.peek().is_some_and(|character| character.is_ascii_alphabetic() || character == '_') {
            return Err("falta un nombre".into());
        }
        while self.peek().is_some_and(|character| character.is_ascii_alphanumeric() || character == '_') {
            name.push(self.chars[self.index]);
            self.index += 1;
        }
        Ok(name)
    }

    fn number(&mut self) -> Result<Expr, String> {
        self.skip();
        let start = self.index;
        while self.peek().is_some_and(|character| character.is_ascii_digit()) {
            self.index += 1;
        }
        if self.peek() == Some('.') {
            self.index += 1;
            while self.peek().is_some_and(|character| character.is_ascii_digit()) {
                self.index += 1;
            }
        }
        let text: String = self.chars[start..self.index].iter().collect();
        text.parse::<f64>()
            .map(Expr::Num)
            .map_err(|_| "el número no es válido".into())
    }
}

fn eval(expression: &Expr, variables: &BTreeMap<String, String>) -> Result<f64, String> {
    let value = match expression {
        Expr::Num(value) => *value,
        Expr::Var(name) => {
            let raw = variables
                .get(name)
                .ok_or_else(|| format!("la variable {name} no tiene valor"))?;
            raw.trim()
                .parse::<f64>()
                .map_err(|_| format!("la variable {name} no es un número"))?
        }
        Expr::Neg(inner) => -eval(inner, variables)?,
        Expr::Not(inner) => {
            if eval(inner, variables)? == 0.0 {
                1.0
            } else {
                0.0
            }
        }
        Expr::Call(name, argument) => {
            let value = eval(argument, variables)?;
            match name.as_str() {
                "sqrt" => {
                    if value < 0.0 {
                        return Err("la raíz no admite un número negativo".into());
                    }
                    value.sqrt()
                }
                "abs" => value.abs(),
                _ => return Err(format!("la función {name} no existe")),
            }
        }
        Expr::Bin(op, left, right) => {
            if matches!(op, BinOp::And) {
                return if eval(left, variables)? == 0.0 {
                    Ok(0.0)
                } else if eval(right, variables)? == 0.0 {
                    Ok(0.0)
                } else {
                    Ok(1.0)
                };
            }
            if matches!(op, BinOp::Or) {
                return if eval(left, variables)? != 0.0 {
                    Ok(1.0)
                } else if eval(right, variables)? != 0.0 {
                    Ok(1.0)
                } else {
                    Ok(0.0)
                };
            }
            let left = eval(left, variables)?;
            let right = eval(right, variables)?;
            match op {
                BinOp::Add => left + right,
                BinOp::Sub => left - right,
                BinOp::Mul => left * right,
                BinOp::Div => {
                    if right == 0.0 {
                        return Err("no se puede dividir entre cero".into());
                    }
                    left / right
                }
                BinOp::Eq => bool_value((left - right).abs() < 1e-9),
                BinOp::Ne => bool_value((left - right).abs() >= 1e-9),
                BinOp::Lt => bool_value(left < right),
                BinOp::Gt => bool_value(left > right),
                BinOp::Le => bool_value(left <= right),
                BinOp::Ge => bool_value(left >= right),
                BinOp::And | BinOp::Or => unreachable!("ya se resolvieron"),
            }
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err("el resultado no es un número".into())
    }
}

fn bool_value(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_the_sample_comparison() {
        assert_eq!(eval_expression("1+2*3", &BTreeMap::new()).unwrap(), 7.0);
        let mut variables = BTreeMap::new();
        variables.insert("a".into(), "4".into());
        variables.insert("b".into(), "9".into());
        assert_eq!(eval_expression("$a > sqrt($b)", &variables).unwrap(), 1.0);
        assert_eq!(format_value(eval_expression("sqrt(9)", &BTreeMap::new()).unwrap()).unwrap(), "3");
        assert!(check_expression("(").is_err());
        assert_eq!(power_flags(PowerOp::Shutdown, true), Some(13));
        assert_eq!(media_virtual_key(MediaOp::PlayPause), 0xb3);
        assert!(any_part_matches("Bloc de notas", "Calc;Bloc"));
        assert!(check_delay(&Delay::Pause { millis: 600_001 }).is_err());
    }
}
