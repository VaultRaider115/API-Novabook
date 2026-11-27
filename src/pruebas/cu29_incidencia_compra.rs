//! CU-29 · Registrar incidencia de compra (RF29, paquete incompleto). Actor:
//! Bodega. Si llegan menos piezas de las facturadas se abre una incidencia con
//! el correo al proveedor; los faltantes que llegan después entran a bodega,
//! se suman a la compra y la cierran.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

/// Compra de `llegaron` de `facturadas` piezas a $100; devuelve la respuesta.
async fn compra_incompleta(c: &Ctx, b: &str, p: &Producto, llegaron: i32, facturadas: i32) -> Value {
    c.post(b, "/api/bodega/compras", json!({
        "id_proveedor": p.proveedor, "factura": format!("F-{}", 9000 + unico()),
        "items": [{ "codigo_ean": p.ean, "cantidad": llegaron, "cantidad_facturada": facturadas, "costo_unitario": 100.0 }]
    }))
    .await
    .exito()
    .data()
    .clone()
}

/// PU-29-01 · Paquete incompleto abre una incidencia
/// Entrada: libro de Penguin; facturadas 5, llegaron 3, costo $100
/// Esperado: 200; entran 3 a bodega, la compra vale $300 y se abre una incidencia ABIERTA con 2 pendientes y el correo al proveedor
#[tokio::test]
async fn pu_29_01_abre_incidencia() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PENGUIN, 200.0, 0, 0).await;
    let d = compra_incompleta(&c, &b, &p, 3, 5).await;
    assert_eq!(c.bodega(p.ean).await, 3);
    assert_eq!(d["total"], 300.0);
    let inc = &d["incidencia"];
    assert_eq!(inc["estado"], "ABIERTA");
    assert_eq!(inc["piezas_pendientes"], 2);
    assert_eq!(inc["correo"]["para"], "ventas@penguinrandomhouse.mx");
    assert!(inc["correo"]["cuerpo"].as_str().unwrap().contains(&p.ean.to_string()));
}

/// PU-29-02 · Lista de incidencias abiertas
/// Entrada: GET /api/bodega/incidencias?estado=ABIERTA
/// Esperado: 200; todas están ABIERTA y aparece la recién creada
#[tokio::test]
async fn pu_29_02_lista_abiertas() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PENGUIN, 200.0, 0, 0).await;
    let id = compra_incompleta(&c, &b, &p, 1, 4).await["incidencia"]["id_incidencia"].clone();
    let r = c.get(&b, "/api/bodega/incidencias?estado=ABIERTA").await.exito();
    let filas = r.data().as_array().unwrap();
    assert!(filas.iter().all(|i| i["estado"] == "ABIERTA"));
    assert!(filas.iter().any(|i| i["id_incidencia"] == id));
}

/// PU-29-03 · Reposición parcial y total
/// Entrada: incidencia con 4 pendientes; llegan 1 y después 3
/// Esperado: tras la primera sigue ABIERTA con 3 pendientes; tras la segunda queda CERRADA, la bodega suma 4 y la compra sube a $500
#[tokio::test]
async fn pu_29_03_reposicion() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PENGUIN, 200.0, 0, 0).await;
    let d = compra_incompleta(&c, &b, &p, 1, 5).await;
    let id = d["incidencia"]["id_incidencia"].as_i64().unwrap();
    let r = c
        .post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .exito();
    assert_eq!(r.data()["estado"], "ABIERTA");
    assert_eq!(r.data()["piezas_pendientes"], 3);
    let r = c
        .post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 3 }] }))
        .await
        .exito();
    assert_eq!(r.data()["estado"], "CERRADA");
    assert_eq!(c.bodega(p.ean).await, 5);
    let total: f64 = sqlx::query_scalar::<_, rust_decimal::Decimal>("SELECT total FROM compras_proveedor WHERE id_compras_proveedor = ?")
        .bind(d["id_compra"].as_i64().unwrap())
        .fetch_one(&c.pool)
        .await
        .map(|t| t.to_string().parse().unwrap())
        .unwrap();
    assert_eq!(total, 500.0);
}

/// PU-29-04 · No se recibe más de lo pendiente
/// Entrada: incidencia con 2 pendientes; llegan 3
/// Esperado: 400 "solo faltan 2 pieza(s); se capturaron 3" y la bodega no cambia
#[tokio::test]
async fn pu_29_04_no_mas_de_lo_pendiente() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PENGUIN, 200.0, 0, 0).await;
    let id = compra_incompleta(&c, &b, &p, 2, 4).await["incidencia"]["id_incidencia"].as_i64().unwrap();
    c.post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 3 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "solo faltan 2 pieza(s); se capturaron 3");
    assert_eq!(c.bodega(p.ean).await, 2);
}

/// PU-29-05 · Reposición inválida
/// Entrada: EAN que no está entre los faltantes; cantidad 0; incidencia 999999
/// Esperado: 400 "no está entre los faltantes"; 400 "debe ser mayor a 0"; 404 "No existe la incidencia"
#[tokio::test]
async fn pu_29_05_reposicion_invalida() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PENGUIN, 200.0, 0, 0).await;
    let otro = c.libro(PENGUIN, 200.0, 0, 0).await;
    let id = compra_incompleta(&c, &b, &p, 1, 3).await["incidencia"]["id_incidencia"].as_i64().unwrap();
    c.post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": otro.ean, "cantidad": 1 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "no está entre los faltantes");
    c.post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 0 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a 0");
    c.post(&b, "/api/bodega/incidencias/999999/reposicion", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .error(StatusCode::NOT_FOUND, "No existe la incidencia");
}

/// PU-29-06 · Una incidencia cerrada no se vuelve a reponer
/// Entrada: incidencia cerrada; otra reposición
/// Esperado: 409 "ya está cerrada"
#[tokio::test]
async fn pu_29_06_cerrada() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PENGUIN, 200.0, 0, 0).await;
    let id = compra_incompleta(&c, &b, &p, 1, 2).await["incidencia"]["id_incidencia"].as_i64().unwrap();
    c.post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .exito();
    c.post(&b, &format!("/api/bodega/incidencias/{id}/reposicion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .error(StatusCode::CONFLICT, "ya está cerrada");
}
