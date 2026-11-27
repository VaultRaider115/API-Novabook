//! CU-24 · Revisar inventario mensual (RF24). Actor: Jefe.
//! Aprobar ajusta el piso con la diferencia (sin pasar del stock actual);
//! rechazar exige motivo.

use axum::http::StatusCode;
use serde_json::json;

use super::cu23_levantar_inventario::{abrir, estado};
use super::*;

/// PU-24-01 · Aprobar ajusta el piso
/// Entrada: libro A con 10 en piso contado en 8; libro B con 3 en piso contado en 5; cerrar y aprobar
/// Esperado: 200, estado APROBADO; piso de A queda en 8 y el de B en 5
#[tokio::test]
async fn pu_24_01_aprobar_ajusta_piso() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let a = c.libro(PLANETA, 200.0, 10, 0).await;
    let b = c.libro(PLANETA, 200.0, 3, 0).await;
    let id = abrir(&c, &v).await;
    for (p, n) in [(&a, 8), (&b, 5)] {
        c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": n }))
            .await
            .exito();
    }
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
    let r = c.put(&j, &format!("/api/jefe/inventarios/{id}/revisar"), json!({ "aprobar": true })).await.exito();
    assert_eq!(estado(&r), "APROBADO");
    assert_eq!((c.piso(a.ean).await, c.piso(b.ean).await), (8, 5));
}

/// PU-24-02 · Al aprobar no se descuenta más de lo que hay
/// Entrada: libro con 5 en piso contado en 1; tras cerrar se venden 3 (quedan 2); se aprueba
/// Esperado: el piso queda en 0 (se descuentan solo las 2 que había, no 4)
#[tokio::test]
async fn pu_24_02_sin_pasar_del_stock_actual() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 5, 0).await;
    let id = abrir(&c, &v).await;
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": 1 }))
        .await
        .exito();
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
    c.post(&v, "/api/vendedor/ventas", json!({ "cliente": "Ana", "metodo_pago": "EFECTIVO", "items": [{ "codigo_ean": p.ean, "cantidad": 3 }] }))
        .await
        .exito();
    c.put(&j, &format!("/api/jefe/inventarios/{id}/revisar"), json!({ "aprobar": true })).await.exito();
    assert_eq!(c.piso(p.ean).await, 0);
}

/// PU-24-03 · Rechazo con motivo
/// Entrada: inventario cerrado; rechazar sin observaciones y luego con "Recontar el estante P-900"
/// Esperado: 400 "Indica por qué se rechaza"; luego 200, estado RECHAZADO y el piso no cambia
#[tokio::test]
async fn pu_24_03_rechazo_con_motivo() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 6, 0).await;
    let id = abrir(&c, &v).await;
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": 2 }))
        .await
        .exito();
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
    c.put(&j, &format!("/api/jefe/inventarios/{id}/revisar"), json!({ "aprobar": false }))
        .await
        .error(StatusCode::BAD_REQUEST, "Indica por qué se rechaza");
    let r = c
        .put(&j, &format!("/api/jefe/inventarios/{id}/revisar"), json!({ "aprobar": false, "observaciones": "Recontar el estante P-900" }))
        .await
        .exito();
    assert_eq!(estado(&r), "RECHAZADO");
    assert_eq!(c.piso(p.ean).await, 6);
}

/// PU-24-04 · Solo se revisa un inventario cerrado
/// Entrada: inventario ABIERTO; el Jefe intenta aprobarlo
/// Esperado: 409 "Solo se revisa un inventario CERRADO"
#[tokio::test]
async fn pu_24_04_solo_cerrados() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let id = abrir(&c, &v).await;
    c.put(&j, &format!("/api/jefe/inventarios/{id}/revisar"), json!({ "aprobar": true }))
        .await
        .error(StatusCode::CONFLICT, "Solo se revisa un inventario CERRADO");
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
}

/// PU-24-05 · Lista e inventario inexistente
/// Entrada: GET /api/jefe/inventarios; GET /api/jefe/inventarios/999999
/// Esperado: 200 con la lista; 404 "no existe"
#[tokio::test]
async fn pu_24_05_consultas() {
    let c = ctx_exclusivo().await;
    let j = c.token(JEFE).await;
    assert!(c.get(&j, "/api/jefe/inventarios").await.exito().data().is_array());
    c.get(&j, "/api/jefe/inventarios/999999").await.error(StatusCode::NOT_FOUND, "no existe");
}

/// PU-24-06 · Solo el Jefe revisa
/// Entrada: el Vendedor hace PUT /api/jefe/inventarios/{id}/revisar
/// Esperado: 403
#[tokio::test]
async fn pu_24_06_solo_el_jefe() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let r = c.put(&v, "/api/jefe/inventarios/1/revisar", json!({ "aprobar": true })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}
