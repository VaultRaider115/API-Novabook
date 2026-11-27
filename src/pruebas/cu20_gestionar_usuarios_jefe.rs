//! CU-20 · Gestionar usuarios como Jefe (RF16). El Jefe consulta, modifica
//! datos y da de baja, pero no crea usuarios, no asigna roles y no toca
//! cuentas de Gerente.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// PU-20-01 · El Jefe consulta la lista de usuarios
/// Entrada: GET /api/usuarios como Jefe
/// Esperado: 200 con los usuarios de la semilla (incluido el Gerente 1001)
#[tokio::test]
async fn pu_20_01_consultar() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.get(&j, "/api/usuarios").await.exito();
    assert!(r.data().as_array().unwrap().iter().any(|u| u["id_usuarios"] == GERENTE.0));
}

/// PU-20-02 · El Jefe no crea usuarios
/// Entrada: POST /api/usuarios como Jefe
/// Esperado: 403 "Solo el Gerente puede dar de alta usuarios"
#[tokio::test]
async fn pu_20_02_no_crea_usuarios() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/usuarios", json!({
        "id_usuarios": 73_000_000 + unico(), "id_roles": 3, "nombre": "Nuevo", "contrasena": "clave123"
    }))
    .await
    .error(StatusCode::FORBIDDEN, "Solo el Gerente puede dar de alta");
}

/// PU-20-03 · El Jefe modifica datos de un vendedor
/// Entrada: vendedor nuevo; PUT con nombre "Rodrigo" y teléfono 8447654321
/// Esperado: 200 y los datos quedan cambiados (el rol sigue igual)
#[tokio::test]
async fn pu_20_03_modifica_datos() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let (id, _) = c.usuario_nuevo(3).await;
    c.put(&j, &format!("/api/usuarios/{id}"), json!({ "nombre": "Rodrigo", "telefono": "8447654321" }))
        .await
        .exito();
    let r = c.get(&j, &format!("/api/usuarios/{id}")).await.exito();
    assert_eq!(r.data()["nombre"], "Rodrigo");
    assert_eq!(r.data()["id_roles"], 3);
}

/// PU-20-04 · El Jefe no asigna roles
/// Entrada: PUT con id_roles 2 sobre un vendedor
/// Esperado: 403 "Solo el Gerente puede asignar o cambiar roles" y el rol no cambia
#[tokio::test]
async fn pu_20_04_no_asigna_roles() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let (id, _) = c.usuario_nuevo(3).await;
    c.put(&j, &format!("/api/usuarios/{id}"), json!({ "id_roles": 2 }))
        .await
        .error(StatusCode::FORBIDDEN, "Solo el Gerente puede asignar o cambiar roles");
    assert_eq!(c.get(&j, &format!("/api/usuarios/{id}")).await.data()["id_roles"], 3);
}

/// PU-20-05 · El Jefe no toca cuentas de Gerente
/// Entrada: PUT y DELETE sobre el Gerente 1001
/// Esperado: 403 "Solo el Gerente puede modificar una cuenta de Gerente" y 403 "dar de baja una cuenta de Gerente"
#[tokio::test]
async fn pu_20_05_no_toca_gerentes() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.put(&j, &format!("/api/usuarios/{}", GERENTE.0), json!({ "nombre": "Otro" }))
        .await
        .error(StatusCode::FORBIDDEN, "cuenta de Gerente");
    c.delete(&j, &format!("/api/usuarios/{}", GERENTE.0))
        .await
        .error(StatusCode::FORBIDDEN, "cuenta de Gerente");
    c.login(GERENTE).await.exito();
}

/// PU-20-06 · El Jefe da de baja a un vendedor
/// Entrada: DELETE de un vendedor nuevo
/// Esperado: 200; el usuario queda inactivo y ya no inicia sesión
#[tokio::test]
async fn pu_20_06_baja_de_vendedor() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let u = c.usuario_nuevo(3).await;
    c.delete(&j, &format!("/api/usuarios/{}", u.0)).await.exito();
    assert_eq!(c.get(&j, &format!("/api/usuarios/{}", u.0)).await.data()["estatus"], false);
    assert_eq!(c.login(u).await.status, StatusCode::UNAUTHORIZED);
}

/// PU-20-07 · El Jefe no se cambia el rol ni se da de baja
/// Entrada: el Jefe hace PUT con estatus false sobre sí mismo y DELETE de su propio usuario
/// Esperado: 4xx en ambos y el Jefe sigue pudiendo iniciar sesión
#[tokio::test]
async fn pu_20_07_no_se_desactiva_a_si_mismo() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.put(&j, &format!("/api/usuarios/{}", JEFE.0), json!({ "estatus": false })).await;
    assert!(r.status.is_client_error(), "{}", r.json);
    c.delete(&j, &format!("/api/usuarios/{}", JEFE.0))
        .await
        .error(StatusCode::BAD_REQUEST, "No puedes eliminar tu propio usuario");
    c.login(JEFE).await.exito();
}
