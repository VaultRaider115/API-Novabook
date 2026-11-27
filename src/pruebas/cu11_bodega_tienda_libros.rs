//! CU-11 · Movimiento Bodega → Tienda de libros (RF07). Actor: Bodega.
//! Traslado suelto y requisición de salida con folio REQ (todo o nada, una
//! editorial por documento); la salida queda PENDIENTE hasta que piso la recibe.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// PU-11-01 · Traslado suelto de bodega a piso
/// Entrada: libro con 20 en bodega y 0 en piso; POST /api/bodega/movimientos/libros con 5 piezas
/// Esperado: 200; bodega 15, piso 5 (también en la respuesta) y el movimiento queda registrado
#[tokio::test]
async fn pu_11_01_traslado_suelto() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PLANETA, 200.0, 0, 20).await;
    let r = c
        .post(&b, "/api/bodega/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 5, "observaciones": "Reabastecimiento" }))
        .await
        .exito();
    assert_eq!(r.data()["stock_origen_restante"], 15);
    assert_eq!(r.data()["stock_destino_nuevo"], 5);
    assert_eq!((c.bodega(p.ean).await, c.piso(p.ean).await), (15, 5));
    assert_eq!(c.escalar_i64("SELECT COALESCE(SUM(cantidad), 0) AS n FROM movimientos WHERE codigo_ean = ?", p.ean).await, 5);
}

/// PU-11-02 · Requisición de salida con folio
/// Entrada: dos libros de Planeta (3 y 2 piezas) con stock en bodega
/// Esperado: 200 con folio REQ-AAAA-n, 5 piezas y editorial Planeta; queda PENDIENTE y las piezas pasan a piso
#[tokio::test]
async fn pu_11_02_requisicion_con_folio() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let a = c.libro(PLANETA, 200.0, 0, 10).await;
    let d = c.libro(PLANETA, 150.0, 0, 10).await;
    let r = c
        .post(&b, "/api/bodega/requisiciones", json!({ "items": [
            { "codigo_ean": a.ean, "cantidad": 3 }, { "codigo_ean": d.ean, "cantidad": 2 }
        ]}))
        .await
        .exito();
    let folio = r.data()["folio"].as_str().unwrap().to_string();
    assert!(folio.starts_with("REQ-"), "{folio}");
    assert_eq!(r.data()["total_piezas"], 5);
    assert!(r.data()["editorial"].as_str().unwrap().contains("Planeta"));
    let det = c.get(&b, &format!("/api/bodega/requisiciones/{folio}")).await.exito();
    assert_eq!(det.data()["estado"], "PENDIENTE");
    assert_eq!((c.bodega(a.ean).await, c.piso(a.ean).await), (7, 3));
    assert_eq!((c.bodega(d.ean).await, c.piso(d.ean).await), (8, 2));
}

/// PU-11-03 · Todo o nada por falta de stock
/// Entrada: requisición con un libro (2 de 10) y otro que pide 50 de 4
/// Esperado: 400 "Stock insuficiente" y ningún libro se mueve
#[tokio::test]
async fn pu_11_03_todo_o_nada() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let a = c.libro(PLANETA, 200.0, 0, 10).await;
    let d = c.libro(PLANETA, 200.0, 0, 4).await;
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [
        { "codigo_ean": a.ean, "cantidad": 2 }, { "codigo_ean": d.ean, "cantidad": 50 }
    ]}))
    .await
    .error(StatusCode::BAD_REQUEST, "Stock insuficiente");
    assert_eq!((c.bodega(a.ean).await, c.piso(a.ean).await), (10, 0));
    assert_eq!(c.bodega(d.ean).await, 4);
}

/// PU-11-04 · Una sola editorial por documento
/// Entrada: requisición con un libro de Planeta y otro de Penguin Random House
/// Esperado: 400 "Una sola editorial por documento" sin mover nada
#[tokio::test]
async fn pu_11_04_una_sola_editorial() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let a = c.libro(PLANETA, 200.0, 0, 10).await;
    let d = c.libro(PENGUIN, 200.0, 0, 10).await;
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [
        { "codigo_ean": a.ean, "cantidad": 1 }, { "codigo_ean": d.ean, "cantidad": 1 }
    ]}))
    .await
    .error(StatusCode::BAD_REQUEST, "Una sola editorial por documento");
    assert_eq!(c.bodega(a.ean).await, 10);
}

/// PU-11-05 · Cantidad en 0 y requisición vacía
/// Entrada: traslado suelto de 0 piezas; requisición sin partidas; requisición con cantidad 0
/// Esperado: 400 "debe ser mayor a 0"; 400 "al menos una partida"; 400 "debe ser mayor a 0"
#[tokio::test]
async fn pu_11_05_cantidades_invalidas() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    c.post(&b, "/api/bodega/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 0 }))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a 0");
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [] }))
        .await
        .error(StatusCode::BAD_REQUEST, "al menos una partida");
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 0 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a 0");
}

/// PU-11-06 · Producto inexistente o de otro tipo
/// Entrada: traslado de libros con un EAN que no existe; traslado de libros con una revista
/// Esperado: 404 "no existe"; 400 "corresponde a un producto de tipo"
#[tokio::test]
async fn pu_11_06_producto_invalido() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/movimientos/libros", json!({ "codigo_ean": ean_unico(978), "cantidad": 1 }))
        .await
        .error(StatusCode::NOT_FOUND, "no existe");
    let rev = c.revista(PLANETA_INFANTIL, 90.0, 0, 5).await;
    c.post(&b, "/api/bodega/movimientos/libros", json!({ "codigo_ean": rev.ean, "cantidad": 1 }))
        .await
        .error(StatusCode::BAD_REQUEST, "corresponde a un producto de tipo");
}

/// PU-11-07 · Solo Bodega baja mercancía a piso
/// Entrada: el Vendedor hace POST /api/bodega/movimientos/libros y POST /api/bodega/requisiciones
/// Esperado: 403 en ambos
#[tokio::test]
async fn pu_11_07_solo_bodega() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let r = c.post(&v, "/api/bodega/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 1 })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = c.post(&v, "/api/bodega/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}
