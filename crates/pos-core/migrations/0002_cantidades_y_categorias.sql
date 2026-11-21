-- Fase 2: productos por peso.
--   * Cantidades y stocks en milésimas (INTEGER): 1 unidad = 1000, 0,532 kg = 532.
--     Los nombres de columna llevan _mil para que la unidad quede explícita.
--   * productos.unidad: 'unidad' o 'kg'. En productos por kg, precio_venta es el precio por kilo.
-- RENAME COLUMN actualiza también los CHECK e índices que nombran la columna.

ALTER TABLE productos ADD COLUMN unidad TEXT NOT NULL DEFAULT 'unidad'
    CHECK (unidad IN ('unidad', 'kg'));

ALTER TABLE productos RENAME COLUMN stock TO stock_mil;
ALTER TABLE productos RENAME COLUMN stock_minimo TO stock_minimo_mil;
UPDATE productos SET stock_mil = stock_mil * 1000, stock_minimo_mil = stock_minimo_mil * 1000;

ALTER TABLE venta_items RENAME COLUMN cantidad TO cantidad_mil;
UPDATE venta_items SET cantidad_mil = cantidad_mil * 1000;

ALTER TABLE movimientos_stock RENAME COLUMN cantidad TO cantidad_mil;
ALTER TABLE movimientos_stock RENAME COLUMN stock_resultante TO stock_resultante_mil;
UPDATE movimientos_stock
SET cantidad_mil = cantidad_mil * 1000, stock_resultante_mil = stock_resultante_mil * 1000;

-- Redondeo del efectivo (Chile no tiene monedas de 1 ni 5 pesos): total = subtotal - descuento + redondeo.
-- Puede ser negativo. Solo se aplica en efectivo y se puede desactivar con la clave de config.
ALTER TABLE ventas ADD COLUMN redondeo INTEGER NOT NULL DEFAULT 0;
INSERT OR IGNORE INTO config (clave, valor) VALUES ('redondeo_efectivo', '1');

-- Categorías: la tabla existe desde la 0001. Este índice sirve al filtro por categoría de la lista
-- de productos, que ordena por nombre, y reemplaza al índice simple por categoría.
DROP INDEX idx_productos_categoria;
CREATE INDEX idx_productos_categoria_nombre ON productos (categoria_id, nombre COLLATE NOCASE);
