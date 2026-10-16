# Arquitectura de entrada en Windows

Hot Keyboard tiene que ver teclas dentro de otros programas y también inyectar teclas y clics. En Windows eso obliga a un gancho global, y un gancho global de 32 bits no entra en procesos de 64 bits. Por eso el paquete trae los dos lados.

## Piezas

```text
HotKeyb.exe (32 bits)
    |  importa hkhook45.dll por ordinal
    v
InstallHKHook
    |  SetWindowsHookEx(WH_GETMESSAGE) en el proceso de 32 bits
    |  si GetNativeSystemInfo dice arquitectura 9 (AMD64):
    v
HKHook64.exe /install-hook
    |
    v
HkHook64_45.dll
    SetWindowsHookEx en el proceso de 64 bits
```

Hecho observado:

- `HotKeyb.exe` importa `hkhook45.dll` y también importa, él mismo, `SetWindowsHookExA`, `UnhookWindowsHookEx` (aparece como cadena), `RegisterHotKey`, `UnregisterHotKey` (cadena), `keybd_event` y `mouse_event`.
- `hkhook45.dll` importa `SetWindowsHookExA`, `CallNextHookEx`, `UnhookWindowsHookEx`, `keybd_event`, `GetAsyncKeyState`, `GetKeyboardState`, `ToAsciiEx`, `GetKeyboardLayout` y `GetForegroundWindow`.
- `HkHook64_45.dll` importa `SetWindowsHookExA`, `CallNextHookEx`, `UnhookWindowsHookEx` y `PostMessageA`.
- `SendInput` no aparece en la tabla de imports de ninguno de estos binarios. La inyección observada está preparada con `keybd_event` y `mouse_event`.

Ghidra 12.1.4 descompiló el arranque. Quien crea el proceso de 64 bits es `InstallHKHook` dentro de `hkhook45.dll`, no `HotKeyb.exe`. El EXE solo menciona `HkHook64.exe` en la lista de archivos que copia al instalar la carpeta de trabajo.

## Qué hace `InstallHKHook`

Evidencia `ev_164c4e6b29e7d0a7a71bffb373c1e1b0eda107e92af473edcd56ac0af8120120`. Entrada `0x100014b1`.

1. Llama a `UninstallHKHook`.
2. Pone un gancho global `SetWindowsHookExA` con identificador 3. En la API de Windows ese número es `WH_GETMESSAGE`. El procedimiento es `FUN_10001242` y el hilo es 0, o sea, todos los hilos de los procesos donde Windows pueda inyectar esta DLL de 32 bits.
3. Decide el ayudante de 64 bits en `FUN_10001021` (evidencia `ev_f1c80e9e2cbb563a6f0526b5c6c556ebb295dc2a7d33047981f8be02b84e2b1e`). Carga `GetNativeSystemInfo` desde `Kernel32.dll` y mira el primer `short` de `SYSTEM_INFO`, que es `wProcessorArchitecture`. Si vale 9, sigue. 9 es `PROCESSOR_ARCHITECTURE_AMD64`. Si la función no existe o el valor no es 9, no crea el proceso.
4. Construye la ruta del ejecutable sustituyendo el nombre de la DLL por `HKHook64.exe` (con esa mayúscula).
5. Crea una tubería, hereda el extremo de lectura como entrada estándar del hijo y se queda el extremo de escritura.
6. `CreateProcessA` con la línea `/install-hook` y la bandera `0x8000000` (`CREATE_NO_WINDOW`). Guarda el `hProcess`.

En un Windows de 32 bits ese paso 3 no lanza nada. En este equipo el procesador es AMD64, así que el camino del ayudante está compilado para tomarse.

## Teclado y ratón

`InstallKeyboardHook` (evidencia `ev_50c337c8f37572690e2375a4e29bdf45409042676509ce4602ba319731b2e6af`) elige el tipo con el bit `0x400` de las banderas globales:

