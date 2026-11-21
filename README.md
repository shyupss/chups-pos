# CHUPS POS

Punto de venta de escritorio para Windows 10, hecho con Tauri 2. Escanea productos,
lleva el inventario en el tiempo, maneja fiados e imprime boletas en impresora térmica
o PDF. La prioridad es el rendimiento: debe correr sin lag en PCs modestos.

## Arquitectura (MVC adaptado a Tauri)

| Capa | Dónde | Qué hace |
| --- | --- | --- |
| Vista | `src/` (React + Tailwind) | Muestra datos y captura teclas. Sin lógica de negocio. Llama al backend solo a través de `src/api.ts`. |
| Controlador | `src-tauri/src/comandos/` | Comandos Tauri: reciben, validan y delegan en el modelo. |
| Modelo | `crates/pos-core/` | Servicios (reglas de negocio) y repositorios (SQL). No depende de Tauri, se prueba con `cargo test`. |
| Datos | SQLite embebido | Archivo `chups-pos.db` en la carpeta de datos de la app. Migraciones en `crates/pos-core/migrations/`. |

Reglas del proyecto:

- Montos en pesos como enteros, nunca `float`.
- Ventas y movimientos de stock no se borran: se anulan o se compensan.
- La interfaz nunca carga el catálogo completo; cada comando devuelve solo lo que la pantalla muestra.

## Desarrollo

Requisitos: Node 22, Rust estable y, en Windows, las
[dependencias de Tauri](https://v2.tauri.app/start/prerequisites/) (WebView2 ya viene en Windows 10 actualizado).

```bash
npm install
npm run tauri dev        # app en modo desarrollo
cargo test --workspace   # tests del modelo
npm run tauri build      # instalador (.exe NSIS) en target/release/bundle/nsis/
```

Cada push a `main` y cada PR corre la CI: tests, clippy, chequeo de tipos y el instalador de Windows,
que queda descargable como artefacto del workflow.
