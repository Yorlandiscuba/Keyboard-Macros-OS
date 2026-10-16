# Inventario del artefacto

Producto observado: **Hot Keyboard Pro 6.5.0.112**, compañía **Imposant**.

La versión sale del recurso de versión de `HotKeyb.exe` y de `hkbres.dll`, y coincide con el XML de datos:

```xml
<Name>Hot Keyboard</Name>
<Version>
  <Major>6</Major>
  <Minor>5</Minor>
  <Build>112</Build>
</Version>
```

Sitio referido por cadenas del binario: `http://www.hot-keyboard.com`. Correo de soporte referido: `support@imposant.com`.

## Archivos

| Archivo | Tamaño | Formato | SHA-256 |
| --- | ---: | --- | --- |
| `HotKeyb.exe` | 3 206 304 | PE32, x86, subsistema GUI | `b0b2f0282beacc0208e88eee021ec97ca34dbb06ae9284f119a75e7e2370ba44` |
| `hkbres.dll` | 719 008 | PE32, x86, GUI, DLL de recursos | `97a9cb08707e204cca4adf06c95a1fb52f3686bbd3d7e7ee20f1274cf4e7c88f` |
| `hkhook45.dll` | 30 880 | PE32, x86, GUI | `5f1697671e8f031535ec5c76cd3f4196186c8341a805ddb5a43c5d4b44d221c5` |
| `HkHook64.exe` | 20 128 | PE32+, x64, consola | `85d43bd769437d87f39c13541268be6a5a4c78c33bc079a1b24c78f99354780c` |
| `HkHook64_45.dll` | 21 664 | PE32+, x64, GUI | `3554cd6e3799e690fc11ecf7cbc83da6c4f99f4a68fe0d7e79ecc4c058303538` |
| `Keyboards/KeyboardLayoutTool.exe` | 264 872 | PE32, x86, GUI, versión 1.0.0.1 | `27327e9085a7147eedf9f58c43909e4247cae72c841dc9988b7136742acf64e4` |
| `hotkeyb.chm` | 1 299 434 | ayuda compilada | — |
| `hotkeyb.hhc` | 7 510 | índice HTML de la ayuda | — |
| `hk.ini` | 101 | rutas de macros y ajustes | — |
| `HotKeyboardData.xml` | 10 996 | estado y macros locales | — |
| `HKBackup00.hk4backup` … `14` | 3 051–10 996 | mismas copias XML rotativas | — |
| `Keyboards/*.txt` | 24 archivos | mapas de teclas extendidas | — |
| `Screenshots/` | vacía | — | — |

`TimeDateStamp` de la cabecera PE (no es la fecha de copia del archivo):

- `HotKeyb.exe` y `hkbres.dll`: 2020-01-18.
- `hkhook45.dll`, `HkHook64.exe` y `HkHook64_45.dll`: 2015-07-31.
- `KeyboardLayoutTool.exe`: 2007-02-07. Su copyright de versión dice 2004-2005.

## Qué es cada binario

`HotKeyb.exe` es la aplicación. Las cadenas RTTI (`.?AV…`) son las de Microsoft Visual C++. Importa la DLL de gancho de 32 bits por ordinales 1 a 11.

`hkbres.dll` no exporta funciones. Concentra la interfaz: 102 diálogos, 89 tablas de cadenas, 47 iconos, 11 menús y 4 `RCDATA`. Ahí viven los textos de los tipos de macro.

`hkhook45.dll` es el gancho de 32 bits. Exporta:

- `InstallHKHook`, `UninstallHKHook`
- `InstallKeyboardHook`, `UninstallKeyboardHook`
- `InstallMouseHook`, `UninstallMouseHook`
- `AddHotKey`, `RemoveHotKey`
- `ChangeHandlingFlags`
- `hkhook_layouts`, `hkhook_umlautKeys`

`HkHook64.exe` es un proceso de consola de 64 bits. Solo importa dos funciones de `HkHook64_45.dll`: `HKHook64_Hook` y `HKHook64_UnHook` (nombres C++ decorados `?HKHook64_Hook@@YAXXZ` y `?HKHook64_UnHook@@YAXXZ`).

`KeyboardLayoutTool.exe` es una utilidad aparte, «Keyboard Layout Tool for Hot Keyboard», para capturar teclas que el teclado no informa con un código virtual estándar y mandar la lista a los desarrolladores.

## Recursos de `HotKeyb.exe`

Evidencia REA `ev_bedeab101a7fb26b64c2c0e04a162130e5745de24824ed425462861924ffd0e0`. Inventario completo: 59 recursos, idioma 1033 (inglés de EE. UU.).

Tipos presentes: cursores, bitmaps, iconos (grupo de 48, 32 y 16 px a 32 bits), diálogos, tablas de cadenas, grupos de cursores, manifiesto y versión. El manifiesto pide Common Controls 6.

## Ayuda

`hotkeyb.hhc` es el mapa de la ayuda, no el texto de cada página. Los capítulos son:

- Primeros pasos: macros, demo, básicos, actualización en vivo, protección por contraseña.
- Tipos de acción.
- Métodos de activación y condiciones.
- Uso: ventana principal, opciones, herramienta de layout, edición, tarjeta de referencia, teclas globales, importar/exportar, organización, integración con el shell, línea de comandos, copias automáticas, `hk.ini`.
- Scripting: tipos, operadores, funciones, variables.
- Información: requisitos, soporte, registro, licencia.

El CHM no se descompiló. Los nombres de tema salen del índice; el texto largo de la ayuda sigue dentro del CHM.
