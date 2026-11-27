//! CU-25 · Registrar solicitudes de editorial (RF25). Actor: Jefe.
//! Títulos que una editorial pide de vuelta; una editorial por solicitud.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// Registra una solicitud y devuelve su id (la API responde con la lista de
/// solicitudes; se busca por la referencia, que es única).
async fn registrar(c: &Ctx, j: &str, proveedor: i32, eans: Vec<i64>) -> (i64, serde_json::Value) {
    let referencia = nombre("Correo de la editorial");
    let r = c
        .post(j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": proveedor, "referencia": referencia, "eans": eans }))
        .await
        .exito();
    let s = r
        .data()
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["referencia"] == referencia.as_str())
        .cloned()
        .expect("la solicitud no aparece en la respuesta");
    (s["id_solicitud"].as_i64().unwrap(), s)
}

/// PU-25-01 · Registrar una solicitud
/// Entrada: proveedor Planeta, referencia del correo, dos libros de Planeta (A con 2 en piso y 3 en bodega; B con 4 en bodega)
/// Esperado: 200; la solicitud queda ABIERTA con los dos títulos y sus piezas por regresar (A 5, B 4)
#[tokio::test]
async fn pu_25_01_registrar() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let a = c.libro(PLANETA, 200.0, 2, 3).await;
    let b = c.libro(PLANETA, 200.0, 0, 4).await;
    let (id, _) = registrar(&c, &j, PLANETA, vec![a.ean, b.ean]).await;
    let lista = c.get(&j, "/api/jefe/solicitudes-editorial").await.exito();
    let s = lista.data().as_array().unwrap().iter().find(|s| s["id_solicitud"] == id).cloned().expect("no aparece");
    assert_eq!(s["estado"], "ABIERTA");
    let titulos = s["items"].as_array().expect("sin títulos");
    let piezas = |ean: i64| titulos.iter().find(|t| t["codigo_ean"] == ean).map(|t| t["piezas"].clone());
    assert_eq!((piezas(a.ean), piezas(b.ean)), (Some(json!(5)), Some(json!(4))));
}

/// PU-25-02 · Una editorial por solicitud
/// Entrada: solicitud de Planeta con un libro de Penguin
/// Esperado: 400 "no es del proveedor de la solicitud"
#[tokio::test]
async fn pu_25_02_una_editorial() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let ajeno = c.libro(PENGUIN, 200.0, 1, 1).await;
    c.post(&j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PLANETA, "referencia": "Correo", "eans": [ajeno.ean] }))
        .await
        .error(StatusCode::BAD_REQUEST, "no es del proveedor de la solicitud");
}

/// PU-25-03 · Datos obligatorios
/// Entrada: sin referencia; sin EAN; EAN que no existe (123)
/// Esperado: 400 "Indica la referencia del correo"; 400 "al menos un EAN"; 404 "no existe"
#[tokio::test]
async fn pu_25_03_validaciones() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 1, 1).await;
    c.post(&j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PLANETA, "referencia": " ", "eans": [p.ean] }))
        .await
        .error(StatusCode::BAD_REQUEST, "Indica la referencia del correo");
    c.post(&j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PLANETA, "referencia": "Correo", "eans": [] }))
        .await
        .error(StatusCode::BAD_REQUEST, "al menos un EAN");
    c.post(&j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PLANETA, "referencia": "Correo", "eans": [123] }))
        .await
        .error(StatusCode::NOT_FOUND, "no existe");
}

/// PU-25-04 · Marcar atendida
/// Entrada: solicitud ABIERTA; PUT .../atender dos veces
/// Esperado: la primera 200 (ATENDIDA); la segunda 409 "no existe o ya fue atendida"
#[tokio::test]
async fn pu_25_04_atender() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 1, 1).await;
    let (id, _) = registrar(&c, &j, PLANETA, vec![p.ean]).await;
    c.put(&j, &format!("/api/jefe/solicitudes-editorial/{id}/atender"), json!({})).await.exito();
    let lista = c.get(&j, "/api/jefe/solicitudes-editorial").await.exito();
    let s = lista.data().as_array().unwrap().iter().find(|s| s["id_solicitud"] == id).cloned().unwrap();
    assert_eq!(s["estado"], "ATENDIDA");
    c.put(&j, &format!("/api/jefe/solicitudes-editorial/{id}/atender"), json!({}))
        .await
        .error(StatusCode::CONFLICT, "ya fue atendida");
}

/// PU-25-05 · Solo el Jefe registra solicitudes
/// Entrada: el Vendedor hace POST /api/jefe/solicitudes-editorial
/// Esperado: 403
#[tokio::test]
async fn pu_25_05_solo_el_jefe() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let r = c.post(&v, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PLANETA, "referencia": "x", "eans": [] })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}