- Sin ese bit: identificador 2, `WH_KEYBOARD`, procedimiento `FUN_10001b18`, gancho global inyectado.
- Con ese bit: identificador `0x0d`, `WH_KEYBOARD_LL`. No inyecta la DLL. Crea un hilo (`FUN_10001c29`) y espera hasta 4 segundos a que el hilo señale un evento. El gancho de bajo nivel corre en ese hilo.

`InstallMouseHook` (evidencia `ev_ccb9a01dd014ac67955313bf7c209f30c9be0455e0b17c3593d3a3cef0817c89`) repite el esquema. Sin el bit `0x400` usa el identificador 7 (`WH_MOUSE`). Con el bit usa `0x0e` (`WH_MOUSE_LL`) y otro hilo.

`ChangeHandlingFlags` (ordinal 7, evidencia `ev_8cc3626e8c26a74fbba445c3b0b9b78c925dc48528a0218a012be32dde136f4c`) recibe dos máscaras: bits que se encienden y bits que se apagan. El resultado se escribe en el bloque compartido, desplazamiento `0x408`. El bit `0x80`, cuando pasa de apagado a encendido, pone a cero otra variable. `InstallKeyboardHook` apaga ese mismo bit al instalarse. El bit `0x400` es el que cambia el gancho clásico por el de bajo nivel. El resto de bits no tiene nombre todavía.

## Ordinales de `hkhook45.dll`

La base de exportación es 1. `HotKeyb.exe` importa los once.

| Ordinal | Nombre |
| --- | --- |
| 1 | `InstallHKHook` |
| 2 | `UninstallHKHook` |
| 3 | `InstallKeyboardHook` |
| 4 | `UninstallKeyboardHook` |
| 5 | `AddHotKey` |
| 6 | `RemoveHotKey` |
| 7 | `ChangeHandlingFlags` |
| 8 | `InstallMouseHook` |
| 9 | `UninstallMouseHook` |
| 10 | `hkhook_layouts` |
| 11 | `hkhook_umlautKeys` |

## Extensión del shell, aparte del gancho

`FUN_00478550` en `HotKeyb.exe` (`0x478550`) registra `HkShExt.dll` con `regsvr32 /s`. Después llama a `GetNativeSystemInfo` y, solo si la arquitectura es 9, repite el registro con `HkShExt64.dll`. Si el programa se considera en Windows Vista o posterior y no está elevado, la operación usa el verbo `runas`. Esto es la integración con el Explorador (`UseShellExtension`), no el gancho de teclas. En los datos de esta instalación esa opción está en 0.

## Memoria compartida

Estas cadenas aparecen en el EXE y en las DLL de gancho. Son nombres de objetos del núcleo, no una traza de ejecución:

- `HotKeyboard_FileMapping`
- `HotKeyboard_Semaphore`
- `HotKeyboard_KeyMonEvent`
- `HotKeyboard_MouseMonEvent`
- `HotKeyboard_WKEvent`
- `HotKeyboard_SimRecBufferMutex`
- `HotKeyboardUniqueAtom`
- `HotKeyboardFakeWnd`
- `HotKeyboard_PlaySeqDaemon_FakeWnd`

`hkhook45.dll` y `HkHook64_45.dll` importan `CreateFileMappingA`, `MapViewOfFile`, `CreateSemaphoreA` y `CreateEventA`. El canal entre el gancho y la aplicación es un file mapping más eventos y un semáforo.

En `InstallHKHook` el bloque compartido ya está creado cuando se escribe. Desplazamientos vistos:

- `0x408`: máscara de `ChangeHandlingFlags`.
- `0x40c`: el argumento que el EXE pasó a `InstallHKHook`.
- `0x410`: se pone a 0 al instalar.
- `0x414`: diez enteros, todos a `0xffffffff`.

