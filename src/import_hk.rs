use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::model::{
    key_from_virtual_code, modifiers_from_mask, Action, Document, Hotkey, Macro, SCHEMA_VERSION,
};

pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    pub texts: usize,
    pub unconverted: usize,
    pub without_hotkey: usize,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        format!(
            "Importación lista: {} nuevas, {} actualizadas, {} textos, {} sin convertir, {} sin atajo.",
            self.added, self.updated, self.texts, self.unconverted, self.without_hotkey
        )
    }
}

pub fn import_path(path: &Path, document: &mut Document) -> Result<ImportReport, String> {
    let xml = fs::read_to_string(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    import_xml(&xml, document)
}

pub fn import_xml(xml: &str, document: &mut Document) -> Result<ImportReport, String> {
    let xml = xml.trim_start_matches('\u{feff}');
    let parsed = roxmltree::Document::parse(xml)
        .map_err(|error| format!("el archivo no es XML válido: {error}"))?;
    if parsed
        .descendants()
        .all(|node| !node.has_tag_name("LocalMacros"))
    {
        return Err("ese XML no tiene el bloque LocalMacros de Hot Keyboard".into());
    }
    document.version = SCHEMA_VERSION;

    let mut groups = HashMap::new();
    for node in parsed.descendants().filter(|node| is_macro(*node)) {
        if node.attribute("class") != Some("MMenuMacro") {
            continue;
        }
        let Some(guid) = node.attribute("guid") else {
            continue;
        };
        groups.insert(guid.to_string(), element_text(node, "Name"));
    }

    let mut report = ImportReport {
        added: 0,
        updated: 0,
        texts: 0,
        unconverted: 0,
        without_hotkey: 0,
    };
    for node in parsed.descendants().filter(|node| is_macro(*node)) {
        if node.attribute("class") == Some("MMenuMacro") {
            continue;
        }
        let imported = convert_macro(node, &groups)?;
        if matches!(imported.action, Action::InsertText { .. }) {
            report.texts += 1;
        } else {
            report.unconverted += 1;
        }
        if imported.hotkey.is_none() {
            report.without_hotkey += 1;
        }
        if let Some(existing) = document
            .macros
            .iter_mut()
            .find(|macro_| macro_.id == imported.id)
        {
            *existing = imported;
            report.updated += 1;
        } else {
            document.macros.push(imported);
            report.added += 1;
        }
    }
    crate::store::check(document)?;
    Ok(report)
}

fn is_macro(node: roxmltree::Node<'_, '_>) -> bool {
    node.is_element() && node.has_tag_name("Macro")
}

fn convert_macro(
    node: roxmltree::Node<'_, '_>,
    groups: &HashMap<String, String>,
) -> Result<Macro, String> {
    let class = node.attribute("class").unwrap_or("desconocida");
    let id = node
        .attribute("guid")
        .map(str::trim)
        .filter(|guid| !guid.is_empty())
        .unwrap_or("importada")
        .to_string();
    let name = {
        let name = element_text(node, "Name");
        if name.is_empty() {
            class.to_string()
        } else {
            name
        }
    };
    let parent = child(node, "Parent")
        .and_then(|parent| parent.text())
        .map(str::trim)
        .filter(|parent| !parent.is_empty())
        .and_then(|guid| groups.get(guid))
        .filter(|name| !name.is_empty())
        .cloned();
    let hotkey = hotkey_of(node)?;
    let action = action_of(node, class);
    Ok(Macro {
        id,
        name,
        parent,
        hotkey,
        action,
    })
}

fn hotkey_of(node: roxmltree::Node<'_, '_>) -> Result<Option<Hotkey>, String> {
    let Some(activation) = node
        .descendants()
        .find(|child| child.has_tag_name("Activation") && child.attribute("class") == Some("MMacroActivationHotkey"))
    else {
        return Ok(None);
    };
    let mask = activation
        .attribute("Modifiers")
        .unwrap_or("0")
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    let mask = u32::from_str_radix(mask, 16)
        .map_err(|_| format!("modificadores del atajo no reconocidos: {mask}"))?;
    let code_text = activation.text().unwrap_or("").trim();
    let code_hex = code_text.trim_start_matches("0x").trim_start_matches("0X");
    let code = u32::from_str_radix(code_hex, 16)
        .map_err(|_| format!("código de tecla no reconocido: {code_text}"))?;
    let Some(key) = key_from_virtual_code(code) else {
        return Err(format!(
            "la tecla virtual {code_text} todavía no tiene nombre en este programa"
        ));
    };
    Ok(Some(Hotkey {
        modifiers: modifiers_from_mask(mask),
        key: key.label().to_string(),
    }))
}

fn action_of(node: roxmltree::Node<'_, '_>, class: &str) -> Action {
    if class == "MPasteMacro" {
        let raw = nested(node, "Text")
            .and_then(|text| text.text())
            .unwrap_or("");
        return Action::InsertText {
            text: rtf_to_plain(raw),
        };
    }
    let blob = nested(node, "Events")
        .and_then(|events| events.text())
        .map(str::trim)
        .filter(|events| !events.is_empty())
        .map(str::to_string);
    let note = if class == "MRecordMacro" {
        "Grabación guardada. Todavía no se puede reproducir."
    } else {
        "Este tipo de macro se guardó y todavía no se puede ejecutar."
    };
    Action::Unconverted {
        kind: class.to_string(),
        note: note.to_string(),
        blob,
    }
}

fn nested<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    node.descendants().find(|child| child.has_tag_name(name))
}

