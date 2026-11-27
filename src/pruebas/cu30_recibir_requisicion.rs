//! CU-30 · Recibir requisición de salida (RF30). Actor: Vendedor.
//! La salida queda PENDIENTE; se recibe una vez: CONFIRMADA si todo coincide,
//! CORREGIDA si no (la diferencia se ajusta contra bodega); cada libro va a un
//! mueble de piso con espacio.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// Bodega baja `cantidad` piezas con requisición; devuelve el folio.
async fn salida(c: &Ctx, p: &Producto, cantidad: i32) -> String {
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": cantidad }] }))
        .await
        .exito()
        .data()["folio"]
        .as_str()
        .unwrap()
        .to_string()
}

/// PU-30-01 · La salida aparece pendiente para el Vendedor
/// Entrada: Bodega registra una salida; GET /api/vendedor/requisiciones?tipo=SALIDA&estado=PENDIENTE
/// Esperado: el folio aparece en la lista
#[tokio::test]
async fn pu_30_01_pendiente() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let folio = salida(&c, &p, 4).await;
    let r = c.get(&v, "/api/vendedor/requisiciones?tipo=SALIDA&estado=PENDIENTE").await.exito();
    assert!(r.data().as_array().unwrap().iter().any(|x| x["folio"] == folio.as_str()));
}

/// PU-30-02 · Recepción sin diferencias
/// Entrada: salida de 4; se reciben 4 en el mueble P-900
/// Esperado: 200, estado CONFIRMADA; piso 4 y bodega 6
#[tokio::test]
async fn pu_30_02_confirmada() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let folio = salida(&c, &p, 4).await;
    let r = c
        .post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 4, "id_mueble": MUEBLE_PISO }] }))
        .await
        .exito();
    assert_eq!(r.data()["estado"], "CONFIRMADA");
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (4, 6));
}

/// PU-30-03 · Llegaron menos: se corrige
/// Entrada: salida de 5; se reciben 3
/// Esperado: 200, estado CORREGIDA; piso 3 y las 2 que faltaron regresan a bodega (8)
#[tokio::test]
async fn pu_30_03_corregida() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let folio = salida(&c, &p, 5).await;
    let r = c
        .post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 3, "id_mueble": MUEBLE_PISO }], "observaciones": "Llegaron 3" }))
        .await
        .exito();
    assert_eq!(r.data()["estado"], "CORREGIDA");
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (3, 7));
}

/// PU-30-04 · La regla de la mitad usa lo que realmente llegó
/// Entrada: salida de 10 corregida a 6; el Vendedor intenta regresar 4 y luego 3
/// Esperado: 4 → 400 (máximo 3 = mitad de 6); 3 → 200
#[tokio::test]
async fn pu_30_04_mitad_de_lo_recibido() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 20).await;
    let folio = salida(&c, &p, 10).await;
    c.post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 6, "id_mueble": MUEBLE_PISO }] }))
        .await
        .exito();
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 4 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "como máximo la mitad");
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 3 }] }))
        .await
        .exito();
}

/// PU-30-05 · Solo una recepción por folio
/// Entrada: salida ya recibida; se intenta recibir otra vez
/// Esperado: 409 "ya fue recibida"
#[tokio::test]
async fn pu_30_05_una_sola_vez() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let folio = salida(&c, &p, 2).await;
    let cuerpo = json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 2, "id_mueble": MUEBLE_PISO }] });
    c.post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), cuerpo.clone()).await.exito();
    c.post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), cuerpo)
        .await
        .error(StatusCode::CONFLICT, "ya fue recibida");
}

/// PU-30-06 · Mueble que no es de piso y partidas incompletas
/// Entrada: recepción en el mueble de bodega B-900; recepción sin la partida del libro; folio que no existe
/// Esperado: 400 "no es un mueble del piso de venta"; 400 "Falta capturar lo recibido"; 404 "No existe la requisición"
#[tokio::test]
async fn pu_30_06_recepcion_invalida() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let folio = salida(&c, &p, 2).await;
    c.post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 2, "id_mueble": MUEBLE_BODEGA }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "no es un mueble del piso de venta");
    c.post(&v, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), json!({ "items": [] }))
        .await
        .error(StatusCode::BAD_REQUEST, "Falta capturar lo recibido");
    c.post(&v, "/api/vendedor/requisiciones/REQ-2000-1/recepcion", json!({ "items": [] }))
        .await
        .error(StatusCode::NOT_FOUND, "No existe la requisición");
}

/// PU-30-07 · Solo se reciben salidas y solo las recibe el Vendedor
/// Entrada: recibir una requisición de ENTRADA; Bodega intenta recibir una salida
/// Esperado: 400 "Solo se reciben en piso las requisiciones de salida"; 403 para Bodega
#[tokio::test]
async fn pu_30_07_solo_salidas_y_solo_vendedor() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 5, 10).await;
    let entrada = c
        .post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .exito()
        .data()["folio"]
        .as_str()
        .unwrap()
        .to_string();
    c.post(&v, &format!("/api/vendedor/requisiciones/{entrada}/recepcion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 1, "id_mueble": MUEBLE_PISO }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "Solo se reciben en piso las requisiciones de salida");
    let folio = salida(&c, &p, 1).await;
    let b = c.token(BODEGA).await;
    let r = c
        .post(&b, &format!("/api/vendedor/requisiciones/{folio}/recepcion"), json!({ "items": [{ "codigo_ean": p.ean, "cantidad_recibida": 1, "id_mueble": MUEBLE_PISO }] }))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}
