# Keyboard Macros OS — investigación

Carpeta de notas de la etapa investigativa. El programa todavía no se implementa.

El objetivo es un gestor de macros de teclado de código abierto, usable en Windows y en Linux. La referencia de comportamiento es la copia local de Hot Keyboard Pro que está en `Hot Keyboard Pro/`. Estas notas describen lo observado en ese artefacto para poder reimplementar la idea con diseño propio.

## Cómo leer esto

1. [01-inventario.md](01-inventario.md) — qué archivos hay y qué identidad tiene cada binario.
2. [02-arquitectura-de-entrada.md](02-arquitectura-de-entrada.md) — cómo captura y reproduce teclado y ratón en Windows.
3. [03-tipos-de-macro.md](03-tipos-de-macro.md) — acciones, formas de activarlas y condiciones.
4. [04-formatos-de-datos.md](04-formatos-de-datos.md) — XML, copias de seguridad, layouts y el perfil de uso de esta instalación.
5. [05-diseno-multiplataforma.md](05-diseno-multiplataforma.md) — qué conviene conservar en Keyboard Macros OS y qué hay que sustituir en Linux.
6. [06-pendientes.md](06-pendientes.md) — lo que esta pasada no cerró.

## Límite de la investigación

Hot Keyboard es software propietario de Imposant (copyright 1998-2018 en los recursos de versión). El proyecto nuevo tiene que ser una reimplementación limpia:

- Se documenta comportamiento, nombres de conceptos y formatos de los datos del usuario.
- No se copia código, recursos gráficos, textos de ayuda ni el esquema de registro.
- `RegInfo` aparece en el XML como un bloque opaco. No se analiza ni se reproduce.
- Las credenciales de red del XML no se copian a estos documentos.

## Evidencia y límites de esta pasada

La inspección estática usó REA (`open_binary`, `inspect_pe_resources`) y una lectura directa de cabeceras PE, imports, exports, cadenas y archivos de datos.

Ghidra 12.1.4 descompiló el arranque del gancho; el detalle está en la arquitectura de entrada. Hopper e IDA siguen sin usarse. El pseudocódigo es una lectura del descompilador, no el fuente original. Una llamada a una API en la tabla de imports demuestra que el binario está preparado para usarla; no demuestra en qué momento la ejecuta. Las conclusiones de ejecución quedan marcadas como inferencia.

El programa no se lanzó durante esta pasada.
