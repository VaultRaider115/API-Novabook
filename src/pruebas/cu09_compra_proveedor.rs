//! CU-09 · Registrar compra a proveedor (RF17). Actor: Bodega.
//! Una editorial por factura; las piezas que llegan entran a bodega; costo
//! mayor a $0 y no mayor que el precio de venta (hito 2).

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn partida(ean: i64, cantidad: i32, costo: f64) -> Value {
    json!({ "codigo_ean": ean, "tipo_producto": "libro", "cantidad": cantidad, "costo_unitario": costo })
}

/// PU-09-01 · Compra completa de dos títulos
/// Entrada: factura F-#### de Planeta; libro A 10 pzs a $150 y libro B 4 pzs a $80 (precios $250 y $200)
/// Esperado: 200; total $1,820.00, 14 piezas; bodega de A +10 y de B +4; la factura aparece en el historial y en la bitácora
#[tokio::test]
async fn pu_09_01_compra_completa() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let a = c.libro(PLANETA, 250.0, 0, 0).await;
    let bb = c.libro(PLANETA, 200.0, 0, 0).await;
    let factura = format!("F-{}", 7000 + unico());
    let r = c
        .post(&b, "/api/bodega/compras", json!({
            "id_proveedor": PLANETA, "factura": factura,
            "items": [partida(a.ean, 10, 150.0), partida(bb.ean, 4, 80.0)]
        }))
        .await
        .exito();
    assert_eq!(r.data()["total"], 1820.0);
    assert_eq!(r.data()["total_articulos"], 14);
    assert!(r.data()["incidencia"].is_null());
    assert_eq!(c.bodega(a.ean).await, 10);
    assert_eq!(c.bodega(bb.ean).await, 4);
    let id = r.data()["id_compra"].clone();
    let lista = c.get(&b, "/api/bodega/compras").await.exito();
    assert!(lista.data().as_array().unwrap().iter().any(|x| x["id_compra"] == id && x["factura"] == factura.as_str()));
    assert!(c.en_bitacora(&format!("Factura #{factura}")).await);
}

/// PU-09-02 · Costo contra precio de venta (hito 2)
/// Entrada: libro de $249; costo $249.01; luego costo $249.00
/// Esperado: 400 "se vendería con pérdida" sin mover stock; con costo igual al precio, 200
#[tokio::test]
async fn pu_09_02_costo_contra_precio() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PLANETA, 249.0, 0, 0).await;
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [partida(p.ean, 1, 249.01)] }))
        .await
        .error(StatusCode::BAD_REQUEST, "se vendería con pérdida");
    assert_eq!(c.bodega(p.ean).await, 0);
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [partida(p.ean, 1, 249.0)] }))
        .await
        .exito();
    assert_eq!(c.bodega(p.ean).await, 1);
}

/// PU-09-03 · Costo obligatorio
/// Entrada: costo unitario $0
/// Esperado: 400 "debe ser mayor a $0"
#[tokio::test]
async fn pu_09_03_costo_cero() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [partida(p.ean, 1, 0.0)] }))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a $0");
}

/// PU-09-04 · Producto fuera del catálogo
/// Entrada: EAN válido que no está registrado
/// Esperado: 404 "no está registrado; regístralo primero en Alta en Almacén"
#[tokio::test]
async fn pu_09_04_producto_fuera_del_catalogo() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [partida(ean_unico(978), 1, 50.0)] }))
        .await
        .error(StatusCode::NOT_FOUND, "no está registrado");
}

/// PU-09-05 · Una sola editorial por factura
/// Entrada: factura de Planeta con un libro de Penguin Random House
/// Esperado: 400 "no pertenece al proveedor" y no entra nada a bodega (todo o nada)
#[tokio::test]
async fn pu_09_05_producto_de_otro_proveedor() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let propio = c.libro(PLANETA, 100.0, 0, 0).await;
    let ajeno = c.libro(PENGUIN, 100.0, 0, 0).await;
    c.post(&b, "/api/bodega/compras", json!({
        "id_proveedor": PLANETA, "items": [partida(propio.ean, 3, 50.0), partida(ajeno.ean, 1, 50.0)]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "no pertenece al proveedor");
    assert_eq!(c.bodega(propio.ean).await, 0);
}

/// PU-09-06 · Cantidades incoherentes
/// Entrada: llegaron 5 con facturadas 3; llegaron 0 de 2 facturadas (única partida)
/// Esperado: 400 "corrige la cantidad facturada"; 400 "No llegó ninguna pieza"
#[tokio::test]
async fn pu_09_06_cantidades_incoherentes() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    let mut it = partida(p.ean, 5, 50.0);
    it["cantidad_facturada"] = json!(3);
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [it] }))
        .await
        .error(StatusCode::BAD_REQUEST, "corrige la cantidad facturada");
    let mut it = partida(p.ean, 0, 50.0);
    it["cantidad_facturada"] = json!(2);
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [it] }))
        .await
        .error(StatusCode::BAD_REQUEST, "No llegó ninguna pieza");
}

/// PU-09-07 · Compra sin partidas y factura demasiado larga
/// Entrada: items vacío; factura de 31 caracteres (límite 30)
/// Esperado: 400 "al menos un producto"; 400 "no puede pasar de 30 caracteres"
#[tokio::test]
async fn pu_09_07_validaciones_del_documento() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [] }))
        .await
        .error(StatusCode::BAD_REQUEST, "al menos un producto");
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "factura": "F".repeat(31), "items": [partida(p.ean, 1, 50.0)] }))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 30 caracteres");
}

/// PU-09-08 · Solo Bodega registra compras
/// Entrada: el Vendedor hace POST /api/bodega/compras
/// Esperado: 403
#[tokio::test]
async fn pu_09_08_solo_bodega() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    let r = c.post(&v, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "items": [partida(p.ean, 1, 50.0)] })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
}
