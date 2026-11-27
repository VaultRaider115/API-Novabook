//! CU-02 · CRUD de Usuarios (RF01). Actor: Gerente (crea, consulta, modifica,
//! da de baja y asigna rol).

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn nuevo_usuario(id: i64, rol: i32) -> Value {
    json!({
        "id_usuarios": id, "id_roles": rol, "nombre": "Laura", "apellido_paterno": "Prueba",
        "apellido_materno": "Unitaria", "telefono": "8441234567", "contrasena": "clave123"
    })
}

fn id_nuevo() -> i64 {
    71_000_000 + unico()
}

/// PU-02-01 · El Gerente da de alta un usuario
/// Entrada: id nuevo, rol Vendedor (3), nombre "Laura Prueba Unitaria", teléfono 8441234567, contraseña "clave123"
/// Esperado: 200 con el usuario creado (rol vendedor, activo); aparece en la lista y puede iniciar sesión
#[tokio::test]
async fn pu_02_01_alta_de_usuario() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let id = id_nuevo();
    let r = c.post(&g, "/api/usuarios", nuevo_usuario(id, 3)).await.exito();
    assert_eq!(r.data()["id_usuarios"], id);
    assert_eq!(r.data()["id_roles"], 3);
    assert_eq!(r.data()["estatus"], true);
    let lista = c.get(&g, "/api/usuarios").await.exito();
    assert!(lista.data().as_array().unwrap().iter().any(|u| u["id_usuarios"] == id));
    c.login((id, "clave123")).await.exito();
}

/// PU-02-02 · ID de usuario repetido
/// Entrada: alta con el ID 628777129 (ya es del Vendedor de la semilla)
/// Esperado: 409 "El usuario con ID 628777129 ya se encuentra registrado"
#[tokio::test]
async fn pu_02_02_id_repetido() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    c.post(&g, "/api/usuarios", nuevo_usuario(VENDEDOR.0, 3))
        .await
        .error(StatusCode::CONFLICT, "ya se encuentra registrado");
}

/// PU-02-03 · Teléfono inválido
/// Entrada: teléfono de 9 dígitos ("844123456") y teléfono que empieza con 0 ("0441234567")
/// Esperado: 400 "debe tener exactamente 10 dígitos" y 400 "no puede empezar con 0"
#[tokio::test]
async fn pu_02_03_telefono_invalido() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let mut u = nuevo_usuario(id_nuevo(), 3);
    u["telefono"] = json!("844123456");
    c.post(&g, "/api/usuarios", u).await.error(StatusCode::BAD_REQUEST, "10 dígitos");
    let mut u = nuevo_usuario(id_nuevo(), 3);
    u["telefono"] = json!("0441234567");
    c.post(&g, "/api/usuarios", u).await.error(StatusCode::BAD_REQUEST, "no puede empezar con 0");
}

/// PU-02-04 · Nombre con números
/// Entrada: nombre "Laura2"
/// Esperado: 400 "El nombre no puede contener números"
#[tokio::test]
async fn pu_02_04_nombre_con_numeros() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let mut u = nuevo_usuario(id_nuevo(), 3);
    u["nombre"] = json!("Laura2");
    c.post(&g, "/api/usuarios", u).await.error(StatusCode::BAD_REQUEST, "no puede contener números");
}

/// PU-02-05 · Contraseña vacía o de más de 16 caracteres
/// Entrada: contraseña "" y contraseña de 17 caracteres
/// Esperado: 400 "no puede estar vacía" y 400 "no puede exceder 16 caracteres"
#[tokio::test]
async fn pu_02_05_contrasena_invalida() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let mut u = nuevo_usuario(id_nuevo(), 3);
    u["contrasena"] = json!("");
    c.post(&g, "/api/usuarios", u).await.error(StatusCode::BAD_REQUEST, "no puede estar vacía");
    let mut u = nuevo_usuario(id_nuevo(), 3);
    u["contrasena"] = json!("a".repeat(17));
    c.post(&g, "/api/usuarios", u).await.error(StatusCode::BAD_REQUEST, "no puede exceder 16");
}

