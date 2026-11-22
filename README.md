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

Una compra a proveedor pagada con la caja es un retiro del turno; los reembolsos incluyen los pagos de
fiados anulados en efectivo. Las ventas fiadas no suman efectivo: entra cuando el cliente paga.

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
  devuelve el resto. Una venta fiada anulada deja de contar como deuda; si el cliente ya había abonado,
  eso pasa como saldo a favor a sus otras deudas y, si no tiene, se le devuelve desde el turno actual.
- **Devolver** es parcial: se elige cuánto de cada línea (por unidad, unidades enteras; por kg, con el
  diálogo de peso), sin pasar de lo vendido menos lo ya devuelto. Cada línea vale
  `subtotal × cantidad devuelta / cantidad vendida`, redondeado al peso, calculado sobre lo acumulado para
  que varias devoluciones sumen exacto el subtotal. "Vuelve al stock" viene marcado; si el producto vuelve
  dañado o vencido se desmarca y queda una merma, sin reponer stock. Cuando todo lo vendido quedó devuelto,
  la venta pasa a "devuelta". Se imprime una "NOTA DE DEVOLUCIÓN".
- **El reembolso**: en efectivo sale de la caja abierta (la exige) y se redondea a la decena como al
  cobrar; con tarjeta o transferencia solo se registra el medio. En una venta fiada no se devuelve dinero:
  baja la deuda de esa venta, nunca por debajo de cero. Si devuelve más de lo que debe (porque ya había
  abonado), lo que sobra pasa a sus otras deudas y lo que no cabe se le devuelve.

## Inventario y ajustes

**Inventario** (F3) tiene cuatro pestañas:

- **Stock**: cada producto con su stock, mínimo y valor a costo, con filtros (buscar, categoría, bajo el
  mínimo, sin stock, negativo). Arriba, el inventario valorizado a costo y a precio de venta. La barra
  lateral muestra cuántos productos están bajo el mínimo; se actualiza cuando cambia el stock, sin sondeo.
- **Movimientos**: todo cambio de stock (venta, compra, ajuste, merma, anulación, devolución) con fecha,
  cantidad, stock resultante y origen (la venta o compra abre su detalle). Filtra por producto, tipo y
  fechas, y pagina por fecha e id para seguir fluida con cientos de miles de filas.
- **Compras** y **Proveedores** (ver abajo).

El stock nunca se edita a mano: queda un movimiento. El **ajuste por conteo** recibe lo que se contó (de
uno o de muchos productos a la vez) y muestra antes de confirmar el resumen de diferencias, en cantidad y
valorizadas a costo; la diferencia se calcula contra el stock al confirmar y todo va en una sola
transacción. La **merma** saca lo que se perdió con su motivo (vencido, dañado, robo, consumo interno u
otro). Ambos piden el PIN si "Ajustar inventario" está protegida (sí por defecto).

## Compras y proveedores

**Registrar compra** (Ctrl+N en Inventario): proveedor (opcional), número de factura o guía, productos con
cantidad y costo unitario (por kg en productos por peso). Suma el stock, deja en cada producto el último
costo y, si se marca "Pagada con efectivo de la caja", registra el retiro en el turno abierto (con el PIN de retiros).
El historial de compras filtra por proveedor y fechas. **Anular** una compra no la borra: la compensa con
movimientos negativos (avisa antes si algún producto quedaría con stock negativo) y, si se pagó con la caja,
devuelve el dinero al turno actual. Pide el PIN de "Ajustar inventario". Los proveedores se crean, editan y
desactivan; nunca se borran.

## Fiados

En el cobro, **Fiado** es la tecla 5 (Alt+5 con el cursor en un campo). Se busca al cliente por nombre,
teléfono o RUT, o se crea con "Nuevo cliente" (nombre y teléfono) sin salir del cobro. Se ve lo que debe,
lo que deberá con esta compra y su crédito disponible. Si la venta pasa el límite de crédito del cliente,
Rust la rechaza; "Fiar sobre el límite" la autoriza con el PIN (acción protegida, sí por defecto). Una venta
fiada no se redondea ni suma efectivo. La boleta dice "FIADO a <cliente>" y la deuda total del cliente
después de esa compra.

