use std::fs;
use std::path::Path;

use crate::model::{
    check_condition, check_delay, check_expression_action, check_folder, check_launch, check_names,
    check_open_file, check_page, check_reminder, check_replace, check_sequence, check_shot,
    check_target_id, check_trigger, check_variable_name, check_volume, check_window, parse_chord,
    parse_key, Action, Document, InputEvent, Repeat, SCHEMA_VERSION,
};

pub fn load(path: &Path) -> Result<Document, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    parse(&text)
}

pub fn save(path: &Path, document: &Document) -> Result<(), String> {
    let text = serde_json::to_string_pretty(document)
        .map_err(|error| format!("no se pudo serializar el archivo: {error}"))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("no se pudo crear {}: {error}", parent.display()))?;
        }
    }
    fs::write(path, text).map_err(|error| format!("no se pudo escribir {}: {error}", path.display()))
}

pub fn parse(text: &str) -> Result<Document, String> {
    let document: Document = serde_json::from_str(text)
        .map_err(|error| format!("el archivo de macros no es JSON válido: {error}"))?;
    check(&document)?;
    Ok(document)
}

pub fn check(document: &Document) -> Result<(), String> {
    if document.version != SCHEMA_VERSION {
        return Err(format!(
            "versión de esquema {} no soportada; este programa lee la versión {SCHEMA_VERSION}",
            document.version
        ));
    }
    let mut seen_ids = Vec::new();
    let mut seen_chords = Vec::new();
    for macro_ in &document.macros {
        if macro_.id.trim().is_empty() {
            return Err("hay una macro sin id".into());
        }
        if seen_ids.contains(&macro_.id) {
            return Err(format!("id repetido: {}", macro_.id));
        }
        seen_ids.push(macro_.id.clone());
        check_action(macro_)?;
        let Some(hotkey) = &macro_.hotkey else {
            continue;
        };
        if hotkey.key.trim().is_empty() {
            if hotkey.modifiers.is_empty() {
                continue;
            }
            return Err(format!("«{}» tiene modificadores y ninguna tecla", macro_.name));
        }
        let chord = parse_chord(hotkey).map_err(|error| {
            format!("atajo de «{}» ({}) inválido: {error}", macro_.name, macro_.id)
        })?;
        if seen_chords.contains(&chord) {
            return Err(format!("el atajo {} está repetido", chord.label()));
        }
        seen_chords.push(chord);
    }
    if document.variables.len() > 64 {
        return Err("hay demasiadas variables guardadas".into());
    }
    for (name, value) in &document.variables {
        check_variable_name(name)?;
        if value.chars().count() > 4_096 {
            return Err(format!("el valor de {name} es demasiado largo"));
        }
    }
    Ok(())
}

fn check_filters(macro_: &crate::model::Macro) -> Result<(), String> {
    if let Some(trigger) = &macro_.trigger {
        check_trigger(trigger).map_err(|error| format!("«{}»: {error}", macro_.name))?;
        if let crate::model::Trigger::Multitap { key, .. } = trigger {
            parse_key(key).map_err(|error| format!("«{}»: {error}", macro_.name))?;
        }
    }
    if let Some(condition) = &macro_.condition {
        check_condition(condition).map_err(|error| format!("«{}»: {error}", macro_.name))?;
    }
    Ok(())
}

fn check_action(macro_: &crate::model::Macro) -> Result<(), String> {
    check_filters(macro_)?;
    let Action::Play {
        events,
        speed,
        repeat,
    } = &macro_.action
    else {
        if let Action::OpenFile {
            path,
            program,
            arguments,
        } = &macro_.action
        {
            return check_open_file(path, program, arguments)
                .map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        if let Action::Launch {
            program,
            arguments,
            directory,
            ..
        } = &macro_.action
        {
            return check_launch(program, arguments, directory)
                .map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        if let Action::OpenFolder { path } = &macro_.action {
            return check_folder(path).map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        if let Action::OpenPage { url, browser, .. } = &macro_.action {
            return check_page(url, browser).map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        if let Action::Window { op } = &macro_.action {
            return check_window(op).map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        if let Action::Volume { op } = &macro_.action {
            return check_volume(op).map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        if let Action::Replace {
            rules,
            max_iterations,
            max_length_kb,
        } = &macro_.action
        {
            return check_replace(rules, *max_iterations, *max_length_kb)
                .map_err(|error| format!("«{}»: {error}", macro_.name));
        }
        let checked = match &macro_.action {
            Action::Power { .. } | Action::Media { .. } => Ok(()),
            Action::Delay { delay } => check_delay(delay),
            Action::Run { id } => check_target_id(id),
            Action::Sequence { ids } => check_sequence(ids),
            Action::Expression { source, variable } => check_expression_action(source, variable),
            Action::Reminder { title, text } => check_reminder(title, text),
            Action::Screenshot { source, dest, .. } => check_shot(source, dest),
            Action::SaveVars { names } => check_names(names),
            _ => Ok(()),
        };
        return checked.map_err(|error| format!("«{}»: {error}", macro_.name));
    };
    if !(10..=400).contains(speed) {
        return Err(format!(
            "la velocidad de «{}» tiene que estar entre 10 y 400",
            macro_.name
        ));
    }
    if let Repeat::Times { count } = repeat {
        if !(1..=100).contains(count) {
            return Err(format!(
                "las repeticiones de «{}» tienen que estar entre 1 y 100",
                macro_.name
            ));
        }
    }
    if events.len() > 5_000 {
        return Err(format!("«{}» tiene demasiados eventos", macro_.name));
    }
    for event in events {
        match event {
            InputEvent::Key { key, .. } => {
                parse_key(key).map_err(|error| format!("«{}»: {error}", macro_.name))?;
            }
            InputEvent::Wait { millis } if *millis > 60_000 => {
                return Err(format!(
                    "una espera de «{}» pasa de 60 segundos",
                    macro_.name
                ));
            }
            InputEvent::Wait { .. } | InputEvent::Button { .. } | InputEvent::Move { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_example_shape() {
        let text = r#"{
          "version": 1,
          "macros": [{
            "id": "hola",
            "name": "Saludo",
            "hotkey": { "modifiers": ["ctrl", "shift"], "key": "h" },
            "action": { "type": "insert_text", "text": "hola" }
          }]
        }"#;
        let document = parse(text).unwrap();
        assert_eq!(document.macros.len(), 1);
        assert_eq!(document.macros[0].id, "hola");
    }

    #[test]
    fn rejects_a_repeated_hotkey() {
        let text = r#"{
          "version": 1,
          "macros": [
            {
              "id": "a",
              "name": "A",
              "hotkey": { "modifiers": ["ctrl"], "key": "h" },
              "action": { "type": "insert_text", "text": "a" }
            },
            {
              "id": "b",
              "name": "B",
              "hotkey": { "modifiers": ["ctrl"], "key": "H" },
              "action": { "type": "insert_text", "text": "b" }
            }
          ]
        }"#;
        assert!(parse(text).is_err());
    }
}
