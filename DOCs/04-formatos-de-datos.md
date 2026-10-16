# Formatos de datos

## `hk.ini`

Lo genera el propio programa. En esta copia:

```ini
; Generated automatically by Hot Keyboard
[Main]
MacrosDir = %HKEXEDIR%
SettingsDir = %HKEXEDIR%
```

`%HKEXEDIR%` es la carpeta del ejecutable. Por eso `HotKeyboardData.xml` y las copias `HKBackup*.hk4backup` están junto a `HotKeyb.exe`, y no en el perfil de usuario.

## `HotKeyboardData.xml`

Documento XML con tres bloques:

1. Identidad de la aplicación (nombre y versión 6.5.112) y fecha `<Created>`.
2. `<HotKeyboardSettings>`: geometría de la ventana, banderas de interfaz, red, teclas globales del propio programa, variables y un campo `RegInfo` opaco. Ese campo no se reproduce aquí.
3. `<LocalMacros>`: el árbol de macros.

Ajustes vistos en esta instalación, con el valor que tenían el 2026-10-08:

| Ajuste | Valor observado | Lectura |
| --- | --- | --- |
| Ventana principal | 856 × 556 | tamaño guardado |
| `TextShortcutDelay` | 100 | milisegundos del atajo de texto |
| `DisableHIDInput` | 0 | la vía HID sigue habilitada |
| `UseExtendedKeyFlag` | 0 | |
| `RunAsAdminOnVista` | 0 | |
| `MinimizeOnClose` | 0 | |
| `UseShellExtension` | 0 | |
| `ShowInPlaceHelp` | 0 | |
| `MacroSharingEnabled` | 0 | |
| Red | `localhost:25552` | conexión al arranque desactivada |
| Teclas globales (`Enable`, `MainWindow`, `RefCard`, `Record`, `NewMacro`, `QuickReminder`) | `0x0000` | ninguna asignada |
| `Variables` | vacío | |

`UserKeyboardLayout` y `AutomaticBackupPath` están vacíos. Las copias `HKBackupNN` están igual en la carpeta del programa; el campo de ruta personalizada no se usa.

## Forma de una macro

Cada macro es un elemento `<Macro>` con:

- `class`: el tipo (`MPasteMacro`, `MMenuMacro`, `MRecordMacro`, …).
- `guid`: identidad estable.
- `disableBalloons`.
- `<Name autoname="1">`.
- `<Parent>` opcional, con el GUID del menú padre.
- `<Activations>` con cero o más `<Activation class="…" Modifiers="0x..">código</Activation>`.
- `<Conditions>`.
- `<Body>` según la clase.

### Menú

```xml
<Macro class="MMenuMacro" guid="{…}" disableBalloons="0">
  <Name autoname="1">Local</Name>
  <Activations/>
  <Conditions/>
  <Body>
    <MenuAction enableChildren="1" displayAsIcons="0"
                centerPointX="2147483647" centerPointY="2147483647">0</MenuAction>
    <RepeatCount forever="0">1</RepeatCount>
  </Body>
</Macro>
```

`2147483647` es el máximo de un entero de 32 bits con signo. Aquí funciona como «sin coordenada fija».

### Pegar texto

El cuerpo es RTF dentro de `<Text method="0">`. El `method` numérico no se descifró más allá de ver el valor 0 en todas las macros de texto de este archivo. El RTF lo genera el control de texto de Windows (`Riched20`).

### Grabar y reproducir

```xml
<MousePosition mouseEnabled="1">0</MousePosition>
<PlaybackSpeed mul="10" div="10"/>
<Repeat forever="1">1</Repeat>
<Events>…base64…</Events>
```

`mul="10" div="10"` es velocidad 1.0. `forever="1"` repite sin fin. `<Events>` es base64 de un búfer binario. Esa codificación no se interpretó en esta pasada: no hay todavía un mapa de eventos (tecla, clic, espera, coordenada).

### Atajo

En las macros de esta instalación el activador es siempre:

```xml
<Activation class="MMacroActivationHotkey" Modifiers="0x02">0x0056</Activation>
```

Inferencia, alineada con las constantes de Windows y con `Keyboards/default.txt`, no con una ejecución:

- `Modifiers` usa la máscara de `RegisterHotKey`: `0x01` Alt, `0x02` Control, `0x04` Shift, `0x08` Windows.
- El número interior es un código virtual. `0x0056` es `V`, `0x006D` es el menos del teclado numérico. `default.txt` documenta esa misma tabla (`0x006D=Num -`, `0x0070=F1`, etc.).

En el archivo hay atajos con `0x02` (Control) y con `0x04` (Shift). No hay Alt ni tecla Windows en esta instalación.

## Qué usa esta instalación

Conteo sobre `HotKeyboardData.xml`:

- 1 `MMenuMacro` llamado `Local`, sin atajo propio.
- 11 `MPasteMacro` colgando de ese menú. Son frases de chat de rol, cada una con un atajo Control o Shift.
- 1 `MRecordMacro`, «Play keys and mouse forever», atajo sin modificadores en el menos numérico, ratón incluido, repetición infinita.
- Ninguna condición, ninguna variable, ningún otro tipo de acción.

Ese perfil es la pista del primer entregable: un atajo global que escribe un texto, más adelante una grabación que se puede repetir. El texto concreto de las frases no se copia a la documentación.

## Copias `HKBackupNN.hk4backup`

Quince archivos, del 00 al 14. Los quince empiezan por el mismo XML (`<HotKeyboardSettings>`). No son un formato binario distinto: son el documento de datos, rotado.

Tamaños: 00–06 tienen 10 996 bytes, igual que el XML actual; del 07 al 14 van bajando hasta 3 051. No se hizo una comparación byte a byte, así que «mismo tamaño» no prueba que 00 y el XML actual sean idénticos.

## Layouts de teclado

`Keyboards/default.txt` se titula `[Default Extended Keyboard]` y lista códigos virtuales de Windows con nombre legible: ratón, IME, cursores, numérico, F1–F24, navegador, volumen y OEM.

Los otros 23 archivos son mapas de teclas extra de modelos concretos (Logitech, Microsoft, Genius, Chicony, Compaq, etc.). Dos formas:

- Con título entre corchetes y luego `0x00AB=Browser Favorites`.
- Solo líneas `código=nombre`, como `Genius Slimstar 310.txt`, que empieza directo en `0x0125=Windows Messenger`.

Los códigos por encima de `0x0100` son teclas de fabricante o de aplicación (Messenger, Excel, iTouch) que el layout por defecto no nombra. La herramienta `KeyboardLayoutTool.exe` existe para descubrir esas teclas en un teclado nuevo.

## Nombres de archivo que el programa espera

Cadenas del EXE, además de los que ya están en la carpeta:

- `HotKeyboardData.xml`
- `HotKeyboardData.lock`
- `HotKeyboard exported macros file`
- `%sHotKeyboard%i.exe` (el proceso de 64 bits encaja con este patrón de nombre)
