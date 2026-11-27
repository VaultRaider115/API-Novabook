//! CU-27 · Gestionar proveedores y editoriales (RF27). CRUD del Jefe; Bodega
//! consulta (la app ya no le muestra el alta; la API todavía acepta su POST).

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn proveedor(nombre: &str, rfc: &str) -> Value {
    json!({
        "nombre_proveedor": nombre, "rfc": rfc, "telefono": "5552274600", "correo": "ventas@prueba.mx",
        "direccion": "Av. Universidad 100, Saltillo", "persona_contacto": "Lic. Prueba"
    })
}

/// RFC único de 12 caracteres ("PRU" + 9 dígitos).
fn rfc() -> String {
    format!("PRU{:09}", 100_000_000 + unico())
}

/// PU-27-01 · El Jefe registra y modifica un proveedor
/// Entrada: proveedor nuevo con RFC, teléfono, correo, dirección y contacto; PUT con otro correo
/// Esperado: 200 en ambos; queda ACTIVO y la consulta trae el correo nuevo
#[tokio::test]
async fn pu_27_01_alta_y_cambio_de_proveedor() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.post(&j, "/api/proveedores", proveedor(&nombre("Proveedor"), &rfc())).await.exito();
    assert_eq!(r.data()["estatus"], "ACTIVO");
    let id = r.data()["id_proveedor"].as_i64().unwrap();
    let mut cambio = r.data().clone();
    cambio["correo"] = json!("compras@prueba.mx");
    c.put(&j, &format!("/api/proveedores/{id}"), cambio).await.exito();
    assert_eq!(c.get(&j, &format!("/api/proveedores/{id}")).await.exito().data()["correo"], "compras@prueba.mx");
}

/// PU-27-02 · Sin proveedores repetidos
/// Entrada: proveedor con el nombre "Planeta Mexico" (ya existe); proveedor con un RFC ya usado
/// Esperado: 409 "Ya existe un proveedor llamado"; 409 "Ya existe un proveedor con el RFC"
#[tokio::test]
async fn pu_27_02_sin_repetidos() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/proveedores", proveedor("Planeta Mexico", &rfc()))
        .await
        .error(StatusCode::CONFLICT, "Ya existe un proveedor llamado");
    let r = rfc();
    c.post(&j, "/api/proveedores", proveedor(&nombre("Proveedor"), &r)).await.exito();
    c.post(&j, "/api/proveedores", proveedor(&nombre("Proveedor"), &r))
        .await
        .error(StatusCode::CONFLICT, "Ya existe un proveedor con el RFC");
}

/// PU-27-03 · Datos obligatorios y límites
/// Entrada: nombre vacío; RFC vacío; RFC de 14 caracteres (límite 13)
/// Esperado: 400 "es obligatorio" (dos veces); 400 "no puede pasar de 13 caracteres"
#[tokio::test]
async fn pu_27_03_validaciones() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/proveedores", proveedor("", &rfc())).await.error(StatusCode::BAD_REQUEST, "es obligatorio");
    c.post(&j, "/api/proveedores", proveedor(&nombre("Proveedor"), "")).await.error(StatusCode::BAD_REQUEST, "es obligatorio");
    c.post(&j, "/api/proveedores", proveedor(&nombre("Proveedor"), "ABCDEFGHIJKLM1"))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 13 caracteres");
}

/// PU-27-04 · Bodega consulta pero no modifica
/// Entrada: Bodega hace GET /api/proveedores y PUT/DELETE de un proveedor
/// Esperado: 200 en la consulta; 403 en la modificación y la baja
#[tokio::test]
async fn pu_27_04_bodega_solo_consulta() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.get(&b, "/api/proveedores").await.exito();
    assert_eq!(c.put(&b, "/api/proveedores/1", proveedor("Planeta Mexico", "PME850101AB1")).await.status, StatusCode::FORBIDDEN);
    assert_eq!(c.delete(&b, "/api/proveedores/1").await.status, StatusCode::FORBIDDEN);
}

/// PU-27-05 · Editoriales: alta, repetida y baja
/// Entrada: editorial nueva; la misma otra vez; DELETE de la nueva
/// Esperado: 200; 409 "Ya existe la editorial"; 200 al eliminarla
#[tokio::test]
async fn pu_27_05_editoriales() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre("Editorial");
    let id = c.post(&j, "/api/editoriales", json!({ "nombre_editorial": n })).await.exito().data()["id_editorial"]
        .as_i64()
        .unwrap();
    c.post(&j, "/api/editoriales", json!({ "nombre_editorial": n }))
        .await
        .error(StatusCode::CONFLICT, "Ya existe la editorial");
    c.delete(&j, &format!("/api/editoriales/{id}")).await.exito();
}

/// PU-27-06 · No se elimina una editorial con revistas
/// Entrada: DELETE de la editorial 1 (Planeta Mexico Infantil, tiene revistas)
/// Esperado: 409 "tiene revistas asociadas"
#[tokio::test]
async fn pu_27_06_editorial_con_revistas() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.revista(PLANETA_INFANTIL, 90.0, 0, 0).await;
    c.delete(&j, "/api/editoriales/1").await.error(StatusCode::CONFLICT, "tiene revistas asociadas");
}

/// PU-27-07 · El Gerente y el Vendedor no administran proveedores
/// Entrada: el Gerente y el Vendedor hacen POST /api/proveedores
/// Esperado: 403 para ambos
#[tokio::test]
async fn pu_27_07_otros_roles() {
    let c = ctx().await;
    for usuario in [GERENTE, VENDEDOR] {
        let t = c.token(usuario).await;
        assert_eq!(c.post(&t, "/api/proveedores", proveedor(&nombre("Proveedor"), &rfc())).await.status, StatusCode::FORBIDDEN);
    }
}
