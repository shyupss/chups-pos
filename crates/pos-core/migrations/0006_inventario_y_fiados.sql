-- Fase 5: inventario, compras a proveedores, clientes y pagos de fiados.
--   * Las compras suman stock con movimientos 'compra' que guardan su compra_id. Anular una compra
--     la compensa con movimientos negativos: nada se borra.
--   * Un pago del cliente (pagos_cliente) se reparte en abonos, uno por venta fiada. El saldo de
--     una venta fiada es total - lo que bajaron sus devoluciones - sus abonos vigentes (los de
--     pagos no anulados). Al llegar a cero pasa a 'pagada' con pagada_en; medio_pago sigue 'fiado'.
--   * Los abonos de una venta anulada o devuelta de más pasan como saldo a favor a otras deudas del
--     cliente: son abonos con traslado_de_venta_id.

CREATE TABLE proveedores (
    id         INTEGER PRIMARY KEY,
    nombre     TEXT NOT NULL CHECK (trim(nombre) <> ''),
    rut        TEXT,
    telefono   TEXT,
    contacto   TEXT,
    nota       TEXT,
    activo     INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0, 1)),
    creado_en  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_proveedores_nombre ON proveedores (nombre COLLATE NOCASE);

CREATE TABLE compras (
    id                  INTEGER PRIMARY KEY,
    proveedor_id        INTEGER REFERENCES proveedores (id),
    fecha               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- N° de factura o guía del proveedor.
    documento           TEXT,
    total               INTEGER NOT NULL CHECK (total >= 0),
    nota                TEXT,
    pagada_con_caja     INTEGER NOT NULL DEFAULT 0 CHECK (pagada_con_caja IN (0, 1)),
    -- El retiro del turno con que se pagó, si fue con efectivo de la caja.
    movimiento_caja_id  INTEGER REFERENCES movimientos_caja (id),
    anulada_en          TEXT,
    motivo_anulacion    TEXT
);
CREATE INDEX idx_compras_fecha ON compras (fecha);
CREATE INDEX idx_compras_proveedor ON compras (proveedor_id, fecha);

-- Costo por unidad o, en productos por kg, por kilo. subtotal = (costo × cantidad_mil + 500) / 1000.
CREATE TABLE compra_items (
    id              INTEGER PRIMARY KEY,
    compra_id       INTEGER NOT NULL REFERENCES compras (id),
    producto_id     INTEGER NOT NULL REFERENCES productos (id),
    cantidad_mil    INTEGER NOT NULL CHECK (cantidad_mil > 0),
    costo_unitario  INTEGER NOT NULL CHECK (costo_unitario >= 0),
    subtotal        INTEGER NOT NULL CHECK (subtotal >= 0)
);
CREATE INDEX idx_compra_items_compra ON compra_items (compra_id);
CREATE INDEX idx_compra_items_producto ON compra_items (producto_id);

-- Movimientos: la compra de origen y el motivo de ajustes y mermas.
ALTER TABLE movimientos_stock ADD COLUMN compra_id INTEGER REFERENCES compras (id);
ALTER TABLE movimientos_stock ADD COLUMN motivo TEXT;
CREATE INDEX idx_movimientos_tipo_fecha ON movimientos_stock (tipo, fecha);
CREATE INDEX idx_movimientos_fecha ON movimientos_stock (fecha);
CREATE INDEX idx_movimientos_compra ON movimientos_stock (compra_id) WHERE compra_id IS NOT NULL;

-- Productos bajo el mínimo (la insignia de Inventario): una consulta por este índice.
CREATE INDEX idx_productos_bajo_minimo ON productos (stock_mil - stock_minimo_mil)
    WHERE activo = 1 AND stock_minimo_mil > 0;

-- Clientes. limite_credito NULL = sin límite. creado_en no puede tener un DEFAULT con expresión en
-- ALTER TABLE: los existentes quedan con la fecha de esta migración y los nuevos la ponen al crear.
ALTER TABLE clientes ADD COLUMN rut TEXT;
ALTER TABLE clientes ADD COLUMN direccion TEXT;
ALTER TABLE clientes ADD COLUMN limite_credito INTEGER CHECK (limite_credito >= 0);
ALTER TABLE clientes ADD COLUMN creado_en TEXT;
UPDATE clientes SET creado_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE creado_en IS NULL;
CREATE INDEX idx_clientes_nombre ON clientes (nombre COLLATE NOCASE);

