use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, GetLastError, GlobalFree, HANDLE, HWND, LPARAM, WIN32_ERROR, WPARAM, FALSE, TRUE,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, SC_SCREENSAVE,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use windows::Win32::UI::Shell::SHEmptyRecycleBinW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, HOT_KEY_MODIFIERS, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
    MOD_WIN, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
    RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_ESCAPE, VK_LCONTROL,
    VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP, VK_RCONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EnumWindows, GetClassNameW, GetDesktopWindow,
    GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, PeekMessageW, PostMessageW,
    PostThreadMessageW, SendMessageW, SetForegroundWindow, SetWindowPos, SetWindowsHookExW,
    ShowWindow, TranslateMessage, UnhookWindowsHookEx, GWL_EXSTYLE, HWND_BOTTOM, HWND_NOTOPMOST,
    HWND_TOP, HWND_TOPMOST, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SW_SHOWMINNOACTIVE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WM_CLOSE, WM_SYSCOMMAND, WS_EX_TOPMOST,
    GetWindowRect, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, MSG, PM_NOREMOVE, PM_REMOVE, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP,
    WM_HOTKEY, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYUP,
};

use crate::engine::Engine;
use crate::model::{
    adjusted_volume, any_part_matches, apply_replace, eval_expression, format_value,
    key_from_virtual_code, media_virtual_key, parse_key, power_flags, window_box, Action, Chord,
    Condition, Delay, InputEvent, Key, Macro, MediaOp, MouseButton, PowerOp, Repeat, ReplaceRule,
    ShotDest, ShotSource, Trigger, VolumeOp, WindowOp, CHAIN_LIMIT, wait_millis,
};

const CF_UNICODETEXT: u32 = 13;
const PASTE_SETTLE: Duration = Duration::from_millis(80);
const COPY_SETTLE: Duration = Duration::from_millis(160);
const WM_RELOAD: u32 = WM_APP + 1;
const WM_RUN: u32 = WM_APP + 2;
const WM_TICK: u32 = WM_APP + 3;

/// Atajos globales en un hilo propio, para que la ventana use el otro.
pub struct Session {
    thread: Option<JoinHandle<()>>,
    thread_id: u32,
    engine: Arc<Mutex<Engine>>,
    report: Arc<Mutex<String>>,
    variables: Arc<Mutex<BTreeMap<String, String>>>,
}

impl Session {
    pub fn spawn(
        engine: Engine,
        path: PathBuf,
        variables: BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let engine = Arc::new(Mutex::new(engine));
        let report = Arc::new(Mutex::new(String::new()));
        let variables = Arc::new(Mutex::new(variables));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let engine_for_thread = Arc::clone(&engine);
        let report_for_thread = Arc::clone(&report);
        let variables_for_thread = Arc::clone(&variables);
        let thread = thread::spawn(move || {
            worker(
                engine_for_thread,
                report_for_thread,
                variables_for_thread,
                path,
                ready_tx,
            );
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
            variables,
        })
    }