**Fiados** (F4) lista a quien debe, de la deuda mayor a la menor, con el total por cobrar. Se resalta al
cliente cuya venta pendiente más antigua tiene más días que "Resaltar deudas antiguas" (Configuración >
Fiados, 30 por defecto). La ficha del cliente muestra sus ventas pendientes (total, lo pagado y lo que
debe de cada una), sus pagos y las ventas ya pagadas. Un cliente se desactiva solo si no debe nada.

## Pagos y estado de cuenta

**Registrar pago** (Ctrl+P en la ficha): monto parcial, "Paga todo" o lo de las ventas marcadas, y medio de
pago. Sin ventas marcadas se aplica de la más antigua a la más nueva; con marcadas, primero a esas. Nunca
puede superar la deuda. En efectivo exige la caja abierta, se redondea a la decena como al cobrar y calcula
el vuelto. Cada venta que llega a cero queda "pagada" con su fecha. Se imprime un "COMPROBANTE DE PAGO" con
las ventas que paga y la deuda restante.

**Anular un pago** mal ingresado pide el PIN de "Anular venta": no se borra, sus ventas vuelven a deberse y,
si fue en efectivo, el dinero sale del turno actual. No se puede si alguna venta que pagó se anuló o
devolvió dinero después.

**Estado de cuenta**: ventas pendientes con lo que se debe de cada una, deuda total, límite y últimos pagos;
se imprime en la térmica o se guarda en PDF (`estado-cuenta-000007.pdf`).

La deuda siempre se calcula en SQL: total de cada venta fiada − lo que bajaron sus devoluciones − sus
abonos vigentes. Nada se borra: pagos y abonos anulados quedan con su motivo.

## Reportes

**Reportes** (F6) abre siempre en **Hoy**: ventas y boletas del día, ticket promedio, ganancia
estimada, comparación con el mismo día de la semana pasada a la misma hora, efectivo esperado del
turno abierto, fiado otorgado y pagos de fiado del día, ventas por hora y por medio de pago, los 10
más vendidos, los productos bajo el mínimo (lleva a Inventario filtrado) y los clientes con fiado
atrasado (lleva a su ficha). Se actualiza al volver a la pantalla y cuando Rust avisa que hubo una
venta, anulación, devolución, pago o movimiento de caja (sin consultar cada tanto).

Las demás pestañas son por rango (hoy, ayer, esta semana, este mes, mes pasado, últimos 30 días,
este año o personalizado):

- **Ventas**: total, boletas, ticket promedio; por día, medio de pago, categoría y día de la
  semana; mapa de calor día × hora.
- **Productos**: ranking por monto o por cantidad (en kg los de peso), filtro por categoría y
  productos sin ventas; cada producto lleva a sus movimientos.
- **Ganancia**: ventas − costo y margen por producto y por categoría. Desde la fase 6 cada venta
  guarda el costo del producto en ese momento; las ventas anteriores usan el costo actual y se
  informan aparte como "costos estimados". Lo vendido sin costo no entra en la ganancia y se lista
  aparte.
- **Turnos**: los cerrados en el rango con esperado, contado y diferencia, y el total de sobrantes
  y faltantes.
- **Fiados**: deuda al cierre del rango, fiado y pagado en el rango, y antigüedad de la deuda.

Las ventas anuladas no cuentan y las devoluciones se descuentan de su venta, así los totales
cuadran con Ventas (F5). La base guarda las fechas en UTC y los reportes agrupan por el día y la
hora del computador, también en los cambios de horario. Cada reporte se exporta a Excel (Ctrl+E) o
PDF (con el nombre del negocio, el rango y la fecha de emisión) en `Documentos\CHUPS POS\Reportes`.

## Importar y exportar Excel

En **Configuración > Datos** se exportan a Excel los productos, el stock con su valorización, las
ventas con su detalle, los movimientos de inventario y las compras (de un rango) y los clientes con
su deuda. Productos, Inventario > Stock y Fiados también tienen su botón. Los montos quedan como
números (se pueden sumar) y las fechas en hora local.

**Importar productos o clientes** acepta .xlsx, .xls, .ods y .csv:

1. Se elige el archivo y la app propone qué columna es cada dato (código, nombre, categoría,
   unidad, precio, costo, stock y stock mínimo; para clientes, nombre, teléfono, RUT, dirección,
   nota y límite). Se puede cambiar a mano.
