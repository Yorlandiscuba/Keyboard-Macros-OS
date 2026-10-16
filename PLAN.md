# Plan de Keyboard Macros OS

Lista viva del proyecto. Se actualiza cuando una tarea cambia de estado.

Leyenda:

- `[x]` terminado
- `[/]` trabajando
- `[ ]` falta

El programa ya tiene el primer corte en Rust: archivo, motor y borde de Windows. La investigación de Hot Keyboard Pro está en `DOCs/`.

## Decisiones

- `[x]` Lenguaje: Rust. Primera plataforma: Windows. El binario se llama `kmos`.
- `[x]` Interfaz gráfica: ventana para listar, crear, editar y borrar macros. Cerrar la ventana la deja en la bandeja.
- `[x]` Licencia MIT. El texto está en `LICENSE`.
- `[x]` Nombre del repositorio: `keyboard-macros-os`.
- `[x]` Alcance de esta primera rebanada: atajo global que inserta un texto, en Windows. Linux queda previsto en el motor, que no llama al sistema.

## Investigación ya hecha

- `[x]` Inventario de `Hot Keyboard Pro` 6.5.0.112: binarios, ayuda, XML y layouts. `DOCs/01-inventario.md`.
- `[x]` Catálogo de tipos de macro, activaciones y condiciones. `DOCs/03-tipos-de-macro.md`.
- `[x]` Formato de `HotKeyboardData.xml`, `hk.ini`, copias y mapas de teclado. `DOCs/04-formatos-de-datos.md`.
- `[x]` Perfil real de esta instalación: un grupo, once textos con atajo, una grabación en bucle.
- `[x]` Arranque del gancho con Ghidra: `InstallHKHook`, ayudante de 64 bits solo si la arquitectura es AMD64, bits `0x400` y `0x80`. `DOCs/02-arquitectura-de-entrada.md`.
- `[x]` Ordinales 1–11 de `hkhook45.dll`.
- `[x]` Dos usos de `HotKeyboard_FileMapping`: banderas del gancho, y el recado de Smart Add / ejecutar macro.
- `[x]` Límite de producto: no se clona el registro ni `RegInfo`.

## Investigación que sigue, en paralelo al código

Ninguna de estas bloquea «atajo → texto».

- `[ ]` Orden real en el EXE entre `RegisterHotKey`, Raw Input y los ganchos.
- `[ ]` Resto de bits de `ChangeHandlingFlags`.
- `[ ]` Cuerpo de `FUN_10001242` y de los procedimientos de teclado y ratón.
- `[ ]` Formato de `<Events>` con una grabación corta y conocida. Hasta entonces el importador guarda el blob y lo marca como no reproducible.
- `[ ]` Comprobar, ejecutando el programa, si pegar texto inserta el RTF o simula teclas.
- `[ ]` Leer el CHM solo cuando entre en alcance el scripting o la línea de comandos.

## Primera versión

Núcleo, en este orden:

- `[x]` Repositorio en `Keyboard Macros OS` con instrucciones de compilación en el README: rustup, host GNU y MinGW (`as.exe`).
- `[x]` Archivo de macros propio, JSON, esquema versión 1. Campos: id, nombre, padre opcional, atajo opcional (modificadores + tecla), acción `insert_text`, `play` o `unconverted`.
- `[x]` Almacén: leer, validar y guardar ese archivo. `macros.example.json`.
- `[x]` Motor: dado un atajo, elegir la macro y pedir «insertar este texto». No llama al sistema.
- `[x]` Contrato del borde: el motor entrega la acción; cada sistema la ejecuta. Hoy solo Windows.
- `[x]` Borde de Windows: `RegisterHotKey` y pegar por el portapapeles con Ctrl+V. Probado: el atajo escribe en el Bloc de notas y se puede repetir sin soltar Control.
- `[x]` Prueba manual: Control o Shift más una tecla escribe una frase en el Bloc de notas.
- `[x]` Ventana mínima: lista, alta, edición y borrado. Cerrar la deja en la bandeja; Salir está en el icono.
- `[x]` Marca de `branding/`: icono de la ventana, K de la bandeja según el tema de la barra, y el logo horizontal como banner. La ventana usa la paleta navy.
- `[x]` Importador de `HotKeyboardData.xml`: menú, nombre, atajo y texto plano. Las grabaciones se guardan y quedan marcadas como no reproducibles.
- `[x]` Arranque con Windows, como opción. El acceso apunta al ejecutable y al archivo de macros abiertos.

Publicación, cuando el Bloc de notas ya recibe el texto:

- `[x]` Workflow de GitHub que, en cada push a `main`, sube el parche y publica el binario de Windows. Linux entra cuando exista ese borde.
- `[x]` El mismo commit se compila en local siguiendo el README. En GitHub el host es MSVC; en esta máquina, GNU.

