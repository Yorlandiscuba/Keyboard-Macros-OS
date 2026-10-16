#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod brand;
mod engine;
mod import_hk;
mod model;
mod store;
mod tray;
mod ui;

#[cfg(windows)]
mod startup;
#[cfg(windows)]
mod windows_host;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use engine::Engine;
use model::{Document, SCHEMA_VERSION};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kmos: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut check_only = false;
    let mut file: Option<PathBuf> = None;
    let mut import_from: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check_only = true,
            "--import" => {
                let path = args
                    .next()
                    .ok_or_else(|| "falta la ruta después de --import".to_string())?;
                import_from = Some(PathBuf::from(path));
            }
            "--file" => {
                let path = args
                    .next()
                    .ok_or_else(|| "falta la ruta después de --file".to_string())?;
                file = Some(PathBuf::from(path));
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("opción desconocida: {other}"));
            }
            other => file = Some(PathBuf::from(other)),
        }
    }
    let path = file.unwrap_or_else(|| PathBuf::from("macros.json"));
    if check_only && import_from.is_none() {
        let document = store::load(&path)?;
        let engine = Engine::from_document(&document)?;
        print_summary(&path, &engine);
        return Ok(());
    }
    let (mut document, mut notice) = if path.exists() {
        (store::load(&path)?, String::new())
    } else {
        (
            Document {
                version: SCHEMA_VERSION,
                macros: Vec::new(),
            },
            format!("Se creará {} al guardar.", path.display()),
        )
    };
    if let Some(source) = import_from {
        let xml = fs::read_to_string(&source)
            .map_err(|error| format!("no se pudo leer {}: {error}", source.display()))?;
        let report = import_hk::import_xml(&xml, &mut document)?;
        store::save(&path, &document)?;
        notice = report.summary();
    }
    let engine = Engine::from_document(&document)?;
    print_summary(&path, &engine);
    if !notice.is_empty() {
        println!("{notice}");
    }
    if check_only {
        return Ok(());
    }
    ui::run(path, document, notice)
}

fn print_summary(path: &std::path::Path, engine: &Engine) {
    println!("kmos leyó {} ({})", path.display(), describe(engine));
    for (index, chord, macro_) in engine.bindings() {
        println!(
            "  [{index}] {}  {}  →  {}",
            macro_.id,
            chord.label(),
            macro_.name
        );
    }
}

fn describe(engine: &Engine) -> String {
    let count = engine.len();
    if count == 1 {
        "1 macro".into()
    } else {
        format!("{count} macros")
    }
}

fn print_help() {
    println!(
        "\
kmos — atajos que insertan texto

Uso:
  kmos [--file] macros.json
  kmos --check macros.json
  kmos --import HotKeyboardData.xml [--file] macros.json

Sin ruta, abre macros.json del directorio actual.
Si el archivo no existe, la ventana arranca vacía y lo crea al guardar.
Copia macros.example.json para empezar.
El atajo de ejemplo es ctrl+shift+h.
Deja el Bloc de notas enfocado y pulsa el atajo.
Cerrar la ventana la deja en la bandeja.
En el icono: Abrir vuelve a mostrar la ventana, Salir termina el programa.
--import lee un XML de Hot Keyboard y guarda menú, nombre, atajo y texto.
Las grabaciones se conservan y quedan marcadas como no reproducibles.
Iniciar con Windows está en la ventana y apunta a este ejecutable."
    );
}