2. La vista previa separa las filas nuevas, las que actualizan un producto existente (por código
   de barras, con lo que cambia) o un cliente (por RUT o nombre), y las que tienen errores con su
   motivo. Nada se guarda todavía.
3. Al confirmar (pide el PIN si "Importar datos" está protegida, sí por defecto) todo se guarda en
   una transacción; las filas con error se omiten. El stock entra como un ajuste de inventario con
   motivo "Importación", nunca escrito directo. Las categorías que no existen se crean. Las deudas
   de los clientes no se importan: solo nacen de ventas fiadas.

"Descargar planilla de ejemplo" deja una planilla con los encabezados correctos y dos filas. Una
exportación de productos o clientes se puede volver a importar tal cual.

## Respaldos

Un respaldo es una copia completa de la base hecha con `VACUUM INTO` (consistente aunque la caja
esté vendiendo), verificada con `PRAGMA integrity_check`, con nombre
`chups-respaldo-AAAA-MM-DD-HHMM.db`.

- **Automáticos**: al abrir la app si el último tiene más de 24 horas, y al cerrar cada turno. Van
  a la carpeta `Respaldos` junto a los datos de la app (o la que se elija en Configuración > Datos)
  y se conservan los últimos 14 (configurable). Corren en segundo plano: la caja nunca espera, y
  solo se avisa si fallan.
- **Manuales**: "Respaldar ahora" y "Respaldar en…" para otra carpeta, por ejemplo un pendrive.
- Configuración > Datos muestra la ruta y el tamaño de la base, el último respaldo (con aviso si
  tiene más de 3 días) y la lista de respaldos.

**Restaurar**: Configuración > Datos > "Restaurar un respaldo…", elegir el archivo y revisar su
fecha, versión de la base, productos y última venta. Se rechazan los archivos que no son una base
de CHUPS POS y los de una versión más nueva de la app. Siempre pide el PIN (si hay uno). Antes de
reemplazar se respalda la base actual (queda como `…-antes-de-restaurar.db`); después la app se
reinicia y, si el respaldo es de una versión anterior, las migraciones lo ponen al día.

## PIN de administrador

En Configuración > Seguridad se crea un PIN de 4 a 6 dígitos. Se guarda solo su hash (Argon2id), nunca el
PIN. Protege, según lo que se marque ahí: anular ventas (sí por defecto), devoluciones (sí), retiros de
efectivo (sí), cerrar la caja (no), ajustar inventario (sí), fiar sobre el límite (sí), importar datos
(sí) y cambiar la configuración (sí; se pide una vez mientras Configuración está abierta). Restaurar un
respaldo siempre lo pide. Sin PIN creado, nada se protege. Cambiar la lista de acciones protegidas siempre lo pide.

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

## Interfaz: color, botones y Configuración

Los componentes usan solo clases de token (`bg-fondo`, `text-tenue`, `border-linea`…): nunca colores fijos de
Tailwind ni valores de color en los `.tsx`. Los tokens viven en `src/index.css`, con el tema claro y el oscuro
(`data-theme` en `<html>`), y Tailwind no trae la paleta ni las sombras por defecto.

### Sistema de color por rol

Cada color tiene un solo trabajo. Si un componente necesita otro, se usa el rol que corresponde, no el color
que "se ve bien".

| Rol | Tokens | Para qué se usa (y nada más) |
| --- | --- | --- |
| Acción | `--acento`, `--acento-hover`, `--sobre-acento` | Botón primario, enlaces, borde del campo de escaneo y anillo de foco. |
| Selección | `--seleccion` con texto `--acento` | Lo activo o elegido: ítem de la barra lateral, filtro de rango, pestaña, tema elegido, segmento elegido, fila abierta. |
| Resaltado | `--resalte` con barra de 3 px en `--aviso` | Lo que acaba de pasar: el último producto escaneado. |
| Éxito | `--exito`, `--exito-fondo` | Lo que salió bien: venta registrada, respaldo reciente, PIN activo. |
| Aviso | `--aviso`, `--aviso-fondo` | Algo que conviene mirar: bajo el mínimo, "Con devolución", respaldo viejo. |
| Peligro | `--peligro`, `--peligro-fondo` | Errores y lo que descarta o anula: Cancelar venta, Anular, Restaurar. |
| Información | `--info`, `--info-fondo` | Ayudas y avisos neutros, por ejemplo "no reemplaza la boleta del SII". |
| Neutros | `--fondo` (la aplicación), `--superficie` (tarjetas), `--suave` (encabezados de tabla y chips), `--linea` (separadores), `--borde-control` (borde de inputs y botones secundarios), `--texto`, `--tenue` | La estructura. `--borde-control` es más fuerte que `--linea` para que un campo se vea como campo. |
| Visor | `--visor-bg`, `--visor-texto`, `--visor-tenue` | El bloque oscuro del total en la Caja, igual en ambos temas. |
| Papel | `--papel`, `--tinta` | La vista previa de la boleta: blanca con tinta oscura también en el tema oscuro. |

