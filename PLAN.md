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
- `[ ]` Elegir el nombre del repositorio.
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
- `[x]` Archivo de macros propio, JSON, esquema versión 1. Campos: id, nombre, padre opcional, atajo (modificadores + tecla), acción `insert_text`.
- `[x]` Almacén: leer, validar y guardar ese archivo. `macros.example.json`.
- `[x]` Motor: dado un atajo, elegir la macro y pedir «insertar este texto». No llama al sistema.
- `[x]` Contrato del borde: el motor entrega la acción; cada sistema la ejecuta. Hoy solo Windows.
- `[x]` Borde de Windows: `RegisterHotKey` y pegar por el portapapeles con Ctrl+V. Probado: el atajo escribe en el Bloc de notas y se puede repetir sin soltar Control.
- `[x]` Prueba manual: Control o Shift más una tecla escribe una frase en el Bloc de notas.
- `[x]` Ventana mínima: lista, alta, edición y borrado. Cerrar la deja en la bandeja; Salir está en el icono.
- `[x]` Marca de `branding/`: icono de la ventana, K de la bandeja según el tema de la barra, y el logo horizontal como banner. La ventana usa la paleta navy.
- `[ ]` Importador de `HotKeyboardData.xml`: menú, nombre, atajo y texto plano. Las grabaciones quedan sin convertir.
- `[ ]` Arranque con Windows, como opción, no como requisito del primer día.

Publicación, cuando el Bloc de notas ya recibe el texto:

- `[ ]` Workflow de GitHub que produzca un binario de Windows y, cuando exista el borde, uno de Linux.
- `[ ]` El mismo commit debe poder compilarse en local siguiendo el README.

## Segunda versión

- `[ ]` Borde de Linux: leer el teclado (evdev o el portal de la sesión) e insertar texto.
- `[ ]` Formato propio de grabación: tecla, clic, espera, coordenada. No el base64 de Hot Keyboard.
- `[ ]` Reproducir una secuencia, con repetición.
- `[x]` Icono de bandeja y «seguir corriendo al cerrar la ventana».

## Más adelante, solo si hace falta

- `[ ]` Condición por ventana o proceso, con comportamiento distinto en Windows, X11 y Wayland.
- `[ ]` Grupos que se ejecutan en cadena, espera y «ejecutar otra macro».
- `[ ]` Atajo de texto, agenda y arranque de sesión.
- `[ ]` Abrir archivo, carpeta o URL.
- `[ ]` Volumen, captura y apagado, cada uno como módulo.
- `[ ]` Teclas de fabricante (el equivalente de `Keyboards/*.txt`).

## Fuera del proyecto

- `[x]` Sistema de registro, prueba y `RegInfo` de Hot Keyboard.
- `[x]` Copiar código, recursos o textos de ayuda de Imposant.
- `[ ]` Servidor de macros en red y el puerto 25552. No está en el uso de esta instalación.
