//! CU-01 · Login (RF15, Inicio de Sesión). Actores: los cuatro roles.

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::*;

/// PU-01-01 · Inicio de sesión correcto de cada rol
/// Entrada: los 4 usuarios de prueba con su contraseña (Gerente, Jefe, Bodega, Vendedor)
/// Esperado: 200 con token JWT, tipo Bearer y el rol correcto de cada usuario
#[tokio::test]
async fn pu_01_01_login_correcto_de_cada_rol() {
    let c = ctx().await;
    for (usuario, rol) in [(GERENTE, "gerente"), (JEFE, "jefe"), (BODEGA, "bodega"), (VENDEDOR, "vendedor")] {
        let r = c.login(usuario).await.exito();
        assert!(r.data()["token"].as_str().unwrap().split('.').count() == 3, "el token no es un JWT");
        assert_eq!(r.data()["token_type"], "Bearer");
        let rol_nombre = r.data()["usuario"]["rol_nombre"].as_str().unwrap().to_lowercase();
        assert!(rol_nombre.contains(rol), "rol {rol_nombre} para el usuario {}", usuario.0);
    }
}

/// PU-01-02 · Contraseña incorrecta
/// Entrada: usuario 1001 con contraseña "12345"
/// Esperado: 401 "Credenciales incorrectas" y sin token
#[tokio::test]
async fn pu_01_02_contrasena_incorrecta() {
    let c = ctx().await;
    let r = c.login((GERENTE.0, "12345")).await.error(StatusCode::UNAUTHORIZED, "Credenciales incorrectas");
    assert!(r.data()["token"].is_null());
}

/// PU-01-03 · Usuario que no existe
/// Entrada: usuario 999999 con cualquier contraseña
/// Esperado: 401 con el mismo mensaje genérico (no revela si el usuario existe)
#[tokio::test]
async fn pu_01_03_usuario_inexistente() {
    let c = ctx().await;
    c.login((999_999, "admin123")).await.error(StatusCode::UNAUTHORIZED, "Credenciales incorrectas");
}

/// PU-01-04 · Contraseña vacía
/// Entrada: usuario 1001 con contraseña ""
/// Esperado: rechazo (4xx) sin token
#[tokio::test]
async fn pu_01_04_contrasena_vacia() {
    let c = ctx().await;
    let r = c.login((GERENTE.0, "")).await;
    assert!(r.status.is_client_error(), "respondió {}: {}", r.status, r.json);
    assert!(r.data()["token"].is_null());
}

/// PU-01-05 · Cuerpo incompleto
/// Entrada: JSON sin el campo "contrasena"
/// Esperado: 4xx (la petición no llega al servicio)
#[tokio::test]
async fn pu_01_05_cuerpo_incompleto() {
    let c = ctx().await;
    let r = c.pedir(Method::POST, "/api/auth/login", None, Some(json!({ "id_usuario": 1001 }))).await;
    assert!(r.status.is_client_error(), "respondió {}", r.status);
}

/// PU-01-06 · Usuario dado de baja no inicia sesión
/// Entrada: usuario nuevo de Vendedor dado de baja por el Gerente, luego login con su contraseña
/// Esperado: 401; antes de la baja el mismo login sí funcionaba
#[tokio::test]
async fn pu_01_06_usuario_dado_de_baja() {
    let c = ctx().await;
    let gerente = c.token(GERENTE).await;
    let id = 70_000_000 + unico();
    c.post(&gerente, "/api/usuarios", json!({
        "id_usuarios": id, "id_roles": 3, "nombre": "Prueba", "apellido_paterno": "Baja",
        "contrasena": "clave123"
    }))
    .await
    .exito();
    c.login((id, "clave123")).await.exito();
    c.delete(&gerente, &format!("/api/usuarios/{id}")).await.exito();
    let r = c.login((id, "clave123")).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.json);
}

/// PU-01-07 · El acceso queda en la bitácora
/// Entrada: login correcto del Jefe y login fallido de un usuario
/// Esperado: la bitácora registra ambos eventos de acceso
#[tokio::test]
async fn pu_01_07_acceso_en_bitacora() {
    let c = ctx().await;
    c.login(JEFE).await.exito();
    let antes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bitacora WHERE id_tipo_evento = 2")
        .fetch_one(&c.pool)
        .await
        .unwrap();
    c.login(JEFE).await.exito();
    c.login((JEFE.0, "mala")).await;
    let despues: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bitacora WHERE id_tipo_evento = 2")
        .fetch_one(&c.pool)
        .await
        .unwrap();
    assert!(despues >= antes + 2, "antes {antes}, después {despues}");
}