## Tipos de macro

El catálogo sale de las clases `M*Macro` de `hkbres.dll` y de `HotKeyb.exe`. En el ejecutable también hay nombres viejos `C*Macro` (pegar, menú, volumen, Winamp, Windows Media Player, marcar, asistente). Esas clases viejas ya quedaron absorbidas por las `M*`; no se rehacen aparte. `MEditMacro` es el editor, no una macro que el usuario ejecuta. Dos cadenas del ejecutable están mal escritas (`MRemiderMacro`, `MSoundVoulmeMacro`) y no son tipos nuevos.

Cada tipo de abajo es una acción nuestra, con nuestro archivo y nuestra ventana. No se copia el diálogo ni el texto de ayuda del programa original.

### Acciones

- `[x]` Pegar texto (`MPasteMacro`). Hecho: texto plano, atajo e importación del RTF a texto. Falta dentro de este tipo: fecha, portapapeles, contenido de un archivo y texto pedido al vuelo.
- `[/]` Grupo (`MMenuMacro`). Hecho: el nombre del grupo en la macro hija, y una cadena que ejecuta otras macros por id, en orden, con un tope de 8 pasos. Falta: mostrar un menú.
- `[x]` Grabación (`MRecordMacro`). Formato propio: tecla, clic, espera, coordenada, velocidad y repetición. Escape termina la grabación y una repetición infinita. La grabación importada de Hot Keyboard se guarda y sigue sin reproducirse: no se interpreta el base64 de `<Events>`.
- `[x]` Abrir un archivo (`MOpenFileMacro`). Ruta, programa opcional y argumentos. Sin programa, Windows usa la asociación del archivo. En el ejecutable original los campos vecinos son `OpenWithClass` y `OpenWithParams`, y la llamada es `ShellExecuteExW`. Esta instalación no tiene ninguna macro de este tipo, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Ejecutar un programa (`MLaunchAppMacro`). Ruta, argumentos, carpeta de inicio, minimizado, traer al frente si ya hay una ventana de ese ejecutable, y ejecutar como administrador. Ese último caso usa el verbo `runas` y Windows pide confirmación. En el ejecutable los campos vecinos son `FileName`, `StartIn`, `ProgramName`, `runMinimized`, `switchToExisting` y `runAsAdmin`. El nombre visible queda en la macro; no hay un segundo nombre de programa. Esta instalación no tiene ninguna, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Abrir una carpeta (`MOpenFolderMacro`). Una ruta, y el atajo la abre en el Explorador. En el ejecutable los campos vecinos son `Folder` y `ItemIDList`; el identificador interno de carpetas especiales no se reproduce. Esta instalación no tiene ninguna, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Abrir una página (`MOpenWebPageMacro`). Dirección `http`, `https` o `ftp`, navegador opcional y ventana nueva. Sin navegador, Windows abre la dirección. La ventana nueva se pide a Chrome, Edge, Brave, Opera, Vivaldi y Firefox. En el ejecutable los campos vecinos son `URL`, `browser` y `newWindow`. Las plantillas `$(input:…)` y `$(selection)` quedan para el lenguaje de expresiones. Esta instalación no tiene ninguna, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Manipular ventanas (`MWindowManipulationsMacro`). Cerrar, minimizar, maximizar, restaurar, minimizar todas, salvapantallas, enviar atrás, vaciar la papelera, siempre visible, cerrar el proceso, mover con un rectángulo, maximizar en un monitor y activar por título. En el ejecutable los campos vecinos son `otherAction`, `Rectangle`, `Monitor` y `Window`. La lista de «otras» acciones incluye cerrar, minimizar, restaurar, minimizar todas, salvapantallas, enviar atrás, papelera, siempre visible y cerrar el proceso. No hay un selector de rectángulo sobre la pantalla: las coordenadas se escriben. Esta instalación no tiene ninguna, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Volumen (`MSoundVolumeMacro`). Subir, bajar o fijar un porcentaje, o alternar el silencio, en el dispositivo de reproducción predeterminado. En el ejecutable los campos vecinos son `AdjustVolume`, `Change` y `percents`. Quedan fuera elegir otro dispositivo (`deviceID`, `controlID`, `muteID`) y ponerlo como predeterminado (`SetDefaultDevice`, con los papeles `console`, `multimedia` y `communications`). Esta instalación no tiene ninguna, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Buscar y reemplazar (`MExtReplaceMacro`). Copia el texto seleccionado, aplica las reglas y pega el resultado. Cada regla tiene búsqueda, reemplazo, expresión regular, ignorar mayúsculas, todas las coincidencias y, si coincide, parar, seguir o volver a la primera. Hay un tope de vueltas y otro de longitud en KB. En el ejecutable los campos vecinos son `Search`, `Replace`, `flags`, `Elements`, `MaxIterations` y `MaxLength`. Quedan fuera teclear el resultado (`PlayKeys`) y la sintaxis extendida de pegado (`AllowPasteSyntax`). Los grupos usan `$1`, `${1}`, `${1U}` y `${1L}`. Esta instalación no tiene ninguna, así que el importador sigue guardándola sin convertir si aparece.
- `[x]` Apagar o cerrar la sesión (`MShutdownMacro`). Bloquear, cerrar sesión, apagar, reiniciar, suspender o hibernar. En el ejecutable los campos vecinos son `force` y `SeShutdownPrivilege`. Forzar solo cierra los programas en cerrar sesión, apagar y reiniciar. Los tests no piden el apagado de verdad.
- `[x]` Controlar el reproductor (`MMediaPlayerControlMacro`). Teclas de medio de Windows: reproducir o pausar, parar, siguiente y anterior. Los nombres viejos de Winamp y del reproductor de Windows no se clonan. En el ejecutable el campo vecino es `Action`.
- `[x]` Espera (`MDelayMacro`). Un tiempo en milisegundos, o esperar a que aparezca una ventana, como mucho 10 minutos. En el ejecutable los campos vecinos son `DelayType`, `DelayPause` y `WindowTitle`. Dentro de una cadena para el resto de la secuencia.
- `[x]` Ejecutar otra macro (`MRunAnotherMacro`). Un id. La cadena se corta a los 8 pasos. En el ejecutable el campo vecino es `MacroToRun`.
- `[x]` Calcular una expresión (`MEvaluateExpressionMacro`) y guardar variables. Números, `$nombre`, suma, resta, producto, división, comparaciones, `&&`, `||`, `sqrt` y `abs`. El resultado puede guardarse en una variable de la sesión. En el ejecutable los campos vecinos son `Expression` y `label`.
- `[x]` Recordatorio (`MReminderMacro`). Un título y un texto, en una ventana aparte, para no frenar los atajos.
- `[x]` Captura de pantalla (`MPrintScreenMacro`). Pantalla, ventana activa o rectángulo, al portapapeles o a un archivo BMP. En el ejecutable los campos vecinos son `Source`, `Destination`, `SourcePosition`, `overwrite` y `path`. PNG y JPEG quedan fuera.
- `[x]` Conservar variables entre arranques (`MSaveVariableMacro`). Guarda en el archivo los nombres indicados. Al abrir el programa, esos valores vuelven a estar disponibles.

