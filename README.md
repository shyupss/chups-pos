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
- Cantidades y stocks en milésimas, también enteros: 1 unidad = 1000 y 0,532 kg = 532. Las columnas
  terminan en `_mil` (`stock_mil`, `cantidad_mil`). En productos por kg, `precio_venta` es el precio por
  kilo. El subtotal de una línea es `(precio × cantidad_mil + 500) / 1000` con división entera, o sea,
  redondeado al peso más cercano. En la interfaz los decimales solo aparecen al mostrar o al leer lo
  que escribe el usuario.
- Redondeo del efectivo: como en Chile no hay monedas de 1 ni 5 pesos, el total que se paga en efectivo
  se redondea a la decena. Si termina en 1 a 5 baja y si termina en 6 a 9 sube (1.224 → 1.220,
  1.225 → 1.220, 1.226 → 1.230). Lo aplica el servicio de ventas, no la vista, y queda en
  `ventas.redondeo` (puede ser negativo), con total = subtotal − descuento + redondeo. Débito, crédito y
  transferencia no se redondean. Se activa o desactiva con la clave `redondeo_efectivo` de la tabla
  `config` (activo por defecto), que se cambia en Configuración > Caja.
- Ventas y movimientos de stock no se borran: se anulan o se compensan.
- La interfaz nunca carga el catálogo completo; cada comando devuelve solo lo que la pantalla muestra.

## Impresoras

Cada venta genera una boleta: un **comprobante interno**, que no reemplaza la boleta electrónica del SII.
Sale en la impresora térmica y se puede guardar como PDF en `DocumentosCHUPS POSBoletas`
(la carpeta se cambia en Configuración).

**Elegir la impresora.** Instala la impresora en Windows con el driver del fabricante (o "Generic / Text
Only"), sea USB, de red o Bluetooth. Aparece en Configuración > Impresora y escáner; si no está, usa
"Actualizar". CHUPS POS le manda los comandos ESC/POS en modo RAW por la cola de impresión, así que el
driver no los cambia. Elige el ancho del papel (58 mm = 32 caracteres, 80 mm = 48) y usa "Imprimir página
de prueba". Para probar sin impresora está "Archivo (pruebas)", que guarda cada trabajo como un `.bin`
en la carpeta de datos (`impresora-pruebas`).

**Tabla de caracteres.** Si en la página de prueba las tildes o la ñ salen mal, prueba otra tabla, en este
orden:

1. **CP858** (por defecto): la 850 con el €. Funciona en la mayoría de las térmicas Epson y chinas
   (Xprinter, 3nStar, Bixolon).
2. **CP850**: para modelos antiguos que no tienen la 858.
3. **WPC1252**: la de Windows; algunas impresoras chinas solo responden bien a esta.

Lo que la tabla no tiene sale sin tilde, en ASCII o como "?" (por ejemplo, los emojis).

**Cajón.** Se conecta a la impresora y se abre con un pulso. Casi todos usan el pin 2; si con "Probar
cajón" no abre, cambia al pin 5.

**Estado.** El indicador de la barra lateral muestra lo que informa el driver: lista, desconectada o con
error ("Sin papel", "Tapa abierta"…). Se lee al abrir la app y cada 60 s. Muchas térmicas USB no avisan
cuando están apagadas: siguen como "lista" hasta que falla una impresión. Si una impresión falla, la venta
queda guardada igual y el aviso ofrece "Reintentar". Una impresora que no responde no congela la app:
después de 10 s se informa el error.

### Códigos de balanza

Las balanzas con impresora etiquetan los productos por peso con un EAN-13 de uso interno:

```
20 12345 00532 7
│  │     │     └ dígito verificador
│  │     └────── peso en gramos (o precio en pesos)
│  └──────────── código del producto
└─────────────── prefijo (20 a 29)
```

Se activan en Configuración > Impresora y escáner > Códigos de balanza, donde se eligen los prefijos (20-29
por defecto), si la etiqueta trae peso o precio y cuántos dígitos tiene el código del producto (4 a 6; el
resto, hasta el último, es el valor). En la Caja, si el código escaneado no existe como producto y tiene
ese formato, se agrega el producto por kg con ese peso, sin pedirlo. El producto se busca por su código de
barras igual al de la etiqueta, con o sin ceros a la izquierda (`00123` o `123`), y tiene que venderse
por kg. Si la etiqueta trae precio, el peso se calcula con el precio por kilo del producto. Un dígito
verificador incorrecto se rechaza: vuelve a escanear la etiqueta. Hay un campo para probar una etiqueta.

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