    pub fn variable_snapshot(&self) -> BTreeMap<String, String> {
        self.variables.lock().expect("variables").clone()
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

fn worker(
    engine: Arc<Mutex<Engine>>,
    report: Arc<Mutex<String>>,
    variables: Arc<Mutex<BTreeMap<String, String>>>,
    path: PathBuf,
    ready: mpsc::SyncSender<u32>,
) {
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
    WATCH_THREAD.store(thread_id, Ordering::SeqCst);
    refresh_watch(&engine);
    let hooks = install_watch();
    let ticker = thread_id;
    let ticker_stop = Arc::new(AtomicBool::new(false));
    let ticker_flag = Arc::clone(&ticker_stop);
    thread::spawn(move || {
        while !ticker_flag.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            unsafe {
                let _ = PostThreadMessageW(ticker, WM_TICK, WPARAM(0), LPARAM(0));
            }
        }
    });
    release(&installed);
    run_startup(&engine, &variables, &path);
    installed = apply(&engine, &report);
    loop {
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if status.0 == 0 || status.0 == -1 {
            break;
        }
        if message.message == WM_HOTKEY {
            let slot = (message.wParam.0 as usize).saturating_sub(1);
            let chosen = {
                let engine = engine.lock().expect("motor");
                engine.slot(slot).cloned()
            };
            if let Some(chosen) = chosen {
                release(&installed);
                if let Err(error) = execute(&engine, &variables, &path, &chosen, 0) {
                    eprintln!("kmos: {error}");
                }
                installed = apply(&engine, &report);
            }
            continue;
        }
        if message.message == WM_RUN {
            let pending: Vec<String> = WATCH_PENDING.lock().expect("pendientes").drain(..).collect();
            release(&installed);
            for id in pending {
                let chosen = {
                    let engine = engine.lock().expect("motor");
                    engine.find(&id).cloned()
                };
                if let Some(chosen) = chosen {
                    if let Err(error) = execute(&engine, &variables, &path, &chosen, 0) {
                        eprintln!("kmos: {error}");
                    }
                }
            }
            installed = apply(&engine, &report);
            continue;
        }
        if message.message == WM_TICK {
            release(&installed);
            if let Err(error) = tick(&engine, &variables, &path) {
                eprintln!("kmos: {error}");
            }
            installed = apply(&engine, &report);
            continue;
        }
        if message.message == WM_RELOAD {
            release(&installed);
            installed = apply(&engine, &report);
            refresh_watch(&engine);
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    ticker_stop.store(true, Ordering::Relaxed);
    drop_watch(hooks);
    release(&installed);
}

fn execute(
    engine: &Arc<Mutex<Engine>>,
    variables: &Arc<Mutex<BTreeMap<String, String>>>,
    path: &Path,
    chosen: &Macro,
    depth: u32,
) -> Result<(), String> {
    if depth >= CHAIN_LIMIT {
        return Err("la cadena es demasiado larga".into());
    }
    if !condition_allows(chosen.condition.as_ref(), variables) {
        return Ok(());
    }
    match &chosen.action {
        Action::Run { id } => {
            let next = {
                let engine = engine.lock().expect("motor");
                engine.find(id).cloned()
            };
            let Some(next) = next else {
                return Err(format!("no está la macro {id}"));
            };
            execute(engine, variables, path, &next, depth + 1)
        }
        Action::Sequence { ids } => {
            for id in ids {
                let next = {
                    let engine = engine.lock().expect("motor");
                    engine.find(id).cloned()
                };
                let Some(next) = next else {
                    return Err(format!("no está la macro {id}"));
                };
                execute(engine, variables, path, &next, depth + 1)?;
            }
            Ok(())
        }
        Action::Delay { delay } => wait_delay(delay),
        Action::Expression { source, variable } => store_expression(source, variable, variables),
        Action::SaveVars { names } => persist_variables(path, names, variables),
        Action::Power { op, force } => perform_power(*op, *force),
        Action::Media { op } => tap_media(*op),
        Action::Reminder { title, text } => show_reminder(title, text),
        Action::Screenshot {
            source,
            dest,
            overwrite,
        } => capture_screen(source, dest, *overwrite),
        other => {
            let Some(job) = job_of(other) else {
                return Ok(());
            };
            run_job(job)
        }
    }
}

fn job_of(action: &Action) -> Option<Job> {
    match action {
        Action::InsertText { text } => Some(Job::Text(text.clone())),
        Action::Play { events, speed, repeat } => Some(Job::Play {
            events: events.clone(),
            speed: *speed,
            repeat: repeat.clone(),
        }),
        Action::OpenFile { path, program, arguments } => Some(Job::OpenFile {
            path: path.clone(),
            program: program.clone(),
            arguments: arguments.clone(),
        }),
        Action::Launch {
            program,
            arguments,
            directory,
            minimized,
            activate_existing,
            as_admin,
        } => Some(Job::Launch {
            program: program.clone(),
            arguments: arguments.clone(),
            directory: directory.clone(),
            minimized: *minimized,
            activate_existing: *activate_existing,
            as_admin: *as_admin,
        }),
        Action::OpenFolder { path } => Some(Job::OpenFolder { path: path.clone() }),
        Action::OpenPage { url, browser, new_window } => Some(Job::OpenPage {
            url: url.clone(),
            browser: browser.clone(),
            new_window: *new_window,
        }),
        Action::Window { op } => Some(Job::Window { op: op.clone() }),
        Action::Volume { op } => Some(Job::Volume { op: op.clone() }),
        Action::Replace {
            rules,
            max_iterations,
            max_length_kb,
        } => Some(Job::Replace {
            rules: rules.clone(),
            max_iterations: *max_iterations,
            max_length_kb: *max_length_kb,
        }),
        _ => None,
    }
}

fn condition_allows(
    condition: Option<&Condition>,
    variables: &Arc<Mutex<BTreeMap<String, String>>>,
) -> bool {
    let Some(condition) = condition else {
        return true;
    };
    let (matched, invert) = match condition {
        Condition::Title { text, invert } => (any_part_matches(&foreground_label(Label::Title), text), *invert),
        Condition::Class { text, invert } => (any_part_matches(&foreground_label(Label::Class), text), *invert),
        Condition::Process { text, invert } => (any_part_matches(&foreground_label(Label::Process), text), *invert),
        Condition::Expression { source, invert } => {
            let map = variables.lock().expect("variables").clone();
            (eval_expression(source, &map).unwrap_or(0.0) != 0.0, *invert)
        }
    };
    if invert { !matched } else { matched }
}

enum Label { Title, Class, Process }

fn foreground_label(kind: Label) -> String {
    let Ok(hwnd) = foreground() else {
        return String::new();
    };
    match kind {
        Label::Title => window_text(hwnd),
        Label::Class => class_text(hwnd),
        Label::Process => process_text(hwnd),
    }
}

fn window_text(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

fn class_text(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

fn process_text(hwnd: HWND) -> String {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return String::new();
    }
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return String::new();
    };
    let mut buffer = [0u16; 512];
    let mut size = buffer.len() as u32;
    let name = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    if name.is_err() {
        return String::new();
    }
    let full = String::from_utf16_lossy(&buffer[..size as usize]);
    full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string()
}

fn wait_delay(delay: &Delay) -> Result<(), String> {
    match delay {
        Delay::Pause { millis } => {
            thread::sleep(Duration::from_millis(u64::from(*millis)));
            Ok(())
        }
        Delay::UntilWindow { title, millis } => {
            let deadline = Instant::now() + Duration::from_millis(u64::from(*millis));
            let needle = title.trim().to_lowercase();
            while Instant::now() < deadline {
                if window_present(&needle) {
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(200));
            }
            Err("la ventana no apareció".into())
        }
    }
}

fn window_present(title: &str) -> bool {
    let mut search = TitleSearch {
        title: title.to_string(),
        hwnd: HWND::default(),
    };
    unsafe {
        let _ = EnumWindows(Some(find_title), LPARAM(&mut search as *mut TitleSearch as isize));
    }
    !search.hwnd.0.is_null()
}

fn store_expression(
    source: &str,
    variable: &str,
    variables: &Arc<Mutex<BTreeMap<String, String>>>,
) -> Result<(), String> {
    let mut map = variables.lock().expect("variables");
    let value = eval_expression(source, &map)?;
    let text = format_value(value)?;
    if !variable.trim().is_empty() {
        map.insert(variable.trim().to_string(), text);
    }
    Ok(())
}

fn persist_variables(
    path: &Path,
    names: &[String],
    variables: &Arc<Mutex<BTreeMap<String, String>>>,
) -> Result<(), String> {
    let map = variables.lock().expect("variables").clone();
    let mut document = crate::store::load(path)?;
    for name in names {
        let name = name.trim();
        let value = map
            .get(name)
            .ok_or_else(|| format!("la variable {name} no tiene valor"))?;
        document.variables.insert(name.to_string(), value.clone());
    }
    crate::store::save(path, &document)
}

fn show_reminder(title: &str, text: &str) -> Result<(), String> {
    let title = if title.trim().is_empty() {
        "Aviso".to_string()
    } else {
        title.to_string()
    };
    let text = text.to_string();
    thread::spawn(move || {
        let title = wide(&title);
        let text = wide(&text);
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                windows::core::PCWSTR(text.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::MB_OK
                    | windows::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION,
            );
        }
    });
    Ok(())
}

fn tap_media(op: MediaOp) -> Result<(), String> {
    let key = match media_virtual_key(op) {
        0xb0 => VK_MEDIA_NEXT_TRACK,
        0xb1 => VK_MEDIA_PREV_TRACK,
        0xb2 => VK_MEDIA_STOP,
        _ => VK_MEDIA_PLAY_PAUSE,
    };
    send_tap(key)
}

fn send_tap(key: VIRTUAL_KEY) -> Result<(), String> {
    let inputs = [key_input(key, false), key_input(key, true)];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(format!("SendInput envió {sent} de {} eventos", inputs.len()));
    }
    Ok(())
}

fn perform_power(op: PowerOp, force: bool) -> Result<(), String> {
    match op {
        PowerOp::Lock => unsafe { windows::Win32::System::Shutdown::LockWorkStation() }
            .map_err(|error| format!("no se pudo bloquear la sesión: {error}")),
        PowerOp::Sleep => suspend(false),
        PowerOp::Hibernate => suspend(true),
        PowerOp::Logoff | PowerOp::Shutdown | PowerOp::Restart => {
            let _ = enable_shutdown();
            let flags = power_flags(op, force).unwrap_or(0);
            unsafe {
                windows::Win32::System::Shutdown::ExitWindowsEx(
                    windows::Win32::System::Shutdown::EXIT_WINDOWS_FLAGS(flags),
                    windows::Win32::System::Shutdown::SHTDN_REASON_FLAG_PLANNED,
                )
            }
            .map_err(|error| format!("Windows no aceptó la operación: {error}"))
        }
    }
}

fn suspend(hibernate: bool) -> Result<(), String> {
    let ok = unsafe { windows::Win32::System::Power::SetSuspendState(hibernate, false, false) };
    if ok {
        Ok(())
    } else {
        Err("Windows no aceptó la suspensión".into())
    }
}

fn enable_shutdown() -> Result<(), String> {
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::Security::LUID_AND_ATTRIBUTES;
    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .map_err(|error| format!("no se pudo abrir el token: {error}"))?;
        let mut luid = windows::Win32::Foundation::LUID::default();
        if let Err(error) = LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid) {
            let _ = CloseHandle(token);
            return Err(format!("no se encontró el privilegio: {error}"));
        }
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let adjusted = AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None);
        let _ = CloseHandle(token);
        adjusted.map_err(|error| format!("no se pudo habilitar el apagado: {error}"))
    }
}

