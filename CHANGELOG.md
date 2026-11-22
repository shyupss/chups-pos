# Cambios

Un apartado por versión, con el formato `## [X.Y.Z] - AAAA-MM-DD`. El de la versión que se publica
sale tal cual como notas del Release (ver `scripts/notas-version.mjs`) y como "notas de la versión"
en Configuración > Acerca de.

## [1.0.0] - 2026-11-22

Primera versión para entregar a negocios. Resume las fases 0 a 7.

### Caja y ventas
- Caja por teclado con lector de códigos: `3*código` para cantidades y `0,5*código` para productos por kilo, ventas en espera, cobro en efectivo, débito, crédito, transferencia y fiado, y redondeo del efectivo a la decena.
- Códigos de balanza (EAN-13 con peso o precio) y cajón de dinero.
- Turnos de caja con apertura, ingresos y retiros, cierre con conteo de billetes y cuadratura.
- Historial de ventas agrupado por día, anulaciones y devoluciones parciales, siempre con su motivo: nada se borra.

### Productos, inventario y fiados
- Productos por unidad o por kilo, con categorías; cantidades guardadas en milésimas y montos en pesos enteros.
- Inventario en el tiempo: movimientos, ajuste por conteo, mermas, compras y proveedores.
- Fiados con límite de crédito, pagos (parciales o de ventas marcadas) y estado de cuenta.

### Boletas e impresión
- Boleta en impresora térmica (ESC/POS, 58 y 80 mm) y en PDF, con logo, código de barras y la leyenda de comprobante interno (no reemplaza la boleta del SII).
- Impresión de cierres de caja, anulaciones, devoluciones, pagos y estados de cuenta.

### Reportes y datos
- Reportes de hoy y por rango (ventas, productos, ganancia, turnos y fiados), con exportación a Excel y PDF.
- Importar y exportar productos y clientes en Excel.
- Respaldos automáticos (al abrir y al cerrar cada turno) y manuales, con verificación, y restauración con respaldo previo.

### Seguridad
- PIN de administrador (Argon2id) con código de recuperación, y acciones protegidas a elección.

### Interfaz
- Estilo Mostrador: colores por rol, botones con una sola acción principal, tema claro y oscuro, y Configuración en secciones.

### Licencia, instalador y entrega (fase 7)
- Licencia permanente por negocio, firmada y verificable sin internet: nunca vence ni deja de valer con el reloj o las actualizaciones. Prueba de 15 días, 7 de gracia y luego modo consulta (se ve, exporta y respalda; no se abre caja ni se vende).
- Herramienta del vendedor `chups-licencias` para generar claves, emitir, verificar y listar licencias.
- Asistente de primer arranque en 5 pasos: negocio, impresora, escáner, PIN y licencia.
- Instalador NSIS en español con licencia de uso, acceso directo en el escritorio y en el menú Inicio, y un desinstalador que conserva tus datos salvo que marques borrarlos.
- Actualizaciones firmadas: se busca como máximo una vez al día, solo con internet, y nunca se instala con un turno abierto; antes de actualizar se hace un respaldo.
- Registro de errores en archivos rotativos, revisión de la base tras un cierre brusco, una sola instancia de la app y memoria del tamaño y la posición de la ventana.
