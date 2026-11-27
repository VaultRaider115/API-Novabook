//! CU-13 · Movimiento Tienda → Bodega de libros (RF11). Actor: Vendedor.
//! Regla de la mitad: solo regresa hasta la mitad (redondeo hacia abajo) de la
//! última bajada bodega → piso, contando lo ya regresado; sin límite si nunca
//! se bajó. Aplica a la requisición de entrada y al traslado suelto.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// PU-13-01 · Retorno sin bajada previa
/// Entrada: libro con 8 en piso que nunca se bajó de bodega; traslado suelto de las 8 piezas a bodega
/// Esperado: 200; piso 0 y bodega 8 (sin límite)
#[tokio::test]
async fn pu_13_01_sin_bajada_previa() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 8, 0).await;
    c.post(&v, "/api/vendedor/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 8 }))
        .await
        .exito();
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (0, 8));
}

/// PU-13-02 · Regla de la mitad en la requisición de entrada
/// Entrada: Bodega baja 10; el Vendedor pide regresar 6, luego 5 y luego 1 más
/// Esperado: 6 → 400 "como máximo la mitad"; 5 → 200 con folio; 1 más → 400 (ya regresó la mitad)
#[tokio::test]
async fn pu_13_02_regla_de_la_mitad() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 20).await;
    c.post(&b, "/api/bodega/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 10 }] }))
        .await
        .exito();
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 6 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "como máximo la mitad");
    let r = c
        .post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 5 }] }))
        .await
        .exito();
    assert!(r.data()["folio"].as_str().unwrap().starts_with("REQ-"));
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "como máximo la mitad");
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (5, 15));
}

/// PU-13-03 · La mitad se redondea hacia abajo y aplica al traslado suelto
/// Entrada: Bodega baja 7 con traslado suelto; el Vendedor regresa 4 y luego 3 con traslado suelto
/// Esperado: 4 → 400 (máximo 3); 3 → 200
#[tokio::test]
async fn pu_13_03_mitad_hacia_abajo_en_traslado_suelto() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 7).await;
    c.post(&b, "/api/bodega/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 7 }))
        .await
        .exito();
    c.post(&v, "/api/vendedor/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 4 }))
        .await
        .error(StatusCode::BAD_REQUEST, "como máximo la mitad");
    c.post(&v, "/api/vendedor/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 3 }))
        .await
        .exito();
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (4, 3));
}

/// PU-13-04 · La requisición de entrada queda registrada
/// Entrada: requisición de entrada de 2 piezas de un libro que nunca se bajó
/// Esperado: 200; la consulta del folio la muestra como entrada (piso → bodega) y no PENDIENTE
#[tokio::test]
async fn pu_13_04_requisicion_de_entrada_registrada() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 6, 0).await;
    let r = c
        .post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 2 }] }))
        .await
        .exito();
    let folio = r.data()["folio"].as_str().unwrap();
    let det = c.get(&v, &format!("/api/vendedor/requisiciones/{folio}")).await.exito();
    assert_ne!(det.data()["estado"], "PENDIENTE", "{}", det.json);
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (4, 2));
}

/// PU-13-05 · Stock insuficiente en piso
/// Entrada: libro con 2 en piso; requisición de entrada de 3
/// Esperado: 400 "Stock insuficiente" y el piso sigue en 2
#[tokio::test]
async fn pu_13_05_stock_insuficiente_en_piso() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 2, 0).await;
    c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": p.ean, "cantidad": 3 }] }))
        .await
        .error(StatusCode::BAD_REQUEST, "Stock insuficiente");
    assert_eq!(c.piso(p.ean).await, 2);
}

/// PU-13-07 · Requisiciones simultáneas en sentidos opuestos (concurrencia)
/// Entrada: 10 rondas; en cada una, a la vez, Bodega baja 1 pieza de un libro y el Vendedor regresa 1 de otro
/// Esperado: ninguna petición responde 500; cada una termina en 200 (o en un error de negocio, nunca de base de datos)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pu_13_07_requisiciones_simultaneas() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let v = c.token(VENDEDOR).await;
    let mut errores = Vec::new();
    for _ in 0..10 {
        let baja = c.libro(PLANETA, 200.0, 0, 5).await;
        let sube = c.libro(PLANETA, 200.0, 5, 0).await;
        let (r1, r2) = tokio::join!(
            c.post(&b, "/api/bodega/requisiciones", json!({ "items": [{ "codigo_ean": baja.ean, "cantidad": 1 }] })),
            c.post(&v, "/api/vendedor/requisiciones", json!({ "items": [{ "codigo_ean": sube.ean, "cantidad": 1 }] })),
        );
        for r in [r1, r2] {
            if r.status.is_server_error() {
                errores.push(r.mensaje());
            }
        }
    }
    assert!(
        errores.is_empty(),
        "{} de 20 peticiones respondieron 500 (\"{}\"; en el log: MariaDB 1213, bloqueo mutuo)",
        errores.len(),
        errores[0]
    );
}

/// PU-13-06 · Solo el Vendedor regresa mercancía
/// Entrada: Bodega hace POST /api/vendedor/movimientos/libros
/// Esperado: 403
#[tokio::test]
async fn pu_13_06_solo_vendedor() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.libro(PLANETA, 200.0, 2, 0).await;
    let r = c.post(&b, "/api/vendedor/movimientos/libros", json!({ "codigo_ean": p.ean, "cantidad": 1 })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}
