-- Fase 3: boletas.
-- La boleta dice "0,532 kg x $2.300/kg" o "3 x $1.190". La línea vendida guarda cómo se vendió,
-- igual que ya copia el nombre y el precio, para que una reimpresión no cambie si después se
-- edita el producto. Las ventas anteriores toman la unidad actual de su producto.

ALTER TABLE venta_items ADD COLUMN unidad TEXT NOT NULL DEFAULT 'unidad'
    CHECK (unidad IN ('unidad', 'kg'));
UPDATE venta_items
SET unidad = (SELECT p.unidad FROM productos p WHERE p.id = venta_items.producto_id);
