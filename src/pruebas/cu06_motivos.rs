//! CU-06 · Motivos de devolución (RF20). Escribe el Jefe; los demás consultan.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

async fn crear(c: &Ctx, t: &str, n: &str) -> i64 {
    c.post(t, "/api/motivos", json!({ "nombre_motivo": n, "descripcion": "Motivo de prueba" }))
        .await
        .exito()
        .data()["id_motivo"]
        .as_i64()
        .unwrap()
}

/// PU-06-01 · El Jefe registra un motivo
/// Entrada: nombre único (solo letras) y descripción "Motivo de prueba"
/// Esperado: 200 con el id; aparece en la lista con 0 devoluciones
#[tokio::test]
async fn pu_06_01_alta_de_motivo() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre_letras("Motivo");
    let id = crear(&c, &j, &n).await;
    let lista = c.get(&j, "/api/motivos").await.exito();
    let m = lista.data().as_array().unwrap().iter().find(|m| m["id_motivo"] == id).cloned().unwrap();
    assert_eq!(m["motivo"], n.as_str());
    assert_eq!(m["total_devoluciones"], 0);
}

/// PU-06-02 · Motivo repetido
/// Entrada: el nombre "Reabastecimiento" (ya existe en la semilla) en mayúsculas
/// Esperado: 409 "Ya existe un motivo registrado con ese nombre"
#[tokio::test]
async fn pu_06_02_motivo_repetido() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/motivos", json!({ "nombre_motivo": "REABASTECIMIENTO" }))
        .await
        .error(StatusCode::CONFLICT, "Ya existe un motivo");
}

/// PU-06-03 · Nombre obligatorio
/// Entrada: nombre vacío y nombre de solo espacios
/// Esperado: 400 "El motivo es obligatorio" en ambos
#[tokio::test]
async fn pu_06_03_nombre_obligatorio() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    for n in ["", "   "] {
        c.post(&j, "/api/motivos", json!({ "nombre_motivo": n }))
            .await
            .error(StatusCode::BAD_REQUEST, "obligatorio");
    }
}

/// PU-06-04 · Límite de caracteres
/// Entrada: nombre de 51 caracteres (límite 50) y descripción de 201 (límite 200)
/// Esperado: 400 "no puede pasar de 50 caracteres" y 400 "no puede pasar de 200 caracteres"
#[tokio::test]
async fn pu_06_04_limite_de_caracteres() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/motivos", json!({ "nombre_motivo": "a".repeat(51) }))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 50 caracteres");
    c.post(&j, "/api/motivos", json!({ "nombre_motivo": nombre_letras("Motivo"), "descripcion": "d".repeat(201) }))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 200 caracteres");
}

/// PU-06-05 · Modificar un motivo
/// Entrada: motivo nuevo; PUT con otro nombre y descripción
/// Esperado: 200 con los datos nuevos
#[tokio::test]
async fn pu_06_05_modificar_motivo() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let id = crear(&c, &j, &nombre_letras("Motivo")).await;
    let n = nombre_letras("Motivo");
    let r = c.put(&j, &format!("/api/motivos/{id}"), json!({ "nombre_motivo": n, "descripcion": "Cambiado" }))
        .await
        .exito();
    assert_eq!(r.data()["motivo"], n.as_str());
    assert_eq!(r.data()["descripcion"], "Cambiado");
}

/// PU-06-06 · Eliminar motivo sin uso y no eliminar uno usado
/// Entrada: DELETE de un motivo nuevo; DELETE de un motivo usado en una devolución a proveedor
/// Esperado: 200 en el nuevo; 400 "ya está registrado en 1 devolución(es)" en el usado
#[tokio::test]
async fn pu_06_06_eliminar_motivo() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let libre = crear(&c, &j, &nombre_letras("Motivo")).await;
    c.delete(&j, &format!("/api/motivos/{libre}")).await.exito();

    let usado = crear(&c, &j, &nombre_letras("Motivo")).await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    c.post(&v, "/api/vendedor/devoluciones", json!({
        "id_proveedor": PLANETA, "tipo_producto": "libro",
        "items": [{ "codigo_ean": p.ean, "cantidad": 1, "id_motivo": usado }]
    }))
    .await
    .exito();
    c.delete(&j, &format!("/api/motivos/{usado}"))
        .await
        .error(StatusCode::BAD_REQUEST, "1 devolución(es)");
}

/// PU-06-07 · Solo el Jefe escribe
/// Entrada: el Vendedor intenta POST /api/motivos y consulta GET /api/motivos
/// Esperado: 403 en el alta; 200 en la consulta
#[tokio::test]
async fn pu_06_07_solo_el_jefe_escribe() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let r = c.post(&v, "/api/motivos", json!({ "nombre_motivo": nombre_letras("Motivo") })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
    c.get(&v, "/api/motivos").await.exito();
}
