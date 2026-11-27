//! CU-17 · PDF de devolución a proveedor de libros (RF13, acta del diseño 21).
//! Solo existe si está aprobada; la descargan su solicitante, el Jefe o el
//! Gerente.

use axum::http::StatusCode;

use super::*;

fn texto(r: &Resp) -> String {
    String::from_utf8_lossy(&r.bytes).to_string()
}

/// PU-17-01 · El solicitante descarga el acta de libros aprobada
/// Entrada: devolución de 3 piezas de un libro de $250, aprobada; GET /api/vendedor/devoluciones/{id}/pdf/libros
/// Esperado: 200, application/pdf, el archivo empieza con %PDF y contiene el EAN, el título y el autor del libro
#[tokio::test]
async fn pu_17_01_acta_de_libros() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 250.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 3).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/libros")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type, "application/pdf");
    assert!(r.bytes.starts_with(b"%PDF"));
    let t = texto(&r);
    assert!(t.contains(&p.ean.to_string()), "el acta no trae el EAN");
    assert!(t.contains("Libro de prueba"), "el acta no trae el título");
    assert!(t.contains("Garc"), "el acta no trae el autor bajo el título");
}

/// PU-17-02 · Costo y precio de venta en el acta
/// Entrada: libro de $250 comprado a $140; devolución aprobada
/// Esperado: el acta trae las columnas COSTO y P. VENTA con $140.00 y $250.00
#[tokio::test]
async fn pu_17_02_costo_y_precio() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 250.0, 0, 0).await;
    c.post(&b, "/api/bodega/compras", serde_json::json!({
        "id_proveedor": PLANETA, "items": [{ "codigo_ean": p.ean, "cantidad": 5, "costo_unitario": 140.0 }]
    }))
    .await
    .exito();
    let id = c.devolucion(&v, &p, 2).await;
    c.evaluar(&j, id, true, "").await.exito();
    let t = texto(&c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/libros")).await);
    assert!(t.contains("COSTO") && t.contains("P. VENTA"), "faltan las columnas");
    assert!(t.contains("140.00") && t.contains("250.00"), "faltan los importes");
}

/// PU-17-03 · No hay acta mientras está pendiente o si se rechazó
/// Entrada: acta de una devolución PENDIENTE; acta de una RECHAZADA
/// Esperado: 409 "solo está disponible cuando está aprobada" en ambas
#[tokio::test]
async fn pu_17_03_solo_aprobadas() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 250.0, 0, 10).await;
    let pendiente = c.devolucion(&v, &p, 1).await;
    c.get(&v, &format!("/api/vendedor/devoluciones/{pendiente}/pdf/libros"))
        .await
        .error(StatusCode::CONFLICT, "solo está disponible cuando está aprobada");
    let rechazada = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, rechazada, false, "No procede").await.exito();
    c.get(&v, &format!("/api/vendedor/devoluciones/{rechazada}/pdf/libros"))
        .await
        .error(StatusCode::CONFLICT, "solo está disponible cuando está aprobada");
}

/// PU-17-04 · Quién descarga el acta
/// Entrada: acta aprobada; la piden el Jefe, el Gerente y otro Vendedor que no la solicitó
/// Esperado: 200 (PDF) para Jefe y Gerente; 403 "Solo el solicitante o la jefatura" para el otro Vendedor
#[tokio::test]
async fn pu_17_04_quien_descarga() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 250.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&j, &format!("/api/jefe/devoluciones/{id}/pdf/libros")).await;
    assert!(r.bytes.starts_with(b"%PDF"), "el Jefe no obtuvo el PDF: {}", r.json);
    let g = c.token(GERENTE).await;
    let r = c.get(&g, &format!("/api/gerente/devoluciones/{id}/pdf/libros")).await;
    assert!(r.bytes.starts_with(b"%PDF"), "el Gerente no obtuvo el PDF: {}", r.json);
    let otro = c.usuario_nuevo(3).await;
    let t = c.token(otro).await;
    c.get(&t, &format!("/api/vendedor/devoluciones/{id}/pdf/libros"))
        .await
        .error(StatusCode::FORBIDDEN, "Solo el solicitante o la jefatura");
}

/// PU-17-05 · Acta de una devolución que no existe
/// Entrada: GET del acta de la devolución 999999 como Jefe
/// Esperado: 404 "no encontrada"
#[tokio::test]
async fn pu_17_05_no_existe() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.get(&j, "/api/jefe/devoluciones/999999/pdf/libros")
        .await
        .error(StatusCode::NOT_FOUND, "no encontrada");
}

/// PU-17-06 · El acta de libros no se emite como acta de revistas
/// Entrada: devolución de libros aprobada; GET .../pdf/revistas
/// Esperado: 4xx (el tipo de la ruta debe coincidir con el de la devolución)
#[tokio::test]
async fn pu_17_06_tipo_de_acta_coincide() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 250.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/revistas")).await;
    assert!(r.status.is_client_error(), "generó un acta de revistas para libros ({})", r.status);
}
