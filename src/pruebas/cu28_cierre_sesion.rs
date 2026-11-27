//! CU-28 · Cierre de sesión automático (RF28). La API responde 401 si falta
//! el token, si es inválido o vencido, o si el usuario fue dado de baja; con
//! ese 401 la app regresa al login (la inactividad la mide el cliente Qt).

use axum::http::{Method, StatusCode};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde_json::Value;

use super::*;

/// PU-28-01 · Petición sin token
/// Entrada: GET /api/auth/perfil sin cabecera Authorization
/// Esperado: 401 "Cabecera Authorization no proporcionada"
#[tokio::test]
async fn pu_28_01_sin_token() {
    let c = ctx().await;
    c.pedir(Method::GET, "/api/auth/perfil", None, None)
        .await
        .error(StatusCode::UNAUTHORIZED, "Authorization no proporcionada");
}

/// PU-28-02 · Token alterado o con firma de otra clave
/// Entrada: token válido con un carácter cambiado; token firmado con otra clave
/// Esperado: 401 en ambos
#[tokio::test]
async fn pu_28_02_token_invalido() {
    let c = ctx().await;
    let t = c.token(VENDEDOR).await;
    let mut alterado = t.clone();
    let ultimo = alterado.pop().unwrap();
    alterado.push(if ultimo == 'A' { 'B' } else { 'A' });
    assert_eq!(c.get(&alterado, "/api/auth/perfil").await.status, StatusCode::UNAUTHORIZED);
    let claims = leer_claims(&t);
    let ajeno = encode(&Header::default(), &claims, &EncodingKey::from_secret(b"otra-clave")).unwrap();
    assert_eq!(c.get(&ajeno, "/api/auth/perfil").await.status, StatusCode::UNAUTHORIZED);
}

fn leer_claims(token: &str) -> Value {
    let mut v = Validation::default();
    v.validate_exp = false;
    decode::<Value>(token, &DecodingKey::from_secret(SECRETO_JWT.as_bytes()), &v).unwrap().claims
}

/// PU-28-03 · Token vencido
/// Entrada: los mismos datos de un token válido pero con vencimiento de hace una hora
/// Esperado: 401 (la app regresa al login)
#[tokio::test]
async fn pu_28_03_token_vencido() {
    let c = ctx().await;
    let t = c.token(VENDEDOR).await;
    let mut claims = leer_claims(&t);
    let ahora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    claims["exp"] = Value::from(ahora - 3600);
    let vencido = encode(&Header::default(), &claims, &EncodingKey::from_secret(SECRETO_JWT.as_bytes())).unwrap();
    assert_eq!(c.get(&vencido, "/api/auth/perfil").await.status, StatusCode::UNAUTHORIZED);
}

/// PU-28-04 · Baja inmediata con la sesión abierta
/// Entrada: un vendedor nuevo inicia sesión y consulta su perfil; el Gerente lo da de baja; vuelve a consultar con el mismo token
/// Esperado: primero 200; después 401 "Usuario inactivo o inexistente"
#[tokio::test]
async fn pu_28_04_baja_inmediata() {
    let c = ctx().await;
    let g = c.token(GERENTE).await;
    let u = c.usuario_nuevo(3).await;
    let t = c.token(u).await;
    c.get(&t, "/api/auth/perfil").await.exito();
    c.delete(&g, &format!("/api/usuarios/{}", u.0)).await.exito();
    c.get(&t, "/api/vendedor/existencias")
        .await
        .error(StatusCode::UNAUTHORIZED, "Usuario inactivo o inexistente");
}

/// PU-28-05 · Formato de la cabecera
/// Entrada: cabecera "Authorization: Token abc" (sin "Bearer")
/// Esperado: 401 "Se espera: Bearer <token>"
#[tokio::test]
async fn pu_28_05_formato_de_cabecera() {
    let c = ctx().await;
    let req = axum::http::Request::builder()
        .uri("/api/auth/perfil")
        .header(axum::http::header::AUTHORIZATION, "Token abc")
        .body(axum::body::Body::empty())
        .unwrap();
    use tower::ServiceExt;
    let resp = c.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("Bearer"));
}

/// PU-28-06 · Sesión válida
/// Entrada: GET /api/auth/perfil con el token del Jefe
/// Esperado: 200 con el id y el nombre del Jefe
#[tokio::test]
async fn pu_28_06_sesion_valida() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.get(&j, "/api/auth/perfil").await.exito();
    assert_eq!(r.data()["id_usuarios"], JEFE.0);
}
