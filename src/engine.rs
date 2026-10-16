use crate::model::{parse_chord, Chord, Document, Macro};

/// Elige la macro de un atajo. No habla con el sistema operativo.
pub struct Engine {
    macros: Vec<Macro>,
    chords: Vec<Chord>,
    catalog: Vec<Macro>,
}

impl Engine {
    pub fn from_document(document: &Document) -> Result<Self, String> {
        let mut macros = Vec::with_capacity(document.macros.len());
        let mut chords = Vec::with_capacity(document.macros.len());
        let mut catalog = Vec::new();
        for macro_ in &document.macros {
            if !macro_.action.runs_now() {
                continue;
            }
            catalog.push(macro_.clone());
            let Some(hotkey) = &macro_.hotkey else {
                continue;
            };
            if hotkey.key.trim().is_empty() {
                continue;
            }
            chords.push(parse_chord(hotkey)?);
            macros.push(macro_.clone());
        }
        Ok(Self {
            macros,
            chords,
            catalog,
        })
    }

    pub fn find(&self, id: &str) -> Option<&Macro> {
        self.catalog.iter().find(|macro_| macro_.id == id)
    }

    pub fn catalog(&self) -> &[Macro] {
        &self.catalog
    }

    pub fn len(&self) -> usize {
        self.macros.len()
    }

    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }

    pub fn bindings(&self) -> impl Iterator<Item = (usize, &Chord, &Macro)> {
        self.chords
            .iter()
            .zip(self.macros.iter())
            .enumerate()
            .map(|(index, (chord, macro_))| (index, chord, macro_))
    }

    pub fn slot(&self, slot: usize) -> Option<&Macro> {
        self.macros.get(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Action;

    #[test]
    fn slot_points_at_the_insert_text_action() {
        let document = crate::store::parse(
            r#"{
              "version": 1,
              "macros": [{
                "id": "hola",
                "name": "Saludo",
                "hotkey": { "modifiers": ["ctrl"], "key": "h" },
                "action": { "type": "insert_text", "text": "hola" }
              }]
            }"#,
        )
        .unwrap();
        let engine = Engine::from_document(&document).unwrap();
        match engine.slot(0).map(|macro_| &macro_.action) {
            Some(Action::InsertText { text }) => assert_eq!(text, "hola"),
            other => panic!("acción inesperada: {other:?}"),
        }
        assert!(engine.slot(1).is_none());
    }

    #[test]
    fn a_recording_with_a_hotkey_is_bound() {
        let document = crate::store::parse(
            r#"{
              "version": 1,
              "macros": [{
                "id": "rec",
                "name": "Grabación",
                "hotkey": { "modifiers": [], "key": "f13" },
                "action": {
                  "type": "play",
                  "speed": 100,
                  "repeat": { "kind": "times", "count": 1 },
                  "events": [{ "type": "key", "key": "a", "down": true }]
                }
              }]
            }"#,
        )
        .unwrap();
        let engine = Engine::from_document(&document).unwrap();
        assert_eq!(engine.len(), 1);
        assert!(matches!(engine.slot(0).map(|macro_| &macro_.action), Some(Action::Play { .. })));
    }
}
