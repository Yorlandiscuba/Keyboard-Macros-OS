use std::fs;
use std::path::Path;

use crate::model::{parse_chord, Document, SCHEMA_VERSION};

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
