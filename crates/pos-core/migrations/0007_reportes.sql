-- Fase 6: reportes y datos.
--   * Cada línea vendida guarda el costo del producto al momento de vender, igual que ya guardaba
--     el precio: la ganancia de una venta no cambia cuando después cambia el costo.
--   * Las líneas anteriores se rellenan con el costo actual del producto y quedan marcadas como
--     estimadas (costo_estimado = 1). Las de productos sin costo quedan en NULL: sin costo no se
--     inventa ganancia.
ALTER TABLE venta_items ADD COLUMN costo_unitario INTEGER CHECK (costo_unitario >= 0);
ALTER TABLE venta_items ADD COLUMN costo_estimado INTEGER NOT NULL DEFAULT 0
    CHECK (costo_estimado IN (0, 1));

UPDATE venta_items
SET costo_unitario = (SELECT p.costo FROM productos p WHERE p.id = venta_items.producto_id),
    costo_estimado = 1
WHERE (SELECT p.costo FROM productos p WHERE p.id = venta_items.producto_id) IS NOT NULL;

-- Pagos de fiados recibidos en un día o rango (Reportes > Hoy y Fiados). Los reportes de ventas
-- usan idx_ventas_fecha e idx_venta_items_venta, que ya existen.
CREATE INDEX idx_pagos_cliente_fecha ON pagos_cliente (fecha);