fn capture_screen(source: &ShotSource, dest: &ShotDest, overwrite: bool) -> Result<(), String> {
    let (width, height, pixels) = grab(source)?;
    match dest {
        ShotDest::Clipboard => set_clipboard_dib(width, height, &pixels),
        ShotDest::File { path } => {
            let path = path.trim();
            if std::path::Path::new(path).exists() && !overwrite {
                return Err("la captura ya existe".into());
            }
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .map_err(|error| format!("no se pudo crear la carpeta: {error}"))?;
                }
            }
            let bytes = bmp_file(width, height, &pixels);
            std::fs::write(path, bytes).map_err(|error| format!("no se pudo guardar la captura: {error}"))
        }
    }
}

fn grab(source: &ShotSource) -> Result<(i32, i32, Vec<u8>), String> {
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ, SRCCOPY,
    };
    let (left, top, width, height) = shot_box(source)?;
    unsafe {
        let screen = GetDC(None);
        if screen.0.is_null() {
            return Err("no se pudo leer la pantalla".into());
        }
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        if memory.0.is_null() || bitmap.0.is_null() {
            if !memory.0.is_null() {
                let _ = DeleteDC(memory);
            }
            if !bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            ReleaseDC(None, screen);
            return Err("no se pudo preparar la captura".into());
        }
        let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
        let copied = BitBlt(memory, 0, 0, width, height, Some(screen), left, top, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: height,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        let lines = if copied.is_ok() {
            GetDIBits(
                memory,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr() as *mut _),
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };
        if !previous.0.is_null() {
            SelectObject(memory, previous);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        if lines == 0 {
            return Err("no se pudo leer la captura".into());
        }
        Ok((width, height, pixels))
    }
}

fn shot_box(source: &ShotSource) -> Result<(i32, i32, i32, i32), String> {
    match source {
        ShotSource::Screen => {
            let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            fit_box(left, top, width, height)
        }
        ShotSource::Window => {
            let hwnd = foreground()?;
            let mut rect = windows::Win32::Foundation::RECT::default();
            unsafe { GetWindowRect(hwnd, &mut rect) }
                .map_err(|error| format!("no se pudo medir la ventana: {error}"))?;
            fit_box(rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top)
        }
        ShotSource::Region { left, top, right, bottom } => {
            fit_box(*left, *top, right - left, bottom - top)
        }
    }
}

fn fit_box(left: i32, top: i32, width: i32, height: i32) -> Result<(i32, i32, i32, i32), String> {
    if width <= 0 || height <= 0 || width > 8_000 || height > 8_000 {
        Err("la captura no tiene un tamaño usable".into())
    } else {
        Ok((left, top, width, height))
    }
}

fn bmp_file(width: i32, height: i32, pixels: &[u8]) -> Vec<u8> {
    let header = 14u32;
    let info = 40u32;
    let size = header + info + pixels.len() as u32;
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(header + info).to_le_bytes());
    bytes.extend_from_slice(&info.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0u8; 16]);
    bytes.extend_from_slice(pixels);
    bytes
}

fn set_clipboard_dib(width: i32, height: i32, pixels: &[u8]) -> Result<(), String> {
    let mut units = Vec::with_capacity(40 + pixels.len());
    units.extend_from_slice(&40u32.to_le_bytes());
    units.extend_from_slice(&width.to_le_bytes());
    units.extend_from_slice(&height.to_le_bytes());
    units.extend_from_slice(&1u16.to_le_bytes());
    units.extend_from_slice(&32u16.to_le_bytes());
    units.extend_from_slice(&0u32.to_le_bytes());
    units.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    units.extend_from_slice(&[0u8; 16]);
    units.extend_from_slice(pixels);
    unsafe {
        OpenClipboard(None).map_err(|error| format!("OpenClipboard: {error}"))?;
        if let Err(error) = write_clipboard_handle(8, &units) {
            let _ = CloseClipboard();
            return Err(error);
        }
        CloseClipboard().map_err(|error| format!("CloseClipboard: {error}"))
    }
}

unsafe fn write_clipboard_handle(format: u32, bytes: &[u8]) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    EmptyClipboard().map_err(|error| format!("EmptyClipboard: {error}"))?;
    let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|error| format!("GlobalAlloc: {error}"))?;
    let pointer = GlobalLock(memory);
    if pointer.is_null() {
        let _ = GlobalFree(Some(memory));
        return Err("GlobalLock no devolvió memoria".into());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer as *mut u8, bytes.len());
    if GlobalUnlock(memory).is_err() && GetLastError() != WIN32_ERROR(0) {
        let _ = GlobalFree(Some(memory));
        return Err("GlobalUnlock falló".into());
    }
    SetClipboardData(format, Some(HANDLE(memory.0))).map_err(|error| format!("SetClipboardData: {error}"))?;
    Ok(())
}

fn run_startup(
    engine: &Arc<Mutex<Engine>>,
    variables: &Arc<Mutex<BTreeMap<String, String>>>,
    path: &Path,
) {
    let chosen: Vec<Macro> = {
        let engine = engine.lock().expect("motor");
        engine
            .catalog()
            .iter()
            .filter(|macro_| matches!(macro_.trigger, Some(Trigger::Startup)))
            .cloned()
            .collect()
    };
    for chosen in chosen {
        if let Err(error) = execute(engine, variables, path, &chosen, 0) {
            eprintln!("kmos: {error}");
        }
    }
}

