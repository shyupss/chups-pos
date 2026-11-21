-- Ventas en espera: el carrito de un cliente que se aparta para atender a otro.
-- Se guardan en la base para no perderlas si se cierra la app. No tocan stock ni folios:
-- son solo una lista de productos y cantidades (en milésimas) hasta que se recuperan y cobran.

CREATE TABLE ventas_espera (
    id         INTEGER PRIMARY KEY,
    nombre     TEXT,
    creada_en  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE ventas_espera_items (
    id               INTEGER PRIMARY KEY,
    venta_espera_id  INTEGER NOT NULL REFERENCES ventas_espera (id) ON DELETE CASCADE,
    producto_id      INTEGER NOT NULL REFERENCES productos (id),
    cantidad_mil     INTEGER NOT NULL CHECK (cantidad_mil > 0),
    orden            INTEGER NOT NULL
);
CREATE INDEX idx_ventas_espera_items_venta ON ventas_espera_items (venta_espera_id, orden);
