# Tipos de macro, activación y condiciones

Los nombres de clase salen de tres sitios que coinciden: el RTTI de `HotKeyb.exe`, las cadenas `<ClassName>` de `hkbres.dll` y el índice `hotkeyb.hhc`. El XML del usuario solo usa tres de estas clases.

## Acciones

Clases que el producto presenta como tipos de macro. La descripción corta sale de cadenas de `hkbres.dll` o del título del tema de ayuda.

| Clase en datos | Idea |
| --- | --- |
| `MPasteMacro` | Pegar texto. El cuerpo visto es RTF. Admite trozos dinámicos (fecha, portapapeles, archivo, texto pedido al vuelo). |
| `MRecordMacro` | Grabar y reproducir teclado y ratón, con velocidad y repetición. |
| `MOpenFileMacro` / `MLaunchAppMacro` | Abrir un archivo o ejecutar un programa. La ayuda lista «Open a file» dos veces, en temas distintos (`openfile_help.htm` y `execute_help.htm`). |
| `MOpenFolderMacro` | Abrir una carpeta. |
| `MOpenWebPageMacro` | Abrir una página. Hay plantillas con `$(input:…)` y `$(selection)`. |
| `MWindowManipulationsMacro` | Manipular ventanas. |
| `MMenuMacro` | Agrupar otras macros. Puede mostrar un menú o ejecutar los hijos en cadena. |
| `MSoundVolumeMacro` | Volumen por dispositivo. |
| `MExtReplaceMacro` | Buscar y reemplazar, con opción de expresión regular al estilo Perl y límite de longitud. |
| `MNetworkMacro` | Conectar, desconectar o alternar la conexión con el servidor de Hot Keyboard, o lanzar un comando de red. |
| `MShutdownMacro` | Apagar o cerrar Windows. |
| `MMediaPlayerControlMacro` | Controlar Windows Media Player. En RTTI antiguo también aparecen `CWMPMacro`, `CWinampMacro` y `CCDPlayerMacro`. |
| `MDelayMacro` | Espera dentro de una cadena de macros. |
| `MRunAnotherMacro` | Ejecutar otra macro. |
| `MEvaluateExpressionMacro` | Calcular una expresión y guardar variables. Ejemplo citado en la interfaz: `$variable1 > sqrt($variable2)`. |
| `MReminderMacro` | Recordatorio. |
| `MPrintScreenMacro` | Captura de pantalla, con ruta y nombre que aceptan scripting `${{ … }}`. |
| `MSaveVariableMacro` | Conservar variables entre reinicios. La cadena de la interfaz dice que los valores de la lista se guardan al reiniciar Hot Keyboard. |

RTTI antiguo, sin clase `M*` equivalente vista en el catálogo de `hkbres.dll`: `CDialMacro`, `CBrowseMacro`, `CReplaceMacro`, `CVariableMacro`, `CModifyVariableMacro`, `CSmartAddMacro`, `CExecuteMacro`, `CLinkMacro`, `CRootMacro`. Parte de eso quedó absorbido por los tipos nuevos (marcación dentro de red, reemplazo simple dentro del extendido, enlace dentro de «ejecutar otra macro»).

## Formas de activar

Estructuras RTTI `MMacroActivation*`:

| Clase | Uso según la ayuda y las cadenas |
| --- | --- |
| `MMacroActivationHotkey` | Atajo de teclado. Es la única usada en el XML de esta instalación. |
| `MMacroActivationShortcut` | Atajo de texto. Hay un retardo `TextShortcutDelay` (100 ms en este XML). |
| `MMacroActivationGesture` | Gesto. |
| `MMacroActivationMultitap` | Varias pulsaciones. |
| `MMacroActivationSchedule` | Programación. |
| `MMacroActivationWinStartup` | Al arrancar Windows, con posible espera después de lanzar Hot Keyboard. |
| `MMacroActivationWindowTitle` | Cuando aparece una ventana con cierto título. |

La ayuda agrupa el relato en: hotkey, atajo de texto, ventana emergente, programador, arranque y condiciones.

## Condiciones

Estructuras `MMacroCondition*`. Una cadena de la interfaz resume cada una:

- `MMacroConditionWindowTitle`: la ventana activa tiene ese título. Varios títulos se separan con `;`.
- `MMacroConditionWindowClass`: la clase de ventana activa coincide. También separadas con `;`.
- `MMacroConditionProcessName`: el proceso de la ventana activa coincide. Separados con `;`.
- `MMacroConditionExpression`: la macro solo corre si la expresión es verdadera.

## Scripting

Hay un lenguaje propio, documentado en la ayuda como tipos, operadores, funciones y variables. En recursos se ven dos sintaxis:

- `${{ … }}` en rutas de captura y en ejemplos de HTML.
- `$(input:TEXTO)` y `$(selection)` en URLs de búsqueda.

No se extrajo la gramática completa. Queda pendiente si hace falta para la primera versión.

## Línea de comandos

Cadenas del EXE: `-runmacro` y `-runmacrowait`, más el nombre de evento `hkb_runmacrowait_%u`. La ayuda tiene un tema «Command line parameters» que no se leyó entero.

## Organización

Las macros forman un árbol. Cada una tiene un GUID y, si no es raíz, un `<Parent>` con el GUID del menú. Se pueden importar y exportar («Hot Keyboard exported macros file», «version 1.7»). Hay copias automáticas, protección por contraseña de una macro o de un menú, y uso compartido por carpeta o por servidor.

El servidor, en este XML, apunta a `localhost` puerto `25552`, con intentos e intervalo de reconexión. `MacroSharingEnabled` está en 0. Las credenciales no se documentan.
