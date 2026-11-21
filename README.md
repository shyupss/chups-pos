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
- Ventas, movimientos de stock, turnos, movimientos de caja y devoluciones no se borran: se anulan o
  se compensan. Desde la migración 0005, unos triggers lo impiden en la misma base, así que los folios
  nunca se reutilizan.
- La interfaz nunca carga el catálogo completo; cada comando devuelve solo lo que la pantalla muestra.

## Turnos de caja y cuadratura

Para vender hay que **abrir la caja**: al entrar a la Caja sin turno abierto aparece "Abrir caja", donde se
cuenta el efectivo con que se empieza (sugiere el monto inicial del turno anterior). Solo puede haber un
turno abierto, y sigue abierto aunque se cierre la app o pase la medianoche. Las ventas en espera sí se
guardan y recuperan sin caja. La barra lateral dice "Caja abierta desde 09:12" o "Caja cerrada".

Durante el turno, "Ingreso o retiro de efectivo" registra el sencillo que entra o el dinero que sale (pago
a un proveedor, depósito), siempre con motivo. Un retiro no puede ser mayor que el efectivo esperado.

**Cerrar caja** tiene dos pasos: contar el efectivo (por billete y moneda, que suma solo, o el monto total)
y revisar el resumen: ventas por medio de pago, anulaciones y devoluciones, ingresos y retiros, y el
efectivo esperado contra el contado. La diferencia sale en verde si cuadra, como aviso si sobra y en rojo si
falta. Al confirmar se imprime el "CIERRE DE CAJA" (si hay impresora) y se puede guardar en PDF
(`cierre-000012.pdf`, en la misma carpeta de las boletas).

```
efectivo esperado = monto inicial + ventas en efectivo + abonos en efectivo
                    + ingresos − retiros − reembolsos en efectivo
```

Una venta cuenta en el turno en que se cobró, aunque después se anule: ese dinero entró. La anulación o la
devolución cuentan en el turno en que se hacen. Así, el resumen de un turno cerrado nunca cambia. Las
ventas anteriores a los turnos (bases de la fase 3) quedan sin turno.

## Anulaciones y devoluciones

En **Ventas** (F5) está el historial: filtros por fecha (Hoy, Ayer, Últimos 7 días, Este mes o un rango),
folio, medio de pago, estado y turno, con la cantidad y el total del filtro al pie. El buscador acepta el
folio escrito o escaneado del código de barras de la boleta (Enter abre esa venta). En el detalle se
reimprime (Ctrl+P; la copia dice "COPIA"), se guarda el PDF, se devuelven productos o se anula.

- **Anular** deja la venta como "anulada" (no se borra; su boleta dice "ANULADA"), repone el stock de todo
  lo vendido y registra el reembolso con motivo. Solo se anulan ventas sin devoluciones; si ya tiene, se
  devuelve el resto. Una venta fiada anulada deja de contar como deuda (si tiene abonos, todavía no).
- **Devolver** es parcial: se elige cuánto de cada línea (por unidad, unidades enteras; por kg, con el
  diálogo de peso), sin pasar de lo vendido menos lo ya devuelto. Cada línea vale
  `subtotal × cantidad devuelta / cantidad vendida`, redondeado al peso, calculado sobre lo acumulado para
  que varias devoluciones sumen exacto el subtotal. "Vuelve al stock" viene marcado; si el producto vuelve
  dañado o vencido se desmarca y queda una merma, sin reponer stock. Cuando todo lo vendido quedó devuelto,
  la venta pasa a "devuelta". Se imprime una "NOTA DE DEVOLUCIÓN".
- **El reembolso**: en efectivo sale de la caja abierta (la exige) y se redondea a la decena como al
  cobrar; con tarjeta o transferencia solo se registra el medio. En una venta fiada no se devuelve dinero:
  baja la deuda.

## PIN de administrador

En Configuración > Seguridad se crea un PIN de 4 a 6 dígitos. Se guarda solo su hash (Argon2id), nunca el
PIN. Protege, según lo que se marque ahí: anular ventas (sí por defecto), devoluciones (sí), retiros de
efectivo (sí), cerrar la caja (no) y cambiar la configuración (sí; se pide una vez mientras Configuración
está abierta). Sin PIN creado, nada se protege. Cambiar la lista de acciones protegidas siempre lo pide.

La verificación ocurre en Rust: cada comando protegido recibe el PIN y el servicio lo valida, así que llamar
al comando directamente sin PIN no sirve. Tras 5 intentos fallidos se bloquea 30 segundos.

**Recuperación.** Al crear el PIN se muestra una sola vez un código de 10 caracteres (`ABCDE-12345`):
anótalo y guárdalo fuera del computador. Con él, "Olvidé el PIN" pone uno nuevo y entrega un código nuevo
(el usado deja de servir). También se guarda solo como hash. Si se pierden el PIN y el código, no hay otra
forma de recuperarlo desde la app.

## Impresoras

Cada venta genera una boleta: un **comprobante interno**, que no reemplaza la boleta electrónica del SII.
Sale en la impresora térmica y se puede guardar como PDF en `Documentos\CHUPS POS\Boletas`
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