fn tick(
    engine: &Arc<Mutex<Engine>>,
    variables: &Arc<Mutex<BTreeMap<String, String>>>,
    path: &Path,
) -> Result<(), String> {
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let minute = (now.wHour, now.wMinute);
    let fresh = {
        let mut last = LAST_MINUTE.lock().expect("hora");
        let fresh = *last != Some(minute);
        if fresh {
            *last = Some(minute);
        }
        fresh
    };
    let catalog = {
        let engine = engine.lock().expect("motor");
        engine.catalog().to_vec()
    };
    let current = foreground_label(Label::Title);
    for chosen in catalog {
        match &chosen.trigger {
            Some(Trigger::Schedule { hour, minute })
                if fresh && u16::from(*hour) == now.wHour && u16::from(*minute) == now.wMinute =>
            {
                execute(engine, variables, path, &chosen, 0)?;
            }
            Some(Trigger::Window { title }) => {
                let matches = any_part_matches(&current, title);
                let mut seen = SEEN_WINDOWS.lock().expect("ventanas");
                let was = seen.get(&chosen.id).copied().unwrap_or(false);
                seen.insert(chosen.id.clone(), matches);
                drop(seen);
                if matches && !was {
                    execute(engine, variables, path, &chosen, 0)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct WatchItem {
    id: String,
    kind: WatchKind,
}

enum WatchKind {
    Text(String),
    Gesture(String),
    Tap { key: String, count: u32 },
}

struct WatchState {
    items: Vec<WatchItem>,
    buffer: String,
    tap_key: String,
    tap_count: u32,
    tap_at: Option<Instant>,
    gesture: String,
    drawing: bool,
    last: Option<(i32, i32)>,
}

static WATCH_THREAD: AtomicU32 = AtomicU32::new(0);
static WATCH_PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());
static WATCH: Mutex<WatchState> = Mutex::new(WatchState {
    items: Vec::new(),
    buffer: String::new(),
    tap_key: String::new(),
    tap_count: 0,
    tap_at: None,
    gesture: String::new(),
    drawing: false,
    last: None,
});
static LAST_MINUTE: Mutex<Option<(u16, u16)>> = Mutex::new(None);
static SEEN_WINDOWS: Mutex<BTreeMap<String, bool>> = Mutex::new(BTreeMap::new());

fn refresh_watch(engine: &Arc<Mutex<Engine>>) {
    let items = {
        let engine = engine.lock().expect("motor");
        engine
            .catalog()
            .iter()
            .filter_map(|macro_| {
                let kind = match macro_.trigger.as_ref()? {
                    Trigger::Text { phrase } => WatchKind::Text(phrase.trim().to_lowercase()),
                    Trigger::Gesture { directions } => WatchKind::Gesture(directions.trim().to_string()),
                    Trigger::Multitap { key, count } => WatchKind::Tap {
                        key: key.trim().to_string(),
                        count: *count,
                    },
                    _ => return None,
                };
                Some(WatchItem {
                    id: macro_.id.clone(),
                    kind,
                })
            })
            .collect()
    };
    WATCH.lock().expect("vigía").items = items;
}

fn install_watch() -> Option<(windows::Win32::UI::WindowsAndMessaging::HHOOK, windows::Win32::UI::WindowsAndMessaging::HHOOK)> {
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(watch_keyboard), None, 0) }.ok()?;
    let mouse = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(watch_mouse), None, 0) } {
        Ok(mouse) => mouse,
        Err(_) => {
            unsafe { let _ = UnhookWindowsHookEx(keyboard); }
            return None;
        }
    };
    Some((keyboard, mouse))
}

fn drop_watch(hooks: Option<(windows::Win32::UI::WindowsAndMessaging::HHOOK, windows::Win32::UI::WindowsAndMessaging::HHOOK)>) {
    if let Some((keyboard, mouse)) = hooks {
        unsafe {
            let _ = UnhookWindowsHookEx(keyboard);
            let _ = UnhookWindowsHookEx(mouse);
        }
    }
}

