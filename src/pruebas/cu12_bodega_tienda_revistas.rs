//! CU-12 · Movimiento Bodega → Tienda de revistas (RF08). Actor: Bodega.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// PU-12-01 · Traslado suelto de revistas
/// Entrada: revista con 30 en bodega; POST /api/bodega/movimientos/revistas con 12 piezas
/// Esperado: 200; bodega 18 y piso 12
#[tokio::test]
async fn pu_12_01_traslado_suelto() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 30).await;
    let r = c
        .post(&b, "/api/bodega/movimientos/revistas", json!({ "codigo_ean": p.ean, "cantidad": 12 }))
        .await
        .exito();
    assert_eq!(r.data()["stock_destino_nuevo"], 12);
    assert_eq!((c.bodega(p.ean).await, c.piso(p.ean).await), (18, 12));
}

/// PU-12-02 · Requisición de salida de revistas
/// Entrada: dos revistas de Planeta Mexico Infantil (4 y 6 piezas)
/// Esperado: 200 con folio REQ y 10 piezas; queda PENDIENTE
#[tokio::test]
async fn pu_12_02_requisicion_de_revistas() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let a = c.revista(PLANETA_INFANTIL, 95.0, 0, 10).await;
    let d = c.revista(PLANETA_INFANTIL, 80.0, 0, 10).await;
    let r = c
        .post(&b, "/api/bodega/requisiciones", json!({ "items": [
            { "codigo_ean": a.ean, "cantidad": 4 }, { "codigo_ean": d.ean, "cantidad": 6 }
        ]}))
        .await
        .exito();
    assert_eq!(r.data()["total_piezas"], 10);
    let folio = r.data()["folio"].as_str().unwrap();
    let det = c.get(&b, &format!("/api/bodega/requisiciones/{folio}")).await.exito();
    assert_eq!(det.data()["estado"], "PENDIENTE");
    assert_eq!(c.piso(a.ean).await + c.piso(d.ean).await, 10);
}

/// PU-12-03 · Libro en la ruta de revistas
/// Entrada: POST /api/bodega/movimientos/revistas con el EAN de un libro
/// Esperado: 400 "corresponde a un producto de tipo"
#[tokio::test]
async fn pu_12_03_libro_en_ruta_de_revistas() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let l = c.libro(PLANETA, 100.0, 0, 5).await;
    c.post(&b, "/api/bodega/movimientos/revistas", json!({ "codigo_ean": l.ean, "cantidad": 1 }))
        .await
        .error(StatusCode::BAD_REQUEST, "corresponde a un producto de tipo");
}

/// PU-12-04 · Stock insuficiente
/// Entrada: revista con 3 en bodega; traslado de 4
/// Esperado: 400 "Stock insuficiente" y la bodega sigue en 3
#[tokio::test]
async fn pu_12_04_stock_insuficiente() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 3).await;
    c.post(&b, "/api/bodega/movimientos/revistas", json!({ "codigo_ean": p.ean, "cantidad": 4 }))
        .await
        .error(StatusCode::BAD_REQUEST, "Stock insuficiente");
    assert_eq!(c.bodega(p.ean).await, 3);
}

/// PU-12-05 · Revistas de dos editoriales en un documento
/// Entrada: requisición con una revista de Planeta Mexico Infantil y otra de Editorial Televisa
/// Esperado: 400 "Una sola editorial por documento"
#[tokio::test]
async fn pu_12_05_una_sola_editorial() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let a = c.revista(PLANETA_INFANTIL, 95.0, 0, 5).await;
    let d = c.revista(4, 95.0, 0, 5).await;
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [
        { "codigo_ean": a.ean, "cantidad": 1 }, { "codigo_ean": d.ean, "cantidad": 1 }
    ]}))
    .await
    .error(StatusCode::BAD_REQUEST, "Una sola editorial por documento");
}

/// PU-12-06 · Observaciones demasiado largas
/// Entrada: traslado con observaciones de 151 caracteres (límite 150)
/// Esperado: 400 "no puede pasar de 150 caracteres"
#[tokio::test]
async fn pu_12_06_observaciones_largas() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 5).await;
    c.post(&b, "/api/bodega/movimientos/revistas", json!({ "codigo_ean": p.ean, "cantidad": 1, "observaciones": "o".repeat(151) }))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 150 caracteres");
}
