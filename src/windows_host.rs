use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{
    GetLastError, GlobalFree, HANDLE, LPARAM, WIN32_ERROR, WPARAM,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, HOT_KEY_MODIFIERS, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
    PM_NOREMOVE, WM_APP, WM_HOTKEY, WM_QUIT,
};

use crate::engine::Engine;
use crate::model::{Action, Chord, Key};

const CF_UNICODETEXT: u32 = 13;
const PASTE_SETTLE: Duration = Duration::from_millis(80);
const WM_RELOAD: u32 = WM_APP + 1;

/// Atajos globales en un hilo propio, para que la ventana use el otro.
pub struct Session {
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
    engine: Arc<Mutex<Engine>>,
    report: Arc<Mutex<String>>,
}

impl Session {
    pub fn spawn(engine: Engine) -> Result<Self, String> {
        let engine = Arc::new(Mutex::new(engine));
        let report = Arc::new(Mutex::new(String::new()));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let engine_for_thread = Arc::clone(&engine);
        let report_for_thread = Arc::clone(&report);
        let thread = thread::spawn(move || {
            worker(engine_for_thread, report_for_thread, ready_tx);
        });
        let thread_id = match ready_rx.recv() {
            Ok(thread_id) => thread_id,
            Err(_) => {
                let _ = thread.join();
                return Err("el hilo de atajos terminó al arrancar".into());
            }
        };
        Ok(Self {
            thread: Some(thread),
            thread_id,
            engine,
            report,
        })
    }

    pub fn reload(&self, engine: Engine) {
        *self.engine.lock().expect("motor") = engine;
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_RELOAD, WPARAM(0), LPARAM(0));
        }
    }

    pub fn report(&self) -> String {
        self.report.lock().expect("estado").clone()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn worker(engine: Arc<Mutex<Engine>>, report: Arc<Mutex<String>>, ready: mpsc::SyncSender<u32>) {
    let thread_id = unsafe { GetCurrentThreadId() };
    let mut installed = apply(&engine, &report);
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    if ready.send(thread_id).is_err() {
        release(&installed);
        return;
    }
    loop {
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if status.0 == 0 || status.0 == -1 {
            break;
        }
        if message.message == WM_HOTKEY {
            let slot = (message.wParam.0 as usize).saturating_sub(1);
            let text = {
                let engine = engine.lock().expect("motor");
                match engine.action(slot) {
                    Some(Action::InsertText { text }) => Some(text.clone()),
                    Some(Action::Unconverted { .. }) | None => None,
                }
            };
            if let Some(text) = text {
                release(&installed);
                if let Err(error) = insert_text(&text) {
                    eprintln!("kmos: no se pudo insertar el texto: {error}");
                }
                installed = apply(&engine, &report);
            }
            continue;
        }
        if message.message == WM_RELOAD {
            release(&installed);
            installed = apply(&engine, &report);
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    release(&installed);
}

fn apply(engine: &Mutex<Engine>, report: &Mutex<String>) -> Vec<i32> {
    let engine = engine.lock().expect("motor");
    let (installed, errors) = install(&engine);
    *report.lock().expect("estado") = binding_report(&engine, &installed, &errors);
    installed
}

fn binding_report(engine: &Engine, installed: &[i32], errors: &[String]) -> String {
    let mut lines = Vec::new();
    if engine.is_empty() {
        lines.push("Sin atajos activos.".to_string());
    } else {
        let labels: Vec<String> = engine
            .bindings()
            .filter(|(index, _, _)| installed.contains(&id_of(*index)))
            .map(|(_, chord, _)| chord.label())
            .collect();
        if labels.is_empty() {
            lines.push("Ningún atajo quedó registrado.".to_string());
        } else {
            lines.push(format!("Escuchando {}", labels.join(", ")));
        }
    }
    lines.extend(errors.iter().cloned());
    lines.join("\n")
}

fn install(engine: &Engine) -> (Vec<i32>, Vec<String>) {
    let mut installed = Vec::new();
    let mut errors = Vec::new();
    for (index, chord, _) in engine.bindings() {
        let id = id_of(index);
        if id == 0 {
            errors.push("hay demasiadas macros".to_string());
            break;
        }
        let modifiers = modifiers_of(chord) | MOD_NOREPEAT;
        let vk = virtual_key(chord.key);
        match unsafe { RegisterHotKey(None, id, modifiers, vk as u32) } {
            Ok(()) => installed.push(id),
            Err(error) => {
                errors.push(format!(
                    "{}: otro programa ya usa ese atajo ({error})",
                    chord.label()
                ));
            }
        }
    }
    (installed, errors)
}

fn id_of(index: usize) -> i32 {
    i32::try_from(index + 1).unwrap_or(0)
}

fn release(ids: &[i32]) {
    for id in ids {
        unsafe {
            let _ = UnregisterHotKey(None, *id);
        }
    }
}

fn modifiers_of(chord: &Chord) -> HOT_KEY_MODIFIERS {
    let mut flags = HOT_KEY_MODIFIERS(0);
    if chord.ctrl {
        flags |= MOD_CONTROL;
    }
    if chord.shift {
        flags |= MOD_SHIFT;
    }
    if chord.alt {
        flags |= MOD_ALT;
    }
    if chord.win {
        flags |= MOD_WIN;
    }
    flags
}

fn virtual_key(key: Key) -> u16 {
    match key {
        Key::Letter(c) => c.to_ascii_uppercase() as u16,
        Key::Digit(c) => c as u16,
        Key::Function(n) => 0x70 + u16::from(n) - 1,
        Key::Space => 0x20,
        Key::Enter => 0x0d,
        Key::Tab => 0x09,
        Key::Escape => 0x1b,
        Key::Left => 0x25,
        Key::Up => 0x26,
        Key::Right => 0x27,
        Key::Down => 0x28,
        Key::Numpad(n) => 0x60 + u16::from(n),
        Key::NumpadAdd => 0x6b,
        Key::NumpadSub => 0x6d,
        Key::NumpadMul => 0x6a,
        Key::NumpadDiv => 0x6f,
        Key::NumpadDec => 0x6e,
    }
}

fn insert_text(text: &str) -> Result<(), String> {
    let previous = unsafe { read_clipboard_text() };
    unsafe { set_clipboard_text(text)? };
    unsafe { send_ctrl_v()? };
    thread::sleep(PASTE_SETTLE);
    let restored = match previous {
        Some(old) => unsafe { set_clipboard_text(&old) },
        None => unsafe { clear_clipboard() },
    };
    if let Err(error) = restored {
        eprintln!("kmos: el texto se pegó, pero no se pudo restaurar el portapapeles: {error}");
    }
    Ok(())
}

unsafe fn read_clipboard_text() -> Option<String> {
    if OpenClipboard(None).is_err() {
        return None;
    }
    let text = if IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok() {
        clipboard_string()
    } else {
        None
    };
    let _ = CloseClipboard();
    text
}

unsafe fn clipboard_string() -> Option<String> {
    let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
    let pointer = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0));
    if pointer.is_null() {
        return None;
    }
    let text = utf16_from_ptr(pointer as *const u16);
    let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
    Some(text)
}

unsafe fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    OpenClipboard(None).map_err(|error| format!("OpenClipboard: {error}"))?;
    if let Err(error) = write_clipboard_units(&units) {
        let _ = CloseClipboard();
        return Err(error);
    }
    CloseClipboard().map_err(|error| format!("CloseClipboard: {error}"))
}