Contraste: los pares texto/fondo de la tabla (incluidos Selección y Resaltado) llegan a 4,5:1 en ambos temas.
`--tenue` sobre `--resalte` en el tema oscuro queda en 4,2:1, así que dentro de la fila resaltada el texto
secundario usa `--texto`.

**Elevación** (solo tarjetas y botones, nunca filas, y sin blur grande: es barato de dibujar): `shadow-s1` para
tarjetas y botones, `shadow-s2` para el visor y los menús, `shadow-s3` para diálogos y avisos flotantes y
`shadow-pri` para el botón primario. No se usa ningún otro `shadow-*`. Los radios son 8 px en controles
(`rounded-lg`) y 12 px en tarjetas y paneles (`rounded-panel`); la clase `tarjeta` junta fondo, borde, radio y
`--s1`.

### Botones

Todo botón pasa por `src/componentes/Boton.tsx`, con `Tecla` para los atajos:

- **Una sola acción primaria por pantalla o diálogo** (la que el usuario viene a hacer: Cobrar, Guardar,
  Respaldar ahora). Todo lo demás es **secundario**.
- Lo opcional ("Cancelar" de un diálogo, "Quitar logo", "Cerrar") es **terciario**.
- Lo que descarta o anula (Cancelar venta, Anular, Restaurar) es **peligro**.
- Tamaños: normal 40 px, chico 32 px, grande 56 px (solo Cobrar) e ícono (un ícono solo, cuadrado de 32 px).
- Sin animaciones: solo una transición de color de 100 ms.

Las filas clicables (listas virtuales, resultados de búsqueda), las pestañas y los ítems de la barra lateral son
`<button>` propios porque no son acciones sueltas: su estado "elegido" usa Selección. Los ajustes de sí/no
usan `Interruptor` (un `<input type="checkbox" role="switch">` real) y los de varias opciones `Opciones`
(radios nativos con aspecto de segmentos).

### Configuración

Configuración es una lista de secciones a la izquierda (`<nav>` con `aria-current`) y, a la derecha, solo la
elegida, en una cuadrícula de dos columnas de tarjetas que pasa a una columna con menos de 1100 px de ancho de
contenido. Para agregar una sección: una entrada en `SECCIONES` de `src/pantallas/Configuracion.tsx` y un
componente que devuelva `<Columnas izquierda={…} derecha={…} />` con `Tarjeta` (título, ayuda y cuerpo) y
`Ajuste` (nombre y ayuda a la izquierda, control a la derecha).

## Licencia

Cada negocio tiene una **licencia permanente**: no vence, no tiene plan y no depende de la versión ni de
las actualizaciones. Funciona sin internet: es un texto firmado con Ed25519 (`CHUPS-XXXXX-XXXXX-…`, o un
archivo `.chups-licencia` con ese mismo texto). La app solo trae la clave **pública**
(`crates/pos-licencia/clave-publica.txt`, incrustada al compilar; también se puede fijar con la variable
`CHUPS_CLAVE_PUBLICA`). La privada nunca entra al repositorio.

**Estados** (los decide Rust, en `abrir_turno` y `confirmar_venta`; la vista solo los muestra):

