// Cada reporte por rango sale a Excel (.xlsx) y a PDF con el nombre del negocio, el rango y la
// fecha de emisión, y con las mismas cifras de la pantalla.

mod comun;

use chrono::Utc;
use pos_core::datos::{self, Valor};
use pos_core::servicios::exportar_reportes::{self, TipoReporte};
use pos_core::servicios::reportes::{self, Rango};
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::zona::{Zona, ZonaLocal};

#[tokio::test]
async fn cada_reporte_sale_a_excel_y_pdf() {
    let pool = comun::con_caja().await;
    sqlx::query("UPDATE negocio SET nombre = 'Almacén Don Pepe'")
        .execute(&pool)
        .await
        .unwrap();
    let mut d = comun::datos(Some("L"), "Leche entera Colun 1 L", 1190, 50);
    d.costo = Some(850);
    let leche = pos_core::servicios::productos::crear(&pool, &d)
        .await
        .unwrap()
        .id;
    for medio in [MedioPago::Debito, MedioPago::Credito] {
        ventas::confirmar(
            &pool,
            &NuevaVenta {
                items: vec![ItemPedido {
                    producto_id: leche,
                    cantidad_mil: 2000,
                }],
                medio_pago: medio,
                monto_recibido: None,
                descuento: 0,
                cliente_id: None,
            },
        )
        .await
        .unwrap();
    }
    let hoy = ZonaLocal.dia(Utc::now());
    let rango = Rango {
        desde: hoy - chrono::Duration::days(6),
        hasta: hoy,
    };
    let dir = tempfile::tempdir().unwrap();
    for tipo in [
        TipoReporte::Ventas,
        TipoReporte::Productos,
        TipoReporte::Ganancia,
        TipoReporte::Turnos,
        TipoReporte::Fiados,
    ] {
        let (libro, nombre) =
            exportar_reportes::libro(&pool, &ZonaLocal, tipo, rango, None, Utc::now())
                .await
                .unwrap();
        assert_eq!(libro.negocio, "Almacén Don Pepe");
        assert!(libro.rango.contains(" al "), "{}", libro.rango);
        assert!(!libro.tablas.is_empty());
        assert!(nombre.contains(&hoy.to_string()), "{nombre}");

        let xlsx = dir.path().join(format!("{nombre}.xlsx"));
        datos::excel::escribir(&libro, &xlsx, &ZonaLocal).unwrap();
        let bytes = std::fs::read(&xlsx).unwrap();
        assert!(bytes.starts_with(b"PK"), "{tipo:?}: no es un .xlsx");

        let pdf = datos::pdf::generar(&libro, &ZonaLocal).unwrap();
        assert!(pdf.starts_with(b"%PDF"), "{tipo:?}");
        if let Some(ruta) = std::env::var_os("GUARDAR_PDF_EJEMPLO") {
            std::fs::write(
                std::path::Path::new(&ruta).join(format!("{nombre}.pdf")),
                &pdf,
            )
            .unwrap();
            std::fs::copy(
                &xlsx,
                std::path::Path::new(&ruta).join(format!("{nombre}.xlsx")),
            )
            .unwrap();
        }
    }

    // El resumen del Excel de ventas dice lo mismo que la pantalla.
    let r = reportes::ventas(&pool, &ZonaLocal, rango).await.unwrap();
    let (libro, _) = exportar_reportes::libro(
        &pool,
        &ZonaLocal,
        TipoReporte::Ventas,
        rango,
        None,
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(libro.tablas[0].filas[0][1], Valor::Pesos(r.total));
    assert_eq!(libro.tablas[0].filas[1][1], Valor::Entero(2));
    // Siete días con sus ventas.
    assert_eq!(libro.tablas[1].filas.len(), 7);
}

#[tokio::test]
async fn un_pdf_largo_pasa_a_varias_paginas() {
    let mut t = datos::Tabla::new(
        "Muchas filas",
        vec![
            datos::Columna::new("Producto", 30),
            datos::Columna::new("Ventas", 12),
        ],
    );
    for i in 0..300 {
        t.fila(vec![
            Valor::texto(format!("Producto {i}")),
            Valor::Pesos(i * 1000),
        ]);
    }
    let libro = datos::Libro {
        titulo: "Prueba".into(),
        negocio: "Almacén".into(),
        rango: String::new(),
        emitido: "21-11-2026 10:00".into(),
        tablas: vec![t],
    };
    let pdf = datos::pdf::generar(&libro, &ZonaLocal).unwrap();
    let veces = |b: &[u8]| pdf.windows(b.len()).filter(|w| *w == b).count();
    let paginas = veces(b"/Type/Page") - veces(b"/Type/Pages");
    assert!(paginas >= 4, "{paginas} páginas");
}
