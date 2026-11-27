//! CU-14 · Movimiento Tienda → Bodega de revistas (RF12). Actor: Vendedor.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// PU-14-01 · Retorno de revistas sin bajada previa
/// Entrada: revista con 6 en piso que nunca se bajó; traslado suelto de 6 a bodega
/// Esperado: 200; piso 0 y bodega 6
#[tokio::test]
async fn pu_14_01_retorno_sin_bajada() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 6, 0).await;
    c.post(&v, "/api/vendedor/movimientos/revistas", json!({ "codigo_ean": p.ean, "cantidad": 6 }))
        .await
        .exito();
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (0, 6));
}

/// PU-14-02 · Regla de la mitad con revistas
/// Entrada: Bodega baja 9 revistas con requisición; el Vendedor pide regresar 5 y luego 4
/// Esperado: 5 → 400 "como máximo la mitad" (máximo 4); 4 → 200
#[tokio::test]
async fn pu_14_02_regla_de_la_mitad() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let v = c.token(VENDEDOR).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 9).await;
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 9 }] }))
        .await
        .exito();
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 5 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "como máximo la mitad");
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 4 }] }))
        .await
        .exito();
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (5, 4));
}

/// PU-14-03 · Libro en la ruta de revistas
/// Entrada: POST /api/vendedor/movimientos/revistas con un libro en piso
/// Esperado: 400 "corresponde a un producto de tipo"
#[tokio::test]
async fn pu_14_03_libro_en_ruta_de_revistas() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let l = c.libro(PLANETA, 100.0, 3, 0).await;
    c.post(&v, "/api/vendedor/movimientos/revistas", json!({ "codigo_ean": l.ean, "cantidad": 1 }))
        .await
        .error(StatusCode::BAD_REQUEST, "corresponde a un producto de tipo");
}

/// PU-14-04 · Todo o nada en la requisición de entrada
/// Entrada: requisición de dos revistas de la misma editorial; una pide más de lo que hay en piso
/// Esperado: 400 "Stock insuficiente" y ninguna se mueve
#[tokio::test]
async fn pu_14_04_todo_o_nada() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let a = c.revista(PLANETA_INFANTIL, 95.0, 5, 0).await;
    let d = c.revista(PLANETA_INFANTIL, 95.0, 1, 0).await;
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [
        { "codigo_ean": a.ean, "cantidad": 2 }, { "codigo_ean": d.ean, "cantidad": 3 }
    ]}))
    .await
    .error(StatusCode::BAD_REQUEST, "Stock insuficiente");
    assert_eq!((c.piso(a.ean).await, c.piso(d.ean).await), (5, 1));
}

/// PU-14-05 · Cantidad en 0
/// Entrada: traslado suelto de revistas con cantidad 0
/// Esperado: 400 "debe ser mayor a 0"
#[tokio::test]
async fn pu_14_05_cantidad_cero() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 3, 0).await;
    c.post(&v, "/api/vendedor/movimientos/revistas", json!({ "codigo_ean": p.ean, "cantidad": 0 }))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a 0");
}