/// PU-02-06 · Rol inexistente
/// Entrada: alta con id_roles = 9
/// Esperado: 400 "Rol inválido"
#[tokio::test]
async fn pu_02_06_rol_invalido() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    c.post(&g, "/api/usuarios", nuevo_usuario(id_nuevo(), 9))
        .await
        .error(StatusCode::BAD_REQUEST, "Rol inválido");
}

/// PU-02-07 · El Gerente modifica datos y asigna rol
/// Entrada: usuario Vendedor nuevo; PUT con nombre "Mariana", teléfono 8449876543 y rol Bodega (2)
/// Esperado: 200; la consulta del usuario devuelve el nombre, el teléfono y el rol nuevos
#[tokio::test]
async fn pu_02_07_modificar_y_asignar_rol() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let id = id_nuevo();
    c.post(&g, "/api/usuarios", nuevo_usuario(id, 3)).await.exito();
    c.put(&g, &format!("/api/usuarios/{id}"), json!({ "nombre": "Mariana", "telefono": "8449876543", "id_roles": 2 }))
        .await
        .exito();
    let r = c.get(&g, &format!("/api/usuarios/{id}")).await.exito();
    assert_eq!(r.data()["nombre"], "Mariana");
    assert_eq!(r.data()["telefono"], 8449876543_i64);
    assert_eq!(r.data()["id_roles"], 2);
}

/// PU-02-08 · Baja de un usuario
/// Entrada: DELETE de un usuario activo; después, DELETE otra vez
/// Esperado: 200 y el usuario queda inactivo (estatus false); la segunda baja responde 409 "ya se encuentra inactivo"
#[tokio::test]
async fn pu_02_08_baja_de_usuario() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let id = id_nuevo();
    c.post(&g, "/api/usuarios", nuevo_usuario(id, 3)).await.exito();
    c.delete(&g, &format!("/api/usuarios/{id}")).await.exito();
    let r = c.get(&g, &format!("/api/usuarios/{id}")).await.exito();
    assert_eq!(r.data()["estatus"], false);
    c.delete(&g, &format!("/api/usuarios/{id}"))
        .await
        .error(StatusCode::CONFLICT, "ya se encuentra inactivo");
}

/// PU-02-09 · Nadie se da de baja a sí mismo
/// Entrada: el Gerente 1001 hace DELETE de su propio usuario
/// Esperado: 400 "No puedes eliminar tu propio usuario" (sigue activo)
#[tokio::test]
async fn pu_02_09_no_baja_propia() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    c.delete(&g, &format!("/api/usuarios/{}", GERENTE.0))
        .await
        .error(StatusCode::BAD_REQUEST, "No puedes eliminar tu propio usuario");
    c.login(GERENTE).await.exito();
}

/// PU-02-10 · Usuario que no existe
/// Entrada: GET, PUT y DELETE del usuario 999999
/// Esperado: 404 "no encontrado" en las tres
#[tokio::test]
async fn pu_02_10_usuario_inexistente() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    c.get(&g, "/api/usuarios/999999").await.error(StatusCode::NOT_FOUND, "no encontrado");
    c.put(&g, "/api/usuarios/999999", json!({ "nombre": "Nadie" }))
        .await
        .error(StatusCode::NOT_FOUND, "no encontrado");
    c.delete(&g, "/api/usuarios/999999").await.error(StatusCode::NOT_FOUND, "no encontrado");
}

/// PU-02-11 · Solo Gerente y Jefe entran a Usuarios
/// Entrada: el Vendedor y Bodega consultan GET /api/usuarios
/// Esperado: 403 para ambos
#[tokio::test]
async fn pu_02_11_otros_roles_sin_acceso() {
    let c = ctx().await;
    for usuario in [VENDEDOR, BODEGA] {
        let t = c.token(usuario).await;
        let r = c.get(&t, "/api/usuarios").await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{}: {}", usuario.0, r.json);
    }
}