fn child<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    node.children().find(|child| child.has_tag_name(name))
}

fn element_text(node: roxmltree::Node<'_, '_>, name: &str) -> String {
    child(node, name)
        .and_then(|child| child.text())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn rtf_to_plain(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    let mut skip_depth = 0i32;
    let mut unicode_fallback = 1usize;
    let mut plain = String::new();
    while index < chars.len() {
        let current = chars[index];
        if current == '\n' || current == '\r' {
            index += 1;
            continue;
        }
        if current == '{' {
            if skip_depth > 0 {
                skip_depth += 1;
            } else if group_is_skipped(&chars, index + 1) {
                skip_depth = 1;
            }
            index += 1;
            continue;
        }
        if current == '}' {
            if skip_depth > 0 {
                skip_depth -= 1;
            }
            index += 1;
            continue;
        }
        if current == '\\' {
            index = consume_control(
                &chars,
                index,
                &mut plain,
                &mut unicode_fallback,
                skip_depth > 0,
            );
            continue;
        }
        if skip_depth == 0 {
            plain.push(current);
        }
        index += 1;
    }
    if plain.ends_with('\n') {
        plain.pop();
    }
    plain
}

fn group_is_skipped(chars: &[char], mut index: usize) -> bool {
    while index < chars.len() && chars[index].is_whitespace() {
        index += 1;
    }
    if index >= chars.len() || chars[index] != '\\' {
        return false;
    }
    index += 1;
    if index < chars.len() && chars[index] == '*' {
        return true;
    }
    let start = index;
    while index < chars.len() && chars[index].is_ascii_alphabetic() {
        index += 1;
    }
    let word: String = chars[start..index].iter().collect();
    matches!(
        word.as_str(),
        "fonttbl" | "colortbl" | "stylesheet" | "info" | "generator"
    )
}

fn consume_control(
    chars: &[char],
    start: usize,
    plain: &mut String,
    unicode_fallback: &mut usize,
    skipping: bool,
) -> usize {
    let mut index = start + 1;
    if index >= chars.len() {
        return index;
    }
    let current = chars[index];
    if current == '\\' || current == '{' || current == '}' {
        if !skipping {
            plain.push(current);
        }
        return index + 1;
    }
    if current == '\'' {
        index += 1;
        if index + 1 < chars.len() {
            let hex: String = chars[index..index + 2].iter().collect();
            index += 2;
            if !skipping {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    plain.push(cp1252(byte));
                }
            }
        }
        return index;
    }
    if !current.is_ascii_alphabetic() {
        if !skipping && current == '~' {
            plain.push('\u{00A0}');
        }
        return index + 1;
    }
    let word_start = index;
    while index < chars.len() && chars[index].is_ascii_alphabetic() {
        index += 1;
    }
    let word: String = chars[word_start..index].iter().collect();
    let number_start = index;
    if index < chars.len() && (chars[index] == '-' || chars[index].is_ascii_digit()) {
        if chars[index] == '-' {
            index += 1;
        }
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
    }
    let number = if number_start < index {
        chars[number_start..index]
            .iter()
            .collect::<String>()
            .parse::<i32>()
            .ok()
    } else {
        None
    };
    if index < chars.len() && chars[index] == ' ' {
        index += 1;
    }
    if skipping {
        return index;
    }
    match word.as_str() {
        "par" | "line" => plain.push('\n'),
        "tab" => plain.push('\t'),
        "uc" => {
            if let Some(count) = number {
                *unicode_fallback = count.max(0) as usize;
            }
        }
        "u" => {
            if let Some(value) = number {
                let code = value as u16;
                if let Some(character) = char::from_u32(code as u32) {
                    plain.push(character);
                }
            }
            let mut remaining = *unicode_fallback;
            while remaining > 0 && index < chars.len() {
                index += 1;
                remaining -= 1;
            }
        }
        _ => {}
    }
    index
}

