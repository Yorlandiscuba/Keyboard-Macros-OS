# Pendientes

Preguntas abiertas al cerrar esta pasada. Ninguna bloquea el diseño del núcleo «atajo → texto».

## Sin desensamblar

Ghidra 12.1.4 ya descompiló el arranque del gancho. Quedó en [02-arquitectura-de-entrada.md](02-arquitectura-de-entrada.md). Sigue abierto:

- el orden entre `RegisterHotKey`, Raw Input y estos ganchos dentro del EXE (las tres vías están importadas; no se siguió cuál se instala primero);
- el significado de los bits de `ChangeHandlingFlags` distintos de `0x80` y `0x400`;
- el cuerpo de `FUN_10001242` (el `WH_GETMESSAGE`) y el de los procedimientos de teclado y ratón;
- el formato binario de `<Events>` en una macro grabada.

## Sin ejecutar

No se abrió el programa. Sigue sin observarse:

- si Control+V de una macro de pegado inserta el texto o simula las teclas del RTF;
- qué hace `Text method="0"` frente a otros métodos;
- si el menos numérico de la macro de grabación entra en conflicto con el menos del teclado;
- el contenido real del registro `SOFTWARE\TB Labs\Hot Keyboard`.

## Formato de grabación

`<Events>` es base64. Hace falta una pasada aparte, con una grabación corta y conocida, para proponer un formato propio. Hasta entonces el importador debe conservar el blob y marcarlo como no reproducible.

## Ayuda y scripting

El índice `hotkeyb.hhc` está leído. El cuerpo del CHM, no. Ahí están la gramática de scripting, los parámetros de la línea de comandos y el detalle de cada acción. Se consulta cuando una de esas funciones entre en el alcance.

## Licencia del producto original

Hay registro, prueba con límite de macros y un campo `RegInfo`. Queda fuera del proyecto. Keyboard Macros OS no necesita ser compatible con ese registro.

## Siguiente paso de producto, cuando se salga de la investigación

Definir el archivo de macros propio y el contrato del borde de sistema (evento de tecla que entra, acción de pegar texto que sale), primero en Windows, con la misma interfaz pensada para enchufar el borde de Linux. El código sigue sin empezar.
