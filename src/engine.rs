use crate::model::{parse_chord, Action, Chord, Document, Macro};

/// Elige la macro de un atajo. No habla con el sistema operativo.
pub struct Engine {
    macros: Vec<Macro>,
    chords: Vec<Chord>,
}

impl Engine {
    pub fn from_document(document: &Document) -> Result<Self, String> {
        let mut macros = Vec::with_capacity(document.macros.len());
        let mut chords = Vec::with_capacity(document.macros.len());
        for macro_ in &document.macros {
            if !macro_.action.runs_now() {
                continue;
            }
            let Some(hotkey) = &macro_.hotkey else {
                continue;
            };
            if hotkey.key.trim().is_empty() {
                continue;
            }
            chords.push(parse_chord(hotkey)?);
            macros.push(macro_.clone());
        }
        Ok(Self { macros, chords })
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

    pub fn action(&self, slot: usize) -> Option<&Action> {
        self.macros.get(slot).map(|macro_| &macro_.action)
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
        match engine.action(0) {
            Some(Action::InsertText { text }) => assert_eq!(text, "hola"),
            other => panic!("acción inesperada: {other:?}"),
        }
        assert!(engine.action(1).is_none());
    }
}