fn cp1252(byte: u8) -> char {
    match byte {
        0x80 => '\u{20AC}',
        0x82 => '\u{201A}',
        0x83 => '\u{0192}',
        0x84 => '\u{201E}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02C6}',
        0x89 => '\u{2030}',
        0x8A => '\u{0160}',
        0x8B => '\u{2039}',
        0x8C => '\u{0152}',
        0x8E => '\u{017D}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02DC}',
        0x99 => '\u{2122}',
        0x9A => '\u{0161}',
        0x9B => '\u{203A}',
        0x9C => '\u{0153}',
        0x9E => '\u{017E}',
        0x9F => '\u{0178}',
        other => char::from(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::model::{parse_chord, Action};

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<HotKeyboardSettings>
  <LocalMacros>
    <Macro class="MMenuMacro" guid="{MENU}">
      <Name>Local</Name>
      <Activations/>
      <Body><MenuAction>0</MenuAction></Body>
    </Macro>
    <Macro class="MPasteMacro" guid="{PASTE}">
      <Name>Saludo</Name>
      <Parent>{MENU}</Parent>
      <Activations>
        <Activation class="MMacroActivationHotkey" Modifiers="0x06">0x0048</Activation>
      </Activations>
      <Body>
        <Text method="0">{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}{\*\generator Riched20}\pard\fs20 Hola\par}</Text>
      </Body>
    </Macro>
    <Macro class="MPasteMacro" guid="{LIBRE}">
      <Name>Sin atajo</Name>
      <Parent>{MENU}</Parent>
      <Activations/>
      <Body><Text method="0">{\rtf1\pard Solo\par}</Text></Body>
    </Macro>
    <Macro class="MRecordMacro" guid="{GRAB}">
      <Name>Grabacion</Name>
      <Parent>{MENU}</Parent>
      <Activations>
        <Activation class="MMacroActivationHotkey" Modifiers="0x00">0x006D</Activation>
      </Activations>
      <Body><Events>QQ==</Events></Body>
    </Macro>
  </LocalMacros>
</HotKeyboardSettings>
"#;

    #[test]
    fn imports_menu_text_and_recording() {
        let mut document = Document {
            version: SCHEMA_VERSION,
            macros: Vec::new(),
        };
        let report = import_xml(SAMPLE, &mut document).unwrap();
        assert_eq!(report.added, 3);
        assert_eq!(report.texts, 2);
        assert_eq!(report.unconverted, 1);
        assert_eq!(report.without_hotkey, 1);
        assert_eq!(document.macros[0].parent.as_deref(), Some("Local"));
        assert_eq!(
            document.macros[0].action,
            Action::InsertText {
                text: "Hola".into()
            }
        );
        assert_eq!(
            parse_chord(document.macros[0].hotkey.as_ref().unwrap())
                .unwrap()
                .label(),
            "ctrl+shift+h"
        );
        assert!(document.macros[1].hotkey.is_none());
        match &document.macros[2].action {
            Action::Unconverted { kind, blob, .. } => {
                assert_eq!(kind, "MRecordMacro");
                assert_eq!(blob.as_deref(), Some("QQ=="));
            }
            other => panic!("acción inesperada: {other:?}"),
        }
        let engine = Engine::from_document(&document).unwrap();
        assert_eq!(engine.len(), 1);
    }

    #[test]
    fn second_import_updates_the_same_id() {
        let mut document = Document {
            version: SCHEMA_VERSION,
            macros: Vec::new(),
        };
        import_xml(SAMPLE, &mut document).unwrap();
        let report = import_xml(SAMPLE, &mut document).unwrap();
        assert_eq!(report.added, 0);
        assert_eq!(report.updated, 3);
        assert_eq!(document.macros.len(), 3);
    }

    #[test]
    fn rtf_keeps_line_breaks_accents_and_drops_the_font_table() {
        let plain = rtf_to_plain(
            r#"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fnil Segoe UI;}}\pard uno\par dos\par}"#,
        );
        assert_eq!(plain, "uno\ndos");
        assert_eq!(rtf_to_plain(r#"{\rtf1\pard caf\'e9 \u243?l\par}"#), "café ól");
        assert_eq!(rtf_to_plain(r#"{\rtf1\pard \u8364? \'80\par}"#), "€ €");
    }

    #[test]
    fn reads_the_local_file_without_keeping_rtf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../Hot Keyboard Pro/HotKeyboardData.xml");
        if !path.exists() {
            return;
        }
        let xml = fs::read_to_string(&path).unwrap();
        let mut document = Document {
            version: SCHEMA_VERSION,
            macros: Vec::new(),
        };
        let report = import_xml(&xml, &mut document).unwrap();
        assert_eq!(report.texts, 11);
        assert_eq!(report.unconverted, 1);
        assert_eq!(report.without_hotkey, 1);
        assert_eq!(document.macros.len(), 12);
        assert!(document
            .macros
            .iter()
            .all(|macro_| macro_.parent.as_deref() == Some("Local")));
        let mut newlines = 0usize;
        for macro_ in &document.macros {
            match &macro_.action {
                Action::InsertText { text } => {
                    assert!(!text.is_empty());
                    assert!(!text.contains("rtf"));
                    assert!(!text.contains("Segoe"));
                    assert!(!text.contains("fonttbl"));
                    if text.contains('\n') {
                        newlines += 1;
                    }
                }
                Action::Unconverted { kind, blob, .. } => {
                    assert_eq!(kind, "MRecordMacro");
                    assert!(blob.as_ref().is_some_and(|blob| !blob.is_empty()));
                }
            }
        }
        assert_eq!(newlines, 4);
        let labels: Vec<_> = document
            .macros
            .iter()
            .filter_map(|macro_| macro_.hotkey.as_ref())
            .map(|hotkey| parse_chord(hotkey).unwrap().label())
            .collect();
        assert!(labels.iter().any(|label| label == "ctrl+h"));
        assert!(labels.iter().any(|label| label == "num_sub"));
        let engine = Engine::from_document(&document).unwrap();
        assert_eq!(engine.len(), 10);
    }
}