-- Lo que el cliente entregó de una vez. monto es lo que baja la deuda; en efectivo, lo que entró a
-- la caja es monto + redondeo (a la decena, como al cobrar).
CREATE TABLE pagos_cliente (
    id                  INTEGER PRIMARY KEY,
    cliente_id          INTEGER NOT NULL REFERENCES clientes (id),
    fecha               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    monto               INTEGER NOT NULL CHECK (monto > 0),
    redondeo            INTEGER NOT NULL DEFAULT 0,
    monto_recibido      INTEGER,
    vuelto              INTEGER,
    medio_pago          TEXT NOT NULL
                        CHECK (medio_pago IN ('efectivo', 'debito', 'credito', 'transferencia')),
    turno_id            INTEGER REFERENCES turnos (id),
    nota                TEXT,
    -- Un pago mal ingresado se anula: sus abonos dejan de contar y el dinero sale del turno.
    anulado_en          TEXT,
    motivo_anulacion    TEXT,
    turno_anulacion_id  INTEGER REFERENCES turnos (id)
);
CREATE INDEX idx_pagos_cliente_cliente ON pagos_cliente (cliente_id, fecha);
CREATE INDEX idx_pagos_cliente_turno ON pagos_cliente (turno_id);
CREATE INDEX idx_pagos_cliente_turno_anulacion ON pagos_cliente (turno_anulacion_id)
    WHERE turno_anulacion_id IS NOT NULL;

-- Cada pago se reparte en abonos (uno por venta). Los anteriores a esta versión quedan sin pago.
ALTER TABLE abonos ADD COLUMN pago_id INTEGER REFERENCES pagos_cliente (id);
ALTER TABLE abonos ADD COLUMN traslado_de_venta_id INTEGER REFERENCES ventas (id);
CREATE INDEX idx_abonos_pago ON abonos (pago_id);
CREATE INDEX idx_abonos_traslado ON abonos (traslado_de_venta_id) WHERE traslado_de_venta_id IS NOT NULL;

-- Ventas: cuándo quedó saldada una fiada, la deuda total del cliente justo después de la venta
-- (sale en la boleta) y el ajuste a la decena del reembolso de una anulación.
ALTER TABLE ventas ADD COLUMN pagada_en TEXT;
ALTER TABLE ventas ADD COLUMN deuda_tras_venta INTEGER;
ALTER TABLE ventas ADD COLUMN redondeo_anulacion INTEGER;
UPDATE ventas SET redondeo_anulacion = reembolso_anulacion - total
WHERE reembolso_anulacion IS NOT NULL;

-- Devoluciones: la parte que bajó la deuda de una fiada. Lo que salió en dinero es monto - rebaja.
-- En la fase 4 una devolución de fiada bajaba la deuda completa.
ALTER TABLE devoluciones ADD COLUMN rebaja_deuda INTEGER NOT NULL DEFAULT 0 CHECK (rebaja_deuda >= 0);
UPDATE devoluciones SET rebaja_deuda = monto WHERE medio_reembolso IS NULL;

CREATE TRIGGER compras_no_se_borran BEFORE DELETE ON compras BEGIN
    SELECT RAISE(ABORT, 'Las compras no se borran: se anulan');
END;
CREATE TRIGGER compra_items_no_se_borran BEFORE DELETE ON compra_items BEGIN
    SELECT RAISE(ABORT, 'Las compras no se borran: se anulan');
END;
CREATE TRIGGER pagos_cliente_no_se_borran BEFORE DELETE ON pagos_cliente BEGIN
    SELECT RAISE(ABORT, 'Los pagos no se borran: se anulan');
END;
CREATE TRIGGER abonos_no_se_borran BEFORE DELETE ON abonos BEGIN
    SELECT RAISE(ABORT, 'Los abonos no se borran: se anula su pago');
END;
CREATE TRIGGER proveedores_con_compras_no_se_borran BEFORE DELETE ON proveedores
WHEN EXISTS (SELECT 1 FROM compras WHERE proveedor_id = old.id) BEGIN
    SELECT RAISE(ABORT, 'Un proveedor con compras no se borra: se desactiva');
END;
CREATE TRIGGER clientes_con_ventas_no_se_borran BEFORE DELETE ON clientes
WHEN EXISTS (SELECT 1 FROM ventas WHERE cliente_id = old.id) BEGIN
    SELECT RAISE(ABORT, 'Un cliente con ventas no se borra: se desactiva');
END;