unsafe extern "system" fn watch_keyboard(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if code == 0 && !CAPTURE_ALIVE.load(Ordering::Relaxed) {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let down = wparam.0 == 0x0100 || wparam.0 == 0x0104;
        if down {
            if let Some(key) = key_from_virtual_code(info.vkCode) {
                note_key(&key);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn watch_mouse(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if code == 0 && !CAPTURE_ALIVE.load(Ordering::Relaxed) {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let message = wparam.0 as u32;
        if message == WM_LBUTTONDOWN {
            note_gesture_start(info.pt.x, info.pt.y);
        } else if message == WM_MOUSEMOVE {
            note_gesture_move(info.pt.x, info.pt.y);
        } else if message == WM_LBUTTONUP {
            note_gesture_end();
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn note_key(key: &Key) {
    let mut watch = WATCH.lock().expect("vigía");
    if let Some(character) = match key {
        Key::Letter(character) => Some(character.to_ascii_lowercase()),
        Key::Digit(character) => Some(*character),
        Key::Space => Some(' '),
        _ => None,
    } {
        watch.buffer.push(character);
        if watch.buffer.chars().count() > 40 {
            watch.buffer = watch.buffer.chars().skip(watch.buffer.chars().count() - 40).collect();
        }
        let buffer = watch.buffer.clone();
        let hit = watch.items.iter().find_map(|item| match &item.kind {
            WatchKind::Text(phrase) if buffer.ends_with(phrase) => Some(item.id.clone()),
            _ => None,
        });
        if hit.is_some() {
            watch.buffer.clear();
        }
        if let Some(id) = hit {
            queue_run(id);
        }
    }
    let label = key.label();
    let now = Instant::now();
    if watch.tap_key == label && watch.tap_at.is_some_and(|at| now.duration_since(at) < Duration::from_millis(450)) {
        watch.tap_count = watch.tap_count.saturating_add(1);
    } else {
        watch.tap_key = label.to_string();
        watch.tap_count = 1;
    }
    watch.tap_at = Some(now);
    let count = watch.tap_count;
    let tap_key = watch.tap_key.clone();
    let hit = watch.items.iter().find_map(|item| match &item.kind {
        WatchKind::Tap { key, count: needed } if key == &tap_key && *needed == count => Some(item.id.clone()),
        _ => None,
    });
    if hit.is_some() {
        watch.tap_count = 0;
    }
    if let Some(id) = hit {
        queue_run(id);
    }
}

fn note_gesture_start(x: i32, y: i32) {
    let mut watch = WATCH.lock().expect("vigía");
    watch.drawing = true;
    watch.gesture.clear();
    watch.last = Some((x, y));
}

fn note_gesture_move(x: i32, y: i32) {
    let mut watch = WATCH.lock().expect("vigía");
    if !watch.drawing {
        return;
    }
    let Some((previous_x, previous_y)) = watch.last else {
        return;
    };
    let dx = x - previous_x;
    let dy = y - previous_y;
    if dx.abs() + dy.abs() < 24 {
        return;
    }
    let direction = if dx.abs() > dy.abs() {
        if dx > 0 { 'R' } else { 'L' }
    } else if dy > 0 {
        'D'
    } else {
        'U'
    };
    if watch.gesture.chars().last() != Some(direction) {
        watch.gesture.push(direction);
        if watch.gesture.len() > 16 {
            watch.gesture.remove(0);
        }
    }
    watch.last = Some((x, y));
}

fn note_gesture_end() {
    let mut watch = WATCH.lock().expect("vigía");
    watch.drawing = false;
    let gesture = watch.gesture.clone();
    let hit = watch.items.iter().find_map(|item| match &item.kind {
        WatchKind::Gesture(directions) if directions == &gesture => Some(item.id.clone()),
        _ => None,
    });
    watch.gesture.clear();
    if let Some(id) = hit {
        queue_run(id);
    }
}

fn queue_run(id: String) {
    WATCH_PENDING.lock().expect("pendientes").push(id);
    let thread_id = WATCH_THREAD.load(Ordering::SeqCst);
    if thread_id != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_RUN, WPARAM(0), LPARAM(0));
        }
    }
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
        Key::Ctrl => 0x11,
        Key::Shift => 0x10,
        Key::Alt => 0x12,
        Key::Win => 0x5b,
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
    ctrl_key_steps(VK_V, ctrl_already_down)
}

fn ctrl_c_steps(ctrl_already_down: bool) -> Vec<(VIRTUAL_KEY, bool)> {
    ctrl_key_steps(VK_C, ctrl_already_down)
}

fn ctrl_key_steps(key: VIRTUAL_KEY, ctrl_already_down: bool) -> Vec<(VIRTUAL_KEY, bool)> {
    let mut steps = Vec::new();
    if !ctrl_already_down {
        steps.push((VK_CONTROL, false));
    }
    steps.push((key, false));
    steps.push((key, true));
    if !ctrl_already_down {
        steps.push((VK_CONTROL, true));
    }
    steps
}

fn replace_selection(rules: &[ReplaceRule], max_iterations: u32, max_length_kb: u32) -> Result<(), String> {
    let previous = unsafe { read_clipboard_text() };
    let copied = (|| {
        unsafe { clear_clipboard()? };
        unsafe { send_ctrl_c()? };
        thread::sleep(COPY_SETTLE);
        unsafe { read_clipboard_text() }
            .filter(|text| !text.is_empty())
            .ok_or_else(|| "no había texto seleccionado".to_string())
    })();
    let source = match copied {
        Ok(source) => source,
        Err(error) => {
            restore_clipboard(previous);
            return Err(error);
        }
    };
    let output = match apply_replace(&source, rules, max_iterations, max_length_kb) {
        Ok(output) => output,
        Err(error) => {
            restore_clipboard(previous);
            return Err(error);
        }
    };
    restore_clipboard(previous);
    if output == source {
        return Ok(());
    }
    insert_text(&output)
}

fn restore_clipboard(previous: Option<String>) {
    let restored = match previous {
        Some(old) => unsafe { set_clipboard_text(&old) },
        None => unsafe { clear_clipboard() },
    };
    if let Err(error) = restored {
        eprintln!("kmos: no se pudo restaurar el portapapeles: {error}");
    }
}

unsafe fn send_ctrl_c() -> Result<(), String> {
    let steps = ctrl_c_steps(ctrl_is_held());
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

enum Job {
    Text(String),
    Play {
        events: Vec<InputEvent>,
        speed: u32,
        repeat: Repeat,
    },
    OpenFile {
        path: String,
        program: String,
        arguments: String,
    },
    Launch {
        program: String,
        arguments: String,
        directory: String,
        minimized: bool,
        activate_existing: bool,
        as_admin: bool,
    },
    OpenFolder { path: String },
    OpenPage {
        url: String,
        browser: String,
        new_window: bool,
    },
    Window { op: WindowOp },
    Volume { op: VolumeOp },
    Replace {
        rules: Vec<ReplaceRule>,
        max_iterations: u32,
        max_length_kb: u32,
    },
}

fn run_job(job: Job) -> Result<(), String> {
    match job {
        Job::Text(text) => insert_text(&text),
        Job::Play {
            events,
            speed,
            repeat,
        } => play(&events, speed, &repeat),
        Job::OpenFile {
            path,
            program,
            arguments,
        } => open_path(&path, &program, &arguments),
        Job::Launch {
            program,
            arguments,
            directory,
            minimized,
            activate_existing,
            as_admin,
        } => launch_program(
            &program,
            &arguments,
            &directory,
            minimized,
            activate_existing,
            as_admin,
        ),
        Job::OpenFolder { path } => open_folder(&path),
        Job::OpenPage {
            url,
            browser,
            new_window,
        } => open_page(&url, &browser, new_window),
        Job::Window { op } => arrange_window(&op),
        Job::Volume { op } => change_volume(&op),
        Job::Replace {
            rules,
            max_iterations,
            max_length_kb,
        } => replace_selection(&rules, max_iterations, max_length_kb),
    }
}

struct FolderRequest {
    path: String,
    verb: &'static str,
}

fn folder_request(path: &str) -> FolderRequest {
    FolderRequest {
        path: path.trim().to_string(),
        verb: "explore",
    }
}

fn open_folder(path: &str) -> Result<(), String> {
    let request = folder_request(path);
    shell_execute(
        &request.path,
        "",
        "",
        request.verb,
        windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0,
    )
    .map_err(|error| format!("no se pudo abrir la carpeta: {error}"))
}

struct PageRequest {
    file: String,
    parameters: String,
}

fn page_request(url: &str, browser: &str, new_window: bool) -> PageRequest {
    let url = url.trim();
    let browser = browser.trim();
    if browser.is_empty() {
        return PageRequest {
            file: url.to_string(),
            parameters: String::new(),
        };
    }
    let mut parameters = String::new();
    if new_window {
        if let Some(flag) = new_window_flag(browser) {
            parameters.push_str(flag);
            parameters.push(' ');
        }
    }
    parameters.push_str(&quote_arg(url));
    PageRequest {
        file: browser.to_string(),
        parameters,
    }
}

fn new_window_flag(browser: &str) -> Option<&'static str> {
    let lowered = file_name(browser).to_ascii_lowercase();
    let name = lowered.strip_suffix(".exe").unwrap_or(&lowered);
    match name {
        "chrome" | "msedge" | "brave" | "opera" | "vivaldi" => Some("--new-window"),
        "firefox" => Some("-new-window"),
        _ => None,
    }
}

fn open_page(url: &str, browser: &str, new_window: bool) -> Result<(), String> {
    let request = page_request(url, browser, new_window);
    shell_execute(
        &request.file,
        &request.parameters,
        "",
        "open",
        windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0,
    )
    .map_err(|error| format!("no se pudo abrir la página: {error}"))
}

fn change_volume(op: &VolumeOp) -> Result<(), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };

    unsafe {
        let started = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| {
            let enumerator = CoCreateInstance::<_, IMMDeviceEnumerator>(
                &MMDeviceEnumerator,
                None::<&windows::core::IUnknown>,
                CLSCTX_ALL,
            )
            .map_err(|error| format!("no se pudo abrir el audio: {error}"))?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|error| format!("no hay un dispositivo de reproducción: {error}"))?;
            let volume = device
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                .map_err(|error| format!("no se pudo leer el volumen: {error}"))?;
            apply_volume(&volume, op)
        })();
        if started.is_ok() {
            CoUninitialize();
        }
        result
    }
}

fn apply_volume(volume: &windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume, op: &VolumeOp) -> Result<(), String> {
    if matches!(op, VolumeOp::ToggleMute) {
        let muted = unsafe { volume.GetMute() }
            .map_err(|error| format!("no se pudo leer el silencio: {error}"))?
            .as_bool();
        return unsafe { volume.SetMute(!muted, std::ptr::null()) }
            .map_err(|error| format!("no se pudo cambiar el silencio: {error}"));
    }
    let current = unsafe { volume.GetMasterVolumeLevelScalar() }
        .map_err(|error| format!("no se pudo leer el volumen: {error}"))?;
    let percent = (current.clamp(0.0, 1.0) * 100.0).round() as u32;
    let Some(next) = adjusted_volume(percent, op) else {
        return Ok(());
    };
    let level = (next as f32) / 100.0;
    unsafe { volume.SetMasterVolumeLevelScalar(level, std::ptr::null()) }
        .map_err(|error| format!("no se pudo cambiar el volumen: {error}"))
}