unsafe fn write_clipboard_units(units: &[u16]) -> Result<(), String> {
    EmptyClipboard().map_err(|error| format!("EmptyClipboard: {error}"))?;
    let bytes = units.len() * std::mem::size_of::<u16>();
    let memory = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|error| format!("GlobalAlloc: {error}"))?;
    let pointer = GlobalLock(memory);
    if pointer.is_null() {
        let _ = GlobalFree(Some(memory));
        return Err("GlobalLock no devolvió memoria".into());
    }
    std::ptr::copy_nonoverlapping(units.as_ptr(), pointer as *mut u16, units.len());
    // El último desbloqueo devuelve FALSE con error 0. Eso es el éxito.
    if GlobalUnlock(memory).is_err() && GetLastError() != WIN32_ERROR(0) {
        let _ = GlobalFree(Some(memory));
        return Err("GlobalUnlock falló".into());
    }
    SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0)))
        .map_err(|error| format!("SetClipboardData: {error}"))?;
    Ok(())
}

unsafe fn clear_clipboard() -> Result<(), String> {
    OpenClipboard(None).map_err(|error| format!("OpenClipboard: {error}"))?;
    let cleared = EmptyClipboard();
    let _ = CloseClipboard();
    cleared.map_err(|error| format!("EmptyClipboard: {error}"))
}

unsafe fn send_ctrl_v() -> Result<(), String> {
    // Si Control sigue físicamente pulsado, soltarlo aquí haría que Windows
    // lo diera por levantado. El siguiente H no repetiría el atajo.
    let steps = ctrl_v_steps(ctrl_is_held());
    let inputs: Vec<INPUT> = steps
        .iter()
        .map(|(key, up)| key_input(*key, *up))
        .collect();
    let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    if sent != inputs.len() as u32 {
        return Err(format!("SendInput envió {sent} de {} eventos", inputs.len()));
    }
    Ok(())
}

fn ctrl_is_held() -> bool {
    [VK_CONTROL, VK_LCONTROL, VK_RCONTROL]
        .into_iter()
        .any(key_is_down)
}

fn key_is_down(key: VIRTUAL_KEY) -> bool {
    let state = unsafe { GetAsyncKeyState(i32::from(key.0)) } as u16;
    state & 0x8000 != 0
}

fn ctrl_v_steps(ctrl_already_down: bool) -> Vec<(VIRTUAL_KEY, bool)> {
    let mut steps = Vec::new();
    if !ctrl_already_down {
        steps.push((VK_CONTROL, false));
    }
    steps.push((VK_V, false));
    steps.push((VK_V, true));
    if !ctrl_already_down {
        steps.push((VK_CONTROL, true));
    }
    steps
}

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn utf16_from_ptr(pointer: *const u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0usize;
    unsafe {
        while *pointer.add(length) != 0 {
            length += 1;
            if length > 1_000_000 {
                break;
            }
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_ctrl_is_not_released_by_the_paste() {
        let steps = ctrl_v_steps(true);
        assert!(steps.iter().all(|(key, _)| *key == VK_V));
        assert_eq!(steps, vec![(VK_V, false), (VK_V, true)]);
    }

    #[test]
    fn free_ctrl_is_pressed_for_the_paste_and_then_released() {
        let steps = ctrl_v_steps(false);
        assert_eq!(
            steps,
            vec![
                (VK_CONTROL, false),
                (VK_V, false),
                (VK_V, true),
                (VK_CONTROL, true),
            ]
        );
    }
}
