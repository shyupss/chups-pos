-- Esquema inicial del POS.
-- Reglas generales:
--   * Montos en pesos chilenos como INTEGER (sin decimales, sin float).
--   * Fechas en UTC, texto ISO 8601. Se convierten a hora de Chile al mostrar.
--   * Nada se borra en ventas ni movimientos: se anula o se compensa.

-- Datos del negocio que salen en la boleta. Una sola fila (id = 1).
CREATE TABLE negocio (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    nombre       TEXT NOT NULL DEFAULT '',
    rut          TEXT,
    direccion    TEXT,
    telefono     TEXT,
    logo         BLOB,
    mensaje_pie  TEXT NOT NULL DEFAULT 'Gracias por su compra'
);
INSERT INTO negocio (id) VALUES (1);

-- Ajustes clave/valor: impresora elegida, ancho de papel, cajón, etc.
CREATE TABLE config (
    clave  TEXT PRIMARY KEY,
    valor  TEXT NOT NULL
);

CREATE TABLE categorias (
    id      INTEGER PRIMARY KEY,
    nombre  TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE productos (
    id             INTEGER PRIMARY KEY,
    codigo_barras  TEXT UNIQUE,
    nombre         TEXT NOT NULL,
    categoria_id   INTEGER REFERENCES categorias (id),
    precio_venta   INTEGER NOT NULL CHECK (precio_venta >= 0),
    costo          INTEGER CHECK (costo >= 0),
    stock          INTEGER NOT NULL DEFAULT 0,
    stock_minimo   INTEGER NOT NULL DEFAULT 0,
    activo         INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0, 1)),
    creado_en      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_productos_categoria ON productos (categoria_id);

-- Búsqueda rápida por nombre (escribir "coca" encuentra "Coca-Cola 1.5L").
CREATE VIRTUAL TABLE productos_fts USING fts5 (
    nombre,
    content = 'productos',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER productos_fts_ai AFTER INSERT ON productos BEGIN
    INSERT INTO productos_fts (rowid, nombre) VALUES (new.id, new.nombre);
END;
CREATE TRIGGER productos_fts_ad AFTER DELETE ON productos BEGIN
    INSERT INTO productos_fts (productos_fts, rowid, nombre) VALUES ('delete', old.id, old.nombre);
END;
CREATE TRIGGER productos_fts_au AFTER UPDATE OF nombre ON productos BEGIN
    INSERT INTO productos_fts (productos_fts, rowid, nombre) VALUES ('delete', old.id, old.nombre);
    INSERT INTO productos_fts (rowid, nombre) VALUES (new.id, new.nombre);
END;

CREATE TABLE historial_precios (
    id               INTEGER PRIMARY KEY,
    producto_id      INTEGER NOT NULL REFERENCES productos (id),
    precio_anterior  INTEGER NOT NULL,
    precio_nuevo     INTEGER NOT NULL,
    fecha            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_historial_precios_producto ON historial_precios (producto_id, fecha);

-- Clientes a los que se les fía.
CREATE TABLE clientes (
    id        INTEGER PRIMARY KEY,
    nombre    TEXT NOT NULL,
    telefono  TEXT,
    nota      TEXT,
    activo    INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0, 1))
);

-- Cabecera de cada boleta.
CREATE TABLE ventas (
    id              INTEGER PRIMARY KEY,
    folio           INTEGER NOT NULL UNIQUE,
    fecha_hora      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    subtotal        INTEGER NOT NULL,
    descuento       INTEGER NOT NULL DEFAULT 0,
    total           INTEGER NOT NULL,
    medio_pago      TEXT NOT NULL
                    CHECK (medio_pago IN ('efectivo', 'debito', 'credito', 'transferencia', 'fiado')),
    monto_recibido  INTEGER,
    vuelto          INTEGER,
    estado          TEXT NOT NULL CHECK (estado IN ('pagada', 'fiada', 'anulada')),
    cliente_id      INTEGER REFERENCES clientes (id),
    -- Toda venta fiada tiene un cliente asociado.
    CHECK (medio_pago <> 'fiado' OR cliente_id IS NOT NULL)
);
CREATE INDEX idx_ventas_fecha ON ventas (fecha_hora);
CREATE INDEX idx_ventas_cliente_estado ON ventas (cliente_id, estado);

-- Líneas de la boleta. Nombre y precio se copian al vender (snapshot).
CREATE TABLE venta_items (
    id               INTEGER PRIMARY KEY,
    venta_id         INTEGER NOT NULL REFERENCES ventas (id),
    producto_id      INTEGER NOT NULL REFERENCES productos (id),
    nombre           TEXT NOT NULL,
    precio_unitario  INTEGER NOT NULL,
    cantidad         INTEGER NOT NULL CHECK (cantidad > 0),
    subtotal         INTEGER NOT NULL
);
CREATE INDEX idx_venta_items_venta ON venta_items (venta_id);
CREATE INDEX idx_venta_items_producto ON venta_items (producto_id);

-- Pagos parciales o totales de ventas fiadas.
CREATE TABLE abonos (
    id          INTEGER PRIMARY KEY,
    venta_id    INTEGER NOT NULL REFERENCES ventas (id),
    monto       INTEGER NOT NULL CHECK (monto > 0),
    medio_pago  TEXT NOT NULL CHECK (medio_pago IN ('efectivo', 'debito', 'credito', 'transferencia')),
    fecha       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_abonos_venta ON abonos (venta_id);

-- Historial del inventario: cada cambio de stock deja una fila.
CREATE TABLE movimientos_stock (
    id                INTEGER PRIMARY KEY,
    producto_id       INTEGER NOT NULL REFERENCES productos (id),
    tipo              TEXT NOT NULL
                      CHECK (tipo IN ('venta', 'compra', 'ajuste', 'merma', 'devolucion')),
    cantidad          INTEGER NOT NULL,  -- positivo entra, negativo sale
    stock_resultante  INTEGER NOT NULL,
    venta_id          INTEGER REFERENCES ventas (id),
    nota              TEXT,
    fecha             TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_movimientos_producto_fecha ON movimientos_stock (producto_id, fecha);
CREATE INDEX idx_movimientos_venta ON movimientos_stock (venta_id);