fn arrange_window(op: &WindowOp) -> Result<(), String> {
    match op {
        WindowOp::MinimizeAll => {
            unsafe {
                let _ = EnumWindows(Some(minimize_visible), LPARAM(0));
            }
            Ok(())
        }
        WindowOp::Screensaver => {
            unsafe {
                let _ = SendMessageW(
                    GetDesktopWindow(),
                    WM_SYSCOMMAND,
                    Some(WPARAM(SC_SCREENSAVE as usize)),
                    Some(LPARAM(0)),
                );
            }
            Ok(())
        }
        WindowOp::EmptyRecycleBin => unsafe {
            SHEmptyRecycleBinW(None, windows::core::PCWSTR::null(), 0)
                .map_err(|error| format!("no se pudo vaciar la papelera: {error}"))
        },
        WindowOp::Activate { title } => activate_title(title),
        WindowOp::MaximizeMonitor { monitor } => {
            let hwnd = foreground()?;
            let (left, top, width, height) = monitor_box(*monitor)?;
            place(hwnd, left, top, width, height, true)
        }
        WindowOp::Move {
            left,
            top,
            right,
            bottom,
        } => {
            let hwnd = foreground()?;
            let (left, top, width, height) = window_box(*left, *top, *right, *bottom)?;
            place(hwnd, left, top, width, height, false)
        }
        other => {
            let hwnd = foreground()?;
            match other {
                WindowOp::Close => unsafe {
                    PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))
                        .map_err(|error| format!("no se pudo cerrar la ventana: {error}"))
                },
                WindowOp::Minimize => show(hwnd, SW_MINIMIZE),
                WindowOp::Maximize => show(hwnd, SW_MAXIMIZE),
                WindowOp::Restore => show(hwnd, SW_RESTORE),
                WindowOp::SendBack => unsafe {
                    SetWindowPos(
                        hwnd,
                        Some(HWND_BOTTOM),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    )
                    .map_err(|error| format!("no se pudo enviar la ventana atrás: {error}"))
                },
                WindowOp::ToggleTopmost => toggle_topmost(hwnd),
                WindowOp::EndProcess => end_process(hwnd),
                WindowOp::MinimizeAll
                | WindowOp::Screensaver
                | WindowOp::EmptyRecycleBin
                | WindowOp::Activate { .. }
                | WindowOp::MaximizeMonitor { .. }
                | WindowOp::Move { .. } => Ok(()),
            }
        }
    }
}

fn foreground() -> Result<HWND, String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        Err("no hay una ventana activa".into())
    } else {
        Ok(hwnd)
    }
}

fn show(hwnd: HWND, command: windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD) -> Result<(), String> {
    unsafe {
        let _ = ShowWindow(hwnd, command);
    }
    Ok(())
}

fn place(hwnd: HWND, left: i32, top: i32, width: i32, height: i32, maximize: bool) -> Result<(), String> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(hwnd, Some(HWND_TOP), left, top, width, height, SWP_SHOWWINDOW)
            .map_err(|error| format!("no se pudo mover la ventana: {error}"))?;
        if maximize {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
    }
    Ok(())
}

fn toggle_topmost(hwnd: HWND) -> Result<(), String> {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let topmost = style & WS_EX_TOPMOST.0 as isize != 0;
    let after = if topmost { HWND_NOTOPMOST } else { HWND_TOPMOST };
    unsafe {
        SetWindowPos(hwnd, Some(after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)
            .map_err(|error| format!("no se pudo cambiar siempre visible: {error}"))
    }
}

fn end_process(hwnd: HWND) -> Result<(), String> {
    let mut pid = 0u32;
    unsafe {
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
        return Err("no se cierra el proceso de este programa".into());
    }
    let process = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }
        .map_err(|error| format!("no se pudo abrir el proceso: {error}"))?;
    let ended = unsafe { TerminateProcess(process, 1) };
    let _ = unsafe { CloseHandle(process) };
    ended.map_err(|error| format!("no se pudo cerrar el proceso: {error}"))
}

fn monitor_box(monitor: u32) -> Result<(i32, i32, i32, i32), String> {
    let mut rects = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut rects as *mut Vec<(i32, i32, i32, i32)> as isize),
        );
    }
    rects
        .get(usize::try_from(monitor).unwrap_or(0).saturating_sub(1))
        .copied()
        .ok_or_else(|| "ese monitor no está conectado".into())
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut windows::Win32::Foundation::RECT,
    data: LPARAM,
) -> windows::core::BOOL {
    let list = unsafe { &mut *(data.0 as *mut Vec<(i32, i32, i32, i32)>) };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        let rect = info.rcMonitor;
        list.push((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top));
    }
    TRUE
}

fn activate_title(title: &str) -> Result<(), String> {
    let mut search = TitleSearch {
        title: title.trim().to_lowercase(),
        hwnd: HWND::default(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(find_title),
            LPARAM(&mut search as *mut TitleSearch as isize),
        );
    }
    if search.hwnd.0.is_null() {
        return Err("no hay una ventana con ese título".into());
    }
    unsafe {
        let _ = ShowWindow(search.hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(search.hwnd);
    }
    Ok(())
}

struct TitleSearch {
    title: String,
    hwnd: HWND,
}

unsafe extern "system" fn find_title(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let search = unsafe { &mut *(lparam.0 as *mut TitleSearch) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return TRUE;
    }
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if length > 0 {
        let title = String::from_utf16_lossy(&buffer[..length as usize]).to_lowercase();
        if title.contains(&search.title) {
            search.hwnd = hwnd;
            return FALSE;
        }
    }
    TRUE
}

unsafe extern "system" fn minimize_visible(hwnd: HWND, _lparam: LPARAM) -> windows::core::BOOL {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return TRUE;
    }
    let mut class = [0u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut class) };
    if length > 0 {
        let name = String::from_utf16_lossy(&class[..length as usize]);
        if name == "Shell_TrayWnd" || name == "Progman" || name == "WorkerW" {
            return TRUE;
        }
    }
    let mut title = [0u16; 2];
    if unsafe { GetWindowTextW(hwnd, &mut title) } == 0 {
        return TRUE;
    }
    let _ = unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
    TRUE
}

struct OpenRequest {
    file: String,
    parameters: String,
}

fn open_file_request(path: &str, program: &str, arguments: &str) -> OpenRequest {
    let path = path.trim();
    let program = program.trim();
    let arguments = arguments.trim();
    if program.is_empty() {
        return OpenRequest {
            file: path.to_string(),
            parameters: arguments.to_string(),
        };
    }
    let mut parameters = quote_arg(path);
    if !arguments.is_empty() {
        parameters.push(' ');
        parameters.push_str(arguments);
    }
    OpenRequest {
        file: program.to_string(),
        parameters,
    }
}

