# Qué llevar a Keyboard Macros OS

El programa nuevo no tiene que clonar Hot Keyboard. Tiene que cubrir el uso real de esta instalación y dejar sitio para el resto, en Windows y en Linux, sin arrastrar el modelo de ganchos de 32/64 bits.

## Primera versión, según el uso observado

Lo que esta copia hace de verdad:

1. Un árbol pequeño de macros, con un grupo.
2. Un atajo global (Control o Shift + tecla, o una tecla suelta) que inserta un texto en la aplicación que está delante.
3. Una grabación de teclado y ratón que se repite.

Eso define el núcleo:

- Catálogo de macros con id estable, nombre, padre, activador y cuerpo.
- Activador de teclado con modificadores.
- Acción «escribir este texto».
- Acción «reproducir esta secuencia», en una segunda entrega, cuando el formato de eventos esté definido por nosotros.

El texto debe guardarse como texto, no como RTF de un control de Windows. Un importador puede leer el RTF de Hot Keyboard y quedarse con el texto plano.

## Capa que cambia con el sistema

| Necesidad | Windows, visto en Hot Keyboard | Linux, a diseñar |
| --- | --- | --- |
| Ver teclas de todos los programas | `SetWindowsHookEx` en DLL de 32 y de 64 bits, más Raw Input / HID | evdev o un portal de escritorio. Un solo lector cubre todos los procesos; no hace falta un ayudante de otro ancho de bits. |
| Atajo aunque la tecla también llegue a la app | `RegisterHotKey`, o comerse el evento en el gancho | grabar el dispositivo, o un protocolo del compositor. En Wayland esto depende de la sesión (portal de atajos globales, o permisos de evdev). |
| Escribir texto en la app de delante | `keybd_event` | uinput, o el método de texto del compositor. Pegar por portapapeles es más fiable para frases con acentos que simular cada tecla. |
| Clics y movimiento | `mouse_event` | uinput |
| «Solo si esta ventana está activa» | título, clase y nombre de proceso | en X11 hay título y PID; en Wayland casi no hay título global. Conviene dejar la condición como opcional y por plataforma. |
| Volumen, apagado, Media Player, bandeja | APIs de Windows | fuera del núcleo. Cada una sería un módulo. |

La decisión de producto que más afecta a Linux es el texto: para frases de rol, pegar por el portapapeles (o escribir por el protocolo de texto) evita el lío de distribuciones de teclado. Simular teclas hace falta para la grabación, no para «insertar esta frase».

## Formato propio

Conviene un archivo nuestro, legible, con versión de esquema. Un importador aparte lee `HotKeyboardData.xml` y se queda con menú, nombre, atajo y texto. El base64 de `<Events>` se importa solo cuando sepamos leerlo; hasta entonces esa macro queda marcada como no convertida.

No hace falta reproducir `hk.ini`, el candado, las quince copias rotativas ni el servidor en el puerto 25552. Una copia de seguridad puede ser «guardar el archivo» o un historial simple del propio programa.

## Módulos que pueden esperar

Salen del producto original y no aparecen en el uso de esta instalación:

- Atajo de texto, gesto, multipulsación, agenda y arranque.
- Condiciones de ventana, proceso y expresión.
- Menú visual, cadena de macros, espera, ejecutar otra macro.
- Volumen, red, apagado, navegador, carpetas, captura de pantalla, Media Player.
- Scripting `${{ }}`.
- Compartir macros por carpeta o por servidor.
- Layouts de teclas de fabricante. Útiles el día que alguien tenga un teclado con teclas mudas; `default.txt` ya demuestra que la tabla base es la de códigos virtuales de Windows, y en Linux el equivalente son códigos evdev.

## Forma interna sugerida

Tres piezas, las mismas en los dos sistemas, con un borde distinto detrás:

1. **Almacén.** Lee y escribe el archivo de macros.
2. **Motor.** Decide qué macro corre cuando llega un atajo y ejecuta la acción.
3. **Borde del sistema.** Entrega eventos de teclado y realiza «pegar texto», «pulsar tecla» y «mover ratón».

En Windows el borde puede usar un gancho de bajo nivel o Raw Input, y para pegar texto `SendInput` de Unicode o el portapapeles. Hot Keyboard usa `keybd_event`; el programa nuevo puede usar la API actual. En Linux el borde habla con evdev y uinput, o con el portal de la sesión si se quiere evitar el acceso directo a los dispositivos.

La interfaz (lista, editor, icono de bandeja) viene después del motor. Hot Keyboard concentra la UI en `hkbres.dll` (más de cien diálogos). No hay que reconstruir esa superficie para tener macros útiles.
