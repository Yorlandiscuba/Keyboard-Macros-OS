# Keyboard Macros OS

Atajos globales que insertan un texto en la ventana que esté delante. Esta primera versión corre en Windows. El archivo de macros es propio: no usa el formato de otros programas.

Licencia: [MIT](LICENSE).

## Entorno

Hace falta [rustup](https://rustup.rs/) y el ensamblador de MinGW. En esta máquina no están las herramientas de Visual Studio, así que el host de Rust es el de GNU. El `dlltool` que trae Rust genera bibliotecas de importación y, para eso, llama a `as.exe`. Ese programa no viene con el host: hace falta un MinGW-w64 completo, por ejemplo [WinLibs](https://winlibs.com/) (POSIX, UCRT).

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e
```

`cargo` tiene que ver `as.exe`. En esta máquina está en `E:\tools\winlibs\mingw64\bin`. Cierra y abre la terminal si acabas de cambiar el `PATH`.

## Compilar

Desde esta carpeta:

```powershell
cargo test
cargo build --release
```

El ejecutable queda en `target\release\kmos.exe`.

## Probar el archivo sin instalar atajos

```powershell
cargo run -- --check macros.example.json
```

`--check` solo lee el archivo y muestra los atajos. No se queda escuchando.

## Insertar texto

Al arrancar se abre la ventana: la lista a la izquierda y el texto a la derecha. Nueva crea una macro, Borrar la quita y Guardar escribe el archivo y vuelve a registrar los atajos.

```powershell
Copy-Item macros.example.json macros.json
cargo run --release
```

Deja el Bloc de notas enfocado y pulsa `Ctrl+Shift+H`. El programa pega `Hola desde Keyboard Macros OS` y restaura el portapapeles. Cerrar la ventana la deja en la bandeja: el atajo sigue activo. En el icono, Abrir muestra la ventana y Salir termina el programa.

`macros.json` no se versiona: ahí van los textos propios.
