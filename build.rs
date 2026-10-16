use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=branding/ico/keyboard-macros-os-dark.ico");
    println!("cargo:rerun-if-env-changed=KMOS_FILE_VERSION");
    if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows") {
        return;
    }
    if let Err(error) = embed_icon() {
        panic!("no se pudo incrustar el icono en el ejecutable: {error}");
    }
}

fn embed_icon() -> io::Result<()> {
    let stage = stage_dir()?;
    fs::create_dir_all(&stage)?;
    fs::copy(
        "branding/ico/keyboard-macros-os-dark.ico",
        stage.join("app.ico"),
    )?;
    fs::write(stage.join("app.rc"), resource_script())?;

    let toolchain = env::var("CARGO_CFG_TARGET_ENV").map_err(|error| io::Error::other(error))?;
    match toolchain.as_str() {
        "gnu" => compile_gnu(&stage),
        "msvc" => compile_msvc(&stage),
        other => Err(io::Error::other(format!(
            "el icono solo se incrusta con el host GNU o MSVC, no con {other}"
        ))),
    }
}

fn stage_dir() -> io::Result<PathBuf> {
    let out = env::var("OUT_DIR").map_err(|error| io::Error::other(error))?;
    let mut hash = 0u32;
    for byte in out.bytes() {
        hash = hash.wrapping_mul(167).wrapping_add(u32::from(byte));
    }
    Ok(env::temp_dir().join(format!("kmos-exe-icon-{hash:08x}")))
}

fn file_version() -> String {
    if let Ok(version) = env::var("KMOS_FILE_VERSION") {
        let version = version.trim().trim_start_matches('v');
        if version.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        {
            return version.to_string();
        }
    }
    env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into())
}

fn resource_script() -> String {
    let version = file_version();
    let commas = version_commas(&version);
    format!(
        r#"1 ICON "app.ico"

1 VERSIONINFO
FILEVERSION {commas}
PRODUCTVERSION {commas}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "Yorlandiscuba"
            VALUE "FileDescription", "Keyboard Macros OS"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "kmos"
            VALUE "LegalCopyright", "Copyright (c) 2026 Yorlandiscuba"
            VALUE "OriginalFilename", "kmos.exe"
            VALUE "ProductName", "Keyboard Macros OS"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    )
}

fn version_commas(version: &str) -> String {
    let mut parts: Vec<&str> = version.split('.').collect();
    while parts.len() < 4 {
        parts.push("0");
    }
    parts.truncate(4);
    parts.join(",")
}

fn compile_gnu(stage: &Path) -> io::Result<()> {
    let windres = env::var("WINDRES").unwrap_or_else(|_| "windres".into());
    let arch = env::var("CARGO_CFG_TARGET_ARCH").map_err(|error| io::Error::other(error))?;
    let target = match arch.as_str() {
        "x86_64" => "pe-x86-64",
        "x86" => "pe-i386",
        other => {
            return Err(io::Error::other(format!(
                "arquitectura {other} sin icono en el ejecutable"
            )))
        }
    };
    let status = Command::new(windres)
        .current_dir(stage)
        .args(["--target", target, "-i", "app.rc", "-o", "app.o"])
        .status()?;
    if !status.success() {
        return Err(io::Error::other("windres no pudo compilar el icono"));
    }
    println!("cargo:rustc-link-arg={}", stage.join("app.o").display());
    Ok(())
}

fn compile_msvc(stage: &Path) -> io::Result<()> {
    let rc = find_rc()?;
    let output = stage.join("app.res");
    let status = Command::new(rc)
        .current_dir(stage)
        .arg(format!("/fo{}", output.display()))
        .arg("app.rc")
        .status()?;
    if !status.success() {
        return Err(io::Error::other("rc.exe no pudo compilar el icono"));
    }
    println!("cargo:rustc-link-arg={}", output.display());
    Ok(())
}

fn find_rc() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os("RC_PATH") {
        return Ok(PathBuf::from(path));
    }
    for value in ["KitsRoot10", "KitsRoot81", "KitsRoot"] {
        if let Some(root) = query_kit_root(value)? {
            if let Some(rc) = newest_rc(&root) {
                return Ok(rc);
            }
        }
    }
    Err(io::Error::other(
        "no está rc.exe. Hace falta el Windows SDK para incrustar el icono",
    ))
}

fn query_kit_root(value: &str) -> io::Result<Option<PathBuf>> {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots",
            "/v",
            value,
            "/reg:32",
        ])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().find_map(|line| {
        let (name, root) = line.split_once("REG_SZ")?;
        (name.trim() == value).then(|| PathBuf::from(root.trim()))
    }))
}

fn newest_rc(root: &Path) -> Option<PathBuf> {
    let bin = root.join("bin");
    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&bin) {
        for entry in entries.filter_map(|entry| entry.ok()) {
            let name = entry.file_name();
            let Some(text) = name.to_str() else {
                continue;
            };
            if let Some(key) = version_key(text) {
                versions.push((key, bin.join(name)));
            }
        }
    }
    versions.sort_by(|left, right| right.0.cmp(&left.0));
    let mut dirs: Vec<PathBuf> = versions.into_iter().map(|(_, path)| path).collect();
    dirs.push(bin);
    let archs = ["x64", "x86"];
    dirs.into_iter().find_map(|dir| {
        archs.iter().find_map(|arch| {
            let rc = dir.join(arch).join("rc.exe");
            rc.exists().then_some(rc)
        })
    })
}

fn version_key(name: &str) -> Option<[u32; 4]> {
    let mut parts = [0u32; 4];
    let mut fields = name.split('.');
    for part in &mut parts {
        *part = fields.next()?.parse().ok()?;
    }
    fields.next().is_none().then_some(parts)
}