### Cómo se disparan

- `[x]` Atajo de teclado (`MMacroActivationHotkey`). Es el único activador que corre hoy, y el único que usa el XML de esta instalación.
- `[x]` Atajo de texto (`MMacroActivationShortcut`). Una frase de letras, números y espacios. El programa solo la compara con lo que se escribe y no la guarda.
- `[x]` Gesto (`MMacroActivationGesture`). Direcciones U, D, L y R con el botón izquierdo del ratón.
- `[x]` Varias pulsaciones (`MMacroActivationMultitap`). La misma tecla, de 2 a 8 veces, en menos de medio segundo entre una y otra.
- `[x]` Agenda (`MMacroActivationSchedule`). Hora y minuto locales, una vez por minuto.
- `[x]` Al arrancar el programa (`MMacroActivationWinStartup`). Distinto de la casilla que abre el programa con Windows: aquí la macro se ejecuta sola cuando la ventana ya está abierta.
- `[x]` Cuando aparece una ventana (`MMacroActivationWindowTitle`). Cuando el título activo empieza a contener el texto.

### Condiciones

Una macro con condición solo corre si la condición se cumple. Varias opciones de título, clase o proceso se separan con `;`. Se puede invertir.

- `[x]` Título de la ventana activa (`MMacroConditionWindowTitle`).
- `[x]` Clase de la ventana activa (`MMacroConditionWindowClass`).
- `[x]` Proceso de la ventana activa (`MMacroConditionProcessName`). Compara el nombre del ejecutable.
- `[x]` Expresión (`MMacroConditionExpression`). La misma expresión que la acción de calcular. Distinto de cero es verdadero.

En Linux el título y el proceso no se ven igual que en Windows. La condición queda opcional y por plataforma.

## Siguiente corte

- `[ ]` Borde de Linux: leer el teclado e insertar texto.
- `[ ]` Teclas de fabricante, el día que hagan falta.

## Fuera del proyecto

- `[x]` Sistema de registro, prueba y `RegInfo` de Hot Keyboard.
- `[x]` Copiar código, recursos o textos de ayuda de Imposant.
- `[x]` Clases viejas `C*Macro`. Su comportamiento, si todavía importa, entra por el tipo `M*` correspondiente.
- `[ ]` Red (`MNetworkMacro`) y el servidor del puerto 25552. No está en el uso de esta instalación y no se clona ese protocolo.