Hay otro uso del mismo nombre de mapping, en el EXE (`FUN_00478a10`, `0x478a10`, evidencia `ev_f516c3a10e7c1082bc71635c322825795070c3ef76d597806c41656fe2e36e77`). Ahí el programa crea un mapping de página, escribe un byte de comando y varias cadenas terminadas en cero, y o bien avisa a la ventana `HotKeyboardFakeWnd` con el mensaje `0x8068`, o bien, si esa ventana no existe, guarda el bloque en `HKCU\SOFTWARE\TB Labs\Hot Keyboard` bajo el valor `SmartAdd pending info`. El comando `0xf9` crea además el evento `hkb_runmacrowait_%u` con el PID y espera a que lo señalen. El lector del otro lado es `FUN_0047f6f0`: lee el tamaño, copia el resto y publica el mensaje `0x8069`. Esto es el puente de «ejecutar macro / Smart Add», no el búfer de la grabación.

## Tres vías de captura

Conviven en los imports de `HotKeyb.exe`:

1. **Gancho de Windows** (`SetWindowsHookExA`), instalado desde las DLL de gancho. Sirve para ver teclado y ratón mientras otro programa está en primer plano. El layout tool también importa `SetWindowsHookExA`: esa utilidad escucha teclas para nombrarlas.
2. **Hotkeys del sistema** (`RegisterHotKey`). Atajos registrados en Windows, distintos del gancho de bajo nivel. La DLL de 32 bits exporta `AddHotKey` y `RemoveHotKey`.
3. **Raw Input e HID.** `RegisterRawInputDevices`, `GetRawInputData`, `GetRawInputDeviceList`, `GetRawInputDeviceInfoA`, y en `HID.DLL`: `HidP_GetCaps`, `HidP_GetButtonCaps`, `HidP_GetValueCaps`, `HidP_GetData`, `HidP_MaxDataListLength`. Hay una clase RTTI `CHidInput` con un método cuyo nombre decorado incluye `ConvertRawInputIntoHotKeys`. El ajuste `DisableHIDInput` del XML apaga esta vía.

La cadena `UseExtendedKeyFlag` y los archivos de `Keyboards/` cubren teclas multimedia y de fabricante que no caben en el teclado «normal».

## Reproducción

`MRecordMacro` guarda una secuencia y la vuelve a emitir. En el XML de esta instalación hay velocidad (`PlaybackSpeed mul/div`), posición de ratón (`MousePosition`) y repetición (`Repeat forever`).

La emisión de teclas y clics está ligada a `keybd_event` y `mouse_event`. Una cadena de la DLL de recursos dice que la grabación está pensada para interactuar con ventanas de otras aplicaciones, y que el ajuste de posición del ratón afecta a la reproducción, no a la grabación.

## Otras capacidades de sistema vistas en imports

Sirven para acciones que no son «pulsar una tecla»:

- Volumen: `WINMM.dll` (`mixerOpen`, `mixerGetLineControlsA`, `mixerSetControlDetails`, `PlaySoundA`, `mciSendCommandA`).
- Abrir archivos, carpetas y páginas: `SHELL32.dll` (`ShellExecuteExA`, `SHBrowseForFolderA`, bandeja con `Shell_NotifyIconA`).
- Red del propio producto: `WS2_32.dll` (`getaddrinfo`, sockets por ordinal) y `WININET.dll` para la actualización en vivo.
- Portapapeles e interfaz: `ole32.dll`, `OLEAUT32.dll`, `UxTheme.dll`, GDI y GDI+.
- Impresión de la tarjeta de referencia: `COMDLG32.dll` y `WINSPOOL.DRV`.

## Registro de Windows

La cadena `SOFTWARE\TB Labs\Hot Keyboard` está en el EXE. No se leyó el registro de esta máquina, así que no se sabe qué valores hay escritos ahí. `hk.ini` ya desvía macros y ajustes a la carpeta del ejecutable mediante `%HKEXEDIR%`.