fn quote_arg(value: &str) -> String {
    if value.contains([' ', '\t']) {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

struct LaunchRequest {
    file: String,
    parameters: String,
    directory: String,
    verb: &'static str,
    minimized: bool,
}

fn launch_request(
    program: &str,
    arguments: &str,
    directory: &str,
    minimized: bool,
    as_admin: bool,
) -> LaunchRequest {
    LaunchRequest {
        file: program.trim().to_string(),
        parameters: arguments.trim().to_string(),
        directory: directory.trim().to_string(),
        verb: if as_admin { "runas" } else { "open" },
        minimized,
    }
}

fn same_program(image: &str, program: &str) -> bool {
    let image = image.trim();
    let program = program.trim();
    if image.is_empty() || program.is_empty() {
        return false;
    }
    if image.eq_ignore_ascii_case(program) {
        return true;
    }
    file_name(image).eq_ignore_ascii_case(file_name(program))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn launch_program(
    program: &str,
    arguments: &str,
    directory: &str,
    minimized: bool,
    activate_existing: bool,
    as_admin: bool,
) -> Result<(), String> {
    if activate_existing && activate_running(program) {
        return Ok(());
    }
    let request = launch_request(program, arguments, directory, minimized, as_admin);
    let show = if request.minimized {
        SW_SHOWMINNOACTIVE.0
    } else {
        windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0
    };
    shell_execute(
        &request.file,
        &request.parameters,
        &request.directory,
        request.verb,
        show,
    )
    .map_err(|error| format!("no se pudo iniciar el programa: {error}"))
}

fn open_path(path: &str, program: &str, arguments: &str) -> Result<(), String> {
    let request = open_file_request(path, program, arguments);
    shell_execute(
        &request.file,
        &request.parameters,
        "",
        "open",
        windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0,
    )
    .map_err(|error| format!("no se pudo abrir el archivo: {error}"))
}

fn shell_execute(
    file: &str,
    parameters: &str,
    directory: &str,
    verb: &str,
    show: i32,
) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};

    let file = wide(file);
    let parameters = wide(parameters);
    let verb = wide(verb);
    let directory_wide = if directory.is_empty() {
        None
    } else {
        Some(wide(directory))
    };
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: directory_wide
            .as_ref()
            .map(|text| PCWSTR(text.as_ptr()))
            .unwrap_or(PCWSTR::null()),
        nShow: show,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|error| error.to_string())
}

struct WindowSearch {
    program: String,
    hwnd: HWND,
}

fn activate_running(program: &str) -> bool {
    let mut search = WindowSearch {
        program: program.trim().to_string(),
        hwnd: HWND::default(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(find_program_window),
            LPARAM(&mut search as *mut WindowSearch as isize),
        );
    }
    if search.hwnd.0.is_null() {
        return false;
    }
    unsafe {
        let show = if IsIconic(search.hwnd).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(search.hwnd, show);
        let _ = SetForegroundWindow(search.hwnd);
    }
    true
}

unsafe extern "system" fn find_program_window(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let search = &mut *(lparam.0 as *mut WindowSearch);
    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let mut pid = 0u32;
    if GetWindowThreadProcessId(hwnd, Some(&mut pid)) == 0 || pid == 0 {
        return TRUE;
    }
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return TRUE;
    };
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    if queried.is_ok() {
        let image = String::from_utf16_lossy(&buffer[..size as usize]);
        if same_program(&image, &search.program) {
            search.hwnd = hwnd;
            return FALSE;
        }
    }
    TRUE
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn play(events: &[InputEvent], speed: u32, repeat: &Repeat) -> Result<(), String> {
    let limit = match repeat {
        Repeat::Times { count } => Some((*count).clamp(1, 100)),
        Repeat::Forever => None,
    };
    let mut done = 0u32;
    loop {
        for event in events {
            if escape_is_down() {
                return Ok(());
            }
            match event {
                InputEvent::Wait { millis } => sleep_interruptible(wait_millis(*millis, speed)),
                InputEvent::Key { key, down } => send_key(key, *down)?,
                InputEvent::Button { button, down } => send_button(*button, *down)?,
                InputEvent::Move { x, y } => send_move(*x, *y)?,
            }
        }
        done += 1;
        if limit.is_some_and(|limit| done >= limit) || escape_is_down() {
            return Ok(());
        }
    }
}

fn sleep_interruptible(millis: u64) {
    let mut left = millis;
    while left > 0 {
        if escape_is_down() {
            return;
        }
        let slice = left.min(50);
        thread::sleep(Duration::from_millis(slice));
        left -= slice;
    }
}

fn escape_is_down() -> bool {
    key_is_down(VK_ESCAPE)
}

fn send_key(key: &str, down: bool) -> Result<(), String> {
    let key = parse_key(key)?;
    send_inputs(&[key_input(VIRTUAL_KEY(virtual_key(key)), !down)])
}

fn send_button(button: MouseButton, down: bool) -> Result<(), String> {
    let flags = match (button, down) {
        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
    };
    send_inputs(&[mouse_input(0, 0, flags)])
}

fn send_move(x: i32, y: i32) -> Result<(), String> {
    let (dx, dy) = absolute_point(x, y);
    send_inputs(&[mouse_input(dx, dy, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)])
}

fn absolute_point(x: i32, y: i32) -> (i32, i32) {
    let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1);
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1);
    let dx = (i64::from(x - origin_x) * 65535 / i64::from(width - 1).max(1)) as i32;
    let dy = (i64::from(y - origin_y) * 65535 / i64::from(height - 1).max(1)) as i32;
    (dx.clamp(0, 65535), dy.clamp(0, 65535))
}

fn mouse_input(dx: i32, dy: i32, flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(format!("SendInput envió {sent} de {} eventos", inputs.len()));
    }
    Ok(())
}

struct RecordState {
    events: Vec<InputEvent>,
    last: Instant,
    last_move: Option<(i32, i32)>,
}

static RECORDING: Mutex<Option<RecordState>> = Mutex::new(None);
static CAPTURE_ALIVE: AtomicBool = AtomicBool::new(false);

pub struct Capture {
    stop: Arc<AtomicBool>,
    done: mpsc::Receiver<Result<Vec<InputEvent>, String>>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn poll(&self) -> Option<Result<Vec<InputEvent>, String>> {
        match self.done.try_recv() {
            Ok(result) => Some(result),
            Err(_) => None,
        }
    }
}