| Estado | Cuándo | Qué se puede |
| --- | --- | --- |
| Prueba | 15 días desde el primer arranque (clave `dias_prueba`) | Todo. Una línea discreta en la barra lateral dice los días que quedan. |
| Activa | Hay una licencia válida para este equipo | Todo, para siempre: ningún reloj, fecha ni actualización la desactiva. |
| Gracia | 7 días después de la prueba, sin licencia | Todo, con un aviso visible. |
| Consulta | Después de la gracia, sin licencia | Ver y exportar todo, respaldar y restaurar, cobrar fiados y cerrar el turno abierto. **No** abrir un turno nuevo ni registrar ventas. |

El inicio de la prueba se guarda en la base y en `prueba.dat` (carpeta de datos), así que borrar la base o
reinstalar no la reinicia. Se guarda también el mayor instante visto: si el reloj retrocede, los días de
prueba no se recuperan y a partir de 48 horas se avisa. Una venta en curso nunca se corta: el turno que
estaba abierto al terminar la gracia puede seguir vendiendo hasta 24 horas después.

### Cómo emitir una licencia (paso a paso para el vendedor)

La herramienta `tools/chups-licencias` va en el workspace pero **no** en el instalador. Se usa con
`cargo run -p chups-licencias -- <comando>` o compilándola una vez (`cargo build --release -p chups-licencias`,
queda en `target/release/chups-licencias.exe`).

1. **Una sola vez**, crea el par de claves: `chups-licencias generar-claves`. Guarda la privada en
   `%USERPROFILE%\.chups-licencias\clave-privada.key` (se niega a guardarla dentro de un repositorio git) y
   muestra la pública, que va en `crates/pos-licencia/clave-publica.txt` antes de compilar la app.
   **Respalda la privada fuera del computador**: sin ella no puedes emitir licencias, y si cambias de clave
   las licencias ya emitidas dejan de servir en las versiones nuevas. También se puede dar por la variable
   `CHUPS_CLAVE_PRIVADA` (64 dígitos hex) o `CHUPS_CLAVE_PRIVADA_ARCHIVO`.
2. El cliente abre **Configuración > Licencia** y te manda su **código de solicitud** (botón "Copiar"; se
   puede dictar por teléfono: tiene una suma de control que detecta errores de tipeo).
3. Emite la licencia:
   `chups-licencias emitir --negocio "Almacén Don Pepe" --rut 12.345.678-5 --solicitud ABCDE-… --archivo don-pepe`
   Con `--solicitud` queda atada a ese equipo; sin él, se ata al primer equipo donde se active. `--equipos N`
   cambia la cantidad (1 por defecto; para controlar cada equipo conviene emitir una licencia atada por
   cada uno). Queda anotada en `%USERPROFILE%\.chups-licencias\emitidas.csv` (otra ruta con
   `CHUPS_REGISTRO_LICENCIAS`; nunca dentro de un repo).
4. Entrégale el texto (`CHUPS-…`) o el archivo `.chups-licencia`. El cliente lo **pega o lo carga** en
   Configuración > Licencia (o en el paso 5 del asistente de primer arranque) y queda **activa**.
5. `chups-licencias verificar <texto o archivo>` revisa la firma y muestra los datos; `chups-licencias listar`
   muestra lo emitido.

Una licencia alterada, firmada con otra clave o de otro equipo se rechaza con un mensaje claro y no se guarda.
El contacto que ve el cliente se fija al compilar con `CHUPS_CONTACTO_VENDEDOR`.

## Primer arranque

Con la base vacía (sin productos, ventas ni turnos), en vez de la Caja aparece un asistente de 5 pasos:
negocio (el nombre es obligatorio), impresora (con página de prueba), escáner, PIN con su código de
recuperación y licencia (activar o seguir en prueba). Todos se pueden omitir menos el primero. Al final ofrece
importar los productos desde Excel o crear el primero. Una base con datos, como la de la fase 6, nunca lo
pide; se vuelve a abrir desde Configuración > "Asistente de configuración".

## Actualizaciones

Las actualizaciones van firmadas (`tauri-plugin-updater`; la clave pública está en `tauri.conf.json`). La app
busca una como máximo una vez al día, en segundo plano, solo si hay internet (sin internet falla en silencio)
y **nunca con un turno abierto**. Configuración > Acerca de muestra la versión, las notas y "Buscar
actualizaciones". Con un turno abierto se ofrece **"Instalar al cerrar el turno"**; instalar siempre hace antes
un respaldo de la base, y si el respaldo falla no se actualiza.

