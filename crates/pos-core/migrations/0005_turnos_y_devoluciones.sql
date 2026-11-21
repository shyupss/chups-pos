-- Fase 4: turnos de caja, anulaciones y devoluciones.
--   * Un turno va desde que se abre la caja con un monto inicial hasta que se cierra contando el
--     efectivo. Solo puede haber uno abierto y sigue abierto aunque se cierre la app o pase la
--     medianoche.
--   * Las ventas y los abonos guardan el turno en que entraron. Los anteriores a esta versión
--     quedan sin turno (NULL).
--   * Nada se borra: una venta anulada cambia de estado y una devolución es una fila nueva.
--     Los triggers del final lo hacen cumplir en la base.

CREATE TABLE turnos (
    id                 INTEGER PRIMARY KEY,
    abierto_en         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    cerrado_en         TEXT,
    monto_inicial      INTEGER NOT NULL CHECK (monto_inicial >= 0),
    -- Al cerrar: lo que debía haber, lo que se contó y contado - esperado (negativo = falta).
    efectivo_esperado  INTEGER,
    efectivo_contado   INTEGER CHECK (efectivo_contado >= 0),
    diferencia         INTEGER,
    -- Conteo por billete y moneda, en JSON: [{"denominacion": 20000, "cantidad": 3}, ...].
    detalle_conteo     TEXT,
    nota_cierre        TEXT,
    estado             TEXT NOT NULL DEFAULT 'abierto' CHECK (estado IN ('abierto', 'cerrado')),
    CHECK ((estado = 'abierto') = (cerrado_en IS NULL))
);
-- Un solo turno abierto. Es también el índice que consulta cada venta al confirmarse.
CREATE UNIQUE INDEX idx_turnos_abierto ON turnos (estado) WHERE estado = 'abierto';

-- Ingresos y retiros de efectivo durante el turno (cambio, pago a un proveedor...).
CREATE TABLE movimientos_caja (
    id        INTEGER PRIMARY KEY,
    turno_id  INTEGER NOT NULL REFERENCES turnos (id),
    tipo      TEXT NOT NULL CHECK (tipo IN ('ingreso', 'retiro')),
    monto     INTEGER NOT NULL CHECK (monto > 0),
    motivo    TEXT NOT NULL CHECK (trim(motivo) <> ''),
    fecha     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_movimientos_caja_turno ON movimientos_caja (turno_id);

ALTER TABLE ventas ADD COLUMN turno_id INTEGER REFERENCES turnos (id);
ALTER TABLE abonos ADD COLUMN turno_id INTEGER REFERENCES turnos (id);
CREATE INDEX idx_ventas_turno ON ventas (turno_id, fecha_hora);
CREATE INDEX idx_abonos_turno ON abonos (turno_id);

-- Anulación de toda la venta: cuándo, por qué, en qué turno y cuánto se devolvió y cómo.
-- Una venta fiada anulada no devuelve dinero: medio y monto del reembolso quedan NULL.
ALTER TABLE ventas ADD COLUMN anulada_en TEXT;
ALTER TABLE ventas ADD COLUMN motivo_anulacion TEXT;
ALTER TABLE ventas ADD COLUMN turno_anulacion_id INTEGER REFERENCES turnos (id);
ALTER TABLE ventas ADD COLUMN medio_reembolso_anulacion TEXT
    CHECK (medio_reembolso_anulacion IN ('efectivo', 'debito', 'credito', 'transferencia'));
ALTER TABLE ventas ADD COLUMN reembolso_anulacion INTEGER;
CREATE INDEX idx_ventas_turno_anulacion ON ventas (turno_anulacion_id)
    WHERE turno_anulacion_id IS NOT NULL;

-- Devoluciones parciales. La venta lleva el total devuelto y si quedó todo devuelto, para que
-- la lista de ventas no tenga que sumar.
ALTER TABLE ventas ADD COLUMN monto_devuelto INTEGER NOT NULL DEFAULT 0;
ALTER TABLE ventas ADD COLUMN devolucion TEXT CHECK (devolucion IN ('parcial', 'total'));

CREATE TABLE devoluciones (
    id               INTEGER PRIMARY KEY,
    venta_id         INTEGER NOT NULL REFERENCES ventas (id),
    -- NULL si se hizo sin caja abierta (solo posible sin reembolso en efectivo).
    turno_id         INTEGER REFERENCES turnos (id),
    fecha            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Lo devuelto al cliente, ya con el ajuste a la decena si fue en efectivo.
    monto            INTEGER NOT NULL CHECK (monto >= 0),
    redondeo         INTEGER NOT NULL DEFAULT 0,
    -- NULL en una venta fiada: no se devuelve dinero, baja la deuda.
    medio_reembolso  TEXT CHECK (medio_reembolso IN ('efectivo', 'debito', 'credito', 'transferencia')),
    motivo           TEXT NOT NULL CHECK (trim(motivo) <> '')
);
CREATE INDEX idx_devoluciones_venta ON devoluciones (venta_id);
CREATE INDEX idx_devoluciones_turno ON devoluciones (turno_id);

-- Cada línea devuelta: cuánto (en milésimas), su parte del monto y si volvió al stock
-- (si no, el producto estaba dañado o vencido y el movimiento de stock es una merma).
CREATE TABLE devolucion_items (
    id              INTEGER PRIMARY KEY,
    devolucion_id   INTEGER NOT NULL REFERENCES devoluciones (id),
    venta_item_id   INTEGER NOT NULL REFERENCES venta_items (id),
    cantidad_mil    INTEGER NOT NULL CHECK (cantidad_mil > 0),
    monto           INTEGER NOT NULL CHECK (monto >= 0),
    vuelve_a_stock  INTEGER NOT NULL DEFAULT 1 CHECK (vuelve_a_stock IN (0, 1))
);
CREATE INDEX idx_devolucion_items_devolucion ON devolucion_items (devolucion_id);
CREATE INDEX idx_devolucion_items_item ON devolucion_items (venta_item_id);

-- Lo que cuenta la historia de la caja no se borra (y así los folios nunca se reutilizan).
CREATE TRIGGER ventas_no_se_borran BEFORE DELETE ON ventas BEGIN
    SELECT RAISE(ABORT, 'Las ventas no se borran: se anulan');
END;
CREATE TRIGGER venta_items_no_se_borran BEFORE DELETE ON venta_items BEGIN
    SELECT RAISE(ABORT, 'Las líneas de una venta no se borran: se devuelven');
END;
CREATE TRIGGER movimientos_stock_no_se_borran BEFORE DELETE ON movimientos_stock BEGIN
    SELECT RAISE(ABORT, 'Los movimientos de stock no se borran: se compensan');
END;
CREATE TRIGGER turnos_no_se_borran BEFORE DELETE ON turnos BEGIN
    SELECT RAISE(ABORT, 'Los turnos de caja no se borran');
END;
CREATE TRIGGER movimientos_caja_no_se_borran BEFORE DELETE ON movimientos_caja BEGIN
    SELECT RAISE(ABORT, 'Los movimientos de caja no se borran: se compensan');
END;
CREATE TRIGGER devoluciones_no_se_borran BEFORE DELETE ON devoluciones BEGIN
    SELECT RAISE(ABORT, 'Las devoluciones no se borran');
END;
CREATE TRIGGER devolucion_items_no_se_borran BEFORE DELETE ON devolucion_items BEGIN
    SELECT RAISE(ABORT, 'Las devoluciones no se borran');
END;