pub fn captured_event_count() -> usize {
    RECORDING
        .lock()
        .expect("grabación")
        .as_ref()
        .map(|state| state.events.len())
        .unwrap_or(0)
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start_capture() -> Result<Capture, String> {
    if CAPTURE_ALIVE.swap(true, Ordering::SeqCst) {
        return Err("ya hay una grabación en curso".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, done) = mpsc::channel();
    let stop_for_thread = Arc::clone(&stop);
    let thread = thread::spawn(move || {
        let result = record_loop(stop_for_thread);
        let _ = sender.send(result);
        CAPTURE_ALIVE.store(false, Ordering::SeqCst);
    });
    Ok(Capture {
        stop,
        done,
        thread: Some(thread),
    })
}

fn record_loop(stop: Arc<AtomicBool>) -> Result<Vec<InputEvent>, String> {
    *HOOK_STOP.lock().expect("parada") = Some(Arc::clone(&stop));
    *RECORDING.lock().expect("grabación") = Some(RecordState {
        events: Vec::new(),
        last: Instant::now(),
        last_move: None,
    });
    let keyboard = unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0)
            .map_err(|error| format!("no se pudo enganchar el teclado: {error}"))
    };
    let keyboard = match keyboard {
        Ok(hook) => hook,
        Err(error) => {
            *RECORDING.lock().expect("grabación") = None;
            *HOOK_STOP.lock().expect("parada") = None;
            return Err(error);
        }
    };
    let mouse = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0)
            .map_err(|error| format!("no se pudo enganchar el ratón: {error}"))
    };
    let mouse = match mouse {
        Ok(hook) => hook,
        Err(error) => {
            unsafe {
                let _ = UnhookWindowsHookEx(keyboard);
            }
            *RECORDING.lock().expect("grabación") = None;
            *HOOK_STOP.lock().expect("parada") = None;
            return Err(error);
        }
    };
    let mut message = MSG::default();
    while !stop.load(Ordering::Relaxed) {
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    unsafe {
        let _ = UnhookWindowsHookEx(keyboard);
        let _ = UnhookWindowsHookEx(mouse);
    }
    let events = RECORDING
        .lock()
        .expect("grabación")
        .take()
        .map(|state| state.events)
        .unwrap_or_default();
    *HOOK_STOP.lock().expect("parada") = None;
    Ok(events)
}

unsafe extern "system" fn keyboard_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if code == 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let up = wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize;
        if info.vkCode == u32::from(VK_ESCAPE.0) {
            if up {
                request_stop();
            }
        } else if let Some(key) = key_from_virtual_code(info.vkCode) {
            push_event(InputEvent::Key {
                key: key.label().to_string(),
                down: !up,
            });
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if code == 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let message = wparam.0 as u32;
        match message {
            value if value == WM_MOUSEMOVE => note_move(info.pt.x, info.pt.y),
            value if value == WM_LBUTTONDOWN => push_button(MouseButton::Left, true),
            value if value == WM_LBUTTONUP => push_button(MouseButton::Left, false),
            value if value == WM_RBUTTONDOWN => push_button(MouseButton::Right, true),
            value if value == WM_RBUTTONUP => push_button(MouseButton::Right, false),
            value if value == WM_MBUTTONDOWN => push_button(MouseButton::Middle, true),
            value if value == WM_MBUTTONUP => push_button(MouseButton::Middle, false),
            _ => {}
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn request_stop() {
    if let Some(handle) = HOOK_STOP.lock().expect("parada").as_ref() {
        handle.store(true, Ordering::Relaxed);
    }
}

static HOOK_STOP: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

fn note_move(x: i32, y: i32) {
    let mut recording = RECORDING.lock().expect("grabación");
    let Some(state) = recording.as_mut() else {
        return;
    };
    if let Some((previous_x, previous_y)) = state.last_move {
        let far = (x - previous_x).abs() + (y - previous_y).abs() >= 12;
        let aged = state.last.elapsed() >= Duration::from_millis(40);
        if !far && !aged {
            return;
        }
    }
    push_event_locked(state, InputEvent::Move { x, y });
    state.last_move = Some((x, y));
}

fn push_button(button: MouseButton, down: bool) {
    push_event(InputEvent::Button { button, down });
}

fn push_event(event: InputEvent) {
    let mut recording = RECORDING.lock().expect("grabación");
    if let Some(state) = recording.as_mut() {
        push_event_locked(state, event);
    }
}

fn push_event_locked(state: &mut RecordState, event: InputEvent) {
    if state.events.len() >= 5_000 {
        request_stop();
        return;
    }
    let elapsed = state.last.elapsed().as_millis();
    let with_wait = (20..=60_000).contains(&elapsed);
    if with_wait && state.events.len() + 2 > 5_000 {
        request_stop();
        return;
    }
    if with_wait {
        state.events.push(InputEvent::Wait {
            millis: u32::try_from(elapsed).unwrap_or(60_000),
        });
    }
    state.last = Instant::now();
    state.events.push(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_ctrl_is_not_released_by_the_copy() {
        let steps = ctrl_c_steps(true);
        assert_eq!(steps, vec![(VK_C, false), (VK_C, true)]);
    }

    #[test]
    fn held_ctrl_is_not_released_by_the_paste() {
        let steps = ctrl_v_steps(true);
        assert!(steps.iter().all(|(key, _)| *key == VK_V));
        assert_eq!(steps, vec![(VK_V, false), (VK_V, true)]);
    }

    #[test]
    fn default_association_opens_the_path_itself() {
        let request = open_file_request(r"C:\notas\hoy.txt", "", "");
        assert_eq!(request.file, r"C:\notas\hoy.txt");
        assert!(request.parameters.is_empty());
    }

    #[test]
    fn a_chosen_program_receives_the_quoted_path() {
        let request = open_file_request(
            r"C:\notas\hoy y manana.txt",
            r"C:\Windows\notepad.exe",
            "/a",
        );
        assert_eq!(request.file, r"C:\Windows\notepad.exe");
        assert_eq!(request.parameters, r#""C:\notas\hoy y manana.txt" /a"#);
        let plain = open_file_request(r"C:\notas\hoy.txt", r"C:\Windows\notepad.exe", "");
        assert_eq!(plain.parameters, r"C:\notas\hoy.txt");
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

    #[test]
    fn an_admin_launch_uses_the_runas_verb() {
        let request = launch_request(r"C:\Windows\notepad.exe", "hoy.txt", r"C:\notas", true, true);
        assert_eq!(request.file, r"C:\Windows\notepad.exe");
        assert_eq!(request.parameters, "hoy.txt");
        assert_eq!(request.directory, r"C:\notas");
        assert_eq!(request.verb, "runas");
        assert!(request.minimized);
    }

    #[test]
    fn the_same_executable_name_counts_as_already_running() {
        assert!(same_program(
            r"C:\Windows\System32\notepad.exe",
            r"C:\Windows\notepad.exe"
        ));
        assert!(!same_program(
            r"C:\Windows\System32\notepad.exe",
            r"C:\Windows\System32\calc.exe"
        ));
    }

    #[test]
    fn a_folder_opens_with_the_explore_verb() {
        let request = folder_request(r"  C:\notas  ");
        assert_eq!(request.path, r"C:\notas");
        assert_eq!(request.verb, "explore");
    }

    #[test]
    fn the_default_browser_receives_the_address() {
        let request = page_request("https://example.com", "", true);
        assert_eq!(request.file, "https://example.com");
        assert!(request.parameters.is_empty());
    }

    #[test]
    fn chrome_can_be_asked_for_a_new_window() {
        let request = page_request(
            "https://example.com/a b",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            true,
        );
        assert!(request.file.ends_with("chrome.exe"));
        assert_eq!(
            request.parameters,
            "--new-window \"https://example.com/a b\""
        );
        let firefox = page_request("https://example.com", r"C:\Mozilla Firefox\firefox.exe", true);
        assert_eq!(firefox.parameters, "-new-window https://example.com");
        let other = page_request("https://example.com", r"C:\Apps\browser.exe", true);
        assert_eq!(other.parameters, "https://example.com");
    }
}