## Cómo publicar una versión

1. Escribe las novedades en un apartado `## [X.Y.Z] - AAAA-MM-DD` de `CHANGELOG.md`: sale como notas del
   Release y como "Novedades" en Acerca de.
2. Sube la versión en un solo lugar: `node scripts/version.mjs --set X.Y.Z` (la deja en `package.json`,
   `tauri.conf.json`, `Cargo.toml` y `package-lock.json`). La CI falla si no coinciden
   (`npm run version:check`). Haz `cargo build` y commitea también `Cargo.lock`.
3. Una sola vez, en GitHub > Settings > Secrets and variables > Actions, crea:
   - `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: la clave que firma las
     actualizaciones (se crea con `npx tauri signer generate -w <archivo fuera del repo>`; su parte pública va
     en `tauri.conf.json`). **Obligatorios**: sin ellos el workflow se detiene. Si pierdes la clave o su
     contraseña, las instalaciones existentes no podrán actualizarse solas: habría que entregar un instalador
     nuevo a mano.
   - Opcionales, para firmar el `.exe` y el instalador con tu certificado de firma de código:
     `WINDOWS_CERTIFICATE` (el .pfx en base64: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))`)
     y `WINDOWS_CERTIFICATE_PASSWORD`. Sin ellos se construye igual **sin firmar** (Windows SmartScreen avisará
     al instalar) y el resumen del job lo dice.
4. Sube la etiqueta: `git tag v1.0.0 && git push origin v1.0.0`. El workflow `Publicar versión` construye en
   Windows, firma si hay certificado, genera `latest.json` y deja un **Release en borrador** con el instalador
   y las notas. Para probar el flujo usa `v1.0.0-rc.1` (queda como pre-lanzamiento en borrador).
5. Revisa el borrador y publícalo. Las instalaciones existentes ven la versión nueva a través de
   `releases/latest/download/latest.json`.

El instalador (NSIS, perMachine, en español) muestra el contrato de uso (`docs/licencia-de-uso.txt`, un texto
base que debe revisar el dueño) y crea un acceso directo en el escritorio y en el menú Inicio. El
desinstalador **no borra tus datos**, salvo que marques la casilla "Borrar también mis datos".

## Dónde están los datos del cliente y cómo cambiar de computador

Todo vive en `%APPDATA%\cl.chups.pos\` (por ejemplo `C:\Users\<usuario>\AppData\Roaming\cl.chups.pos`):

| Qué | Dónde |
| --- | --- |
| La base de datos | `chups-pos.db` (con `-wal` y `-shm` mientras la app está abierta) |
| Respaldos automáticos y manuales | `Respaldos\` (o la carpeta elegida en Configuración > Datos y respaldos) |
| Inicio de la prueba | `prueba.dat` |
| Registro de errores | `Registros\` |
| Boletas y reportes en PDF o Excel | `Documentos\CHUPS POS\` |

**Cambiar a otro PC** (o reinstalar Windows): 1) en el computador viejo, Configuración > Datos y respaldos >
"Respaldar ahora" (o "Respaldar en…" a un pendrive); 2) instala CHUPS POS en el nuevo; 3) pídele al vendedor
una **licencia nueva** para ese equipo con su código de solicitud (la licencia está atada al computador: la
base copiada no activa la licencia del otro); 4) en el nuevo, Configuración > Datos y respaldos >
"Restaurar un respaldo…" y elige el archivo (pide el PIN si hay uno). El asistente de primer arranque no
aparece en un equipo que ya tiene datos restaurados.

## Desarrollo

Requisitos: Node 22, Rust estable y, en Windows, las
[dependencias de Tauri](https://v2.tauri.app/start/prerequisites/) (WebView2 ya viene en Windows 10 actualizado).

```bash
npm install
npm run tauri dev        # app en modo desarrollo
cargo test --workspace   # tests del modelo
npm run version:check    # package.json, tauri.conf.json y Cargo.toml en la misma versión
npm run tauri build      # instalador (.exe NSIS) en target/release/bundle/nsis/
```

Cada push a `main` y cada PR corre la CI: tests, clippy, chequeo de tipos y el instalador de Windows,
que queda descargable como artefacto del workflow.
