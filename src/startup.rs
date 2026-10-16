use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "Keyboard Macros OS";

pub fn is_enabled() -> Result<bool, String> {
    Ok(read_sz(RUN_SUBKEY, VALUE_NAME)?.is_some())
}

pub fn set_enabled(enabled: bool, macros: &Path) -> Result<(), String> {
    if !enabled {
        return delete_value(RUN_SUBKEY, VALUE_NAME);
    }
    let exe = std::env::current_exe()
        .map_err(|error| format!("no se encontró el ejecutable: {error}"))?;
    let file = absolute(macros);
    for path in [&exe, &file] {
        if path.display().to_string().contains('"') {
            return Err("la ruta contiene comillas y no se puede registrar".into());
        }
    }
    let command = format!("\"{}\" \"{}\"", exe.display(), file.display());
    write_sz(RUN_SUBKEY, VALUE_NAME, &command)
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

fn read_sz(subkey: &str, name: &str) -> Result<Option<String>, String> {
    let subkey = wide(subkey);
    let name = wide(name);
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND || size == 0 {
        return Ok(None);
    }
    if !status.is_ok() && status != ERROR_MORE_DATA {
        return Err(registry_error(status));
    }
    let mut buffer = vec![0u16; (size as usize / 2).max(1)];
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if !status.is_ok() {
        return Err(registry_error(status));
    }
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    String::from_utf16(&buffer[..end]).map(Some).map_err(|_| {
        "el valor de arranque no es texto válido".to_string()
    })
}

fn write_sz(subkey: &str, name: &str, value: &str) -> Result<(), String> {
    let subkey = wide(subkey);
    let name = wide(name);
    let data = wide(value);
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    };
    status.ok().map_err(|error| format!("no se pudo guardar el arranque: {error}"))
}

fn delete_value(subkey: &str, name: &str) -> Result<(), String> {
    let subkey = wide(subkey);
    let name = wide(name);
    let status = unsafe {
        RegDeleteKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
        )
    };
    if status.is_ok() || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!("no se pudo quitar el arranque: {}", registry_error(status)))
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn registry_error(status: WIN32_ERROR) -> String {
    status.ok().unwrap_err().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_uses_a_private_key() {
        let subkey = "Software\\keyboard-macros-os\\test";
        let name = "roundtrip";
        delete_value(subkey, name).unwrap();
        assert!(read_sz(subkey, name).unwrap().is_none());
        write_sz(subkey, name, "kmos-test").unwrap();
        assert_eq!(read_sz(subkey, name).unwrap().as_deref(), Some("kmos-test"));
        delete_value(subkey, name).unwrap();
        assert!(read_sz(subkey, name).unwrap().is_none());
    }
}
