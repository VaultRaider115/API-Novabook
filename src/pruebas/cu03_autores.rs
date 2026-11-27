//! CU-03 · Catálogo de Autores (RF18). Escribe el Jefe; los demás consultan.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn autor(nombre: &str) -> Value {
    json!({ "nombre": nombre, "apellidos": "Prueba Unitaria", "nacionalidad": "Mexicana" })
}

/// PU-03-01 · El Jefe registra un autor
/// Entrada: nombre único (solo letras), apellidos "Prueba Unitaria", nacionalidad "Mexicana"
/// Esperado: 200 con el autor y su id; aparece en la lista de autores
#[tokio::test]
async fn pu_03_01_alta_de_autor() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre_letras("Autor");
    let r = c.post(&j, "/api/autores", autor(&n)).await.exito();
    let id = r.data()["id_autor"].as_i64().unwrap();
    let lista = c.get(&j, "/api/autores").await.exito();
    assert!(lista.data().as_array().unwrap().iter().any(|a| a["id_autor"] == id && a["nombre"] == n.as_str()));
}

/// PU-03-02 · Autor repetido
/// Entrada: el mismo nombre y apellidos dos veces (la segunda en mayúsculas)
/// Esperado: 409 "Ya existe el autor"
#[tokio::test]
async fn pu_03_02_autor_repetido() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre_letras("Autor");
    c.post(&j, "/api/autores", autor(&n)).await.exito();
    c.post(&j, "/api/autores", autor(&n.to_uppercase()))
        .await
        .error(StatusCode::CONFLICT, "Ya existe el autor");
}

/// PU-03-03 · Datos obligatorios y sin números
/// Entrada: nombre vacío; nombre "Autor7"
/// Esperado: 400 "es obligatorio"; 400 "no puede contener números"
#[tokio::test]
async fn pu_03_03_validaciones_de_nombre() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/autores", autor("")).await.error(StatusCode::BAD_REQUEST, "es obligatorio");
    c.post(&j, "/api/autores", autor("Autor7"))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede contener números");
}

/// PU-03-04 · El Jefe modifica un autor
/// Entrada: autor nuevo; PUT con apellidos "Corregido" y nacionalidad "Chilena"
/// Esperado: 200; la consulta del autor devuelve los datos nuevos
#[tokio::test]
async fn pu_03_04_modificar_autor() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre_letras("Autor");
    let id = c.post(&j, "/api/autores", autor(&n)).await.exito().data()["id_autor"].as_i64().unwrap();
    c.put(&j, &format!("/api/autores/{id}"), json!({ "nombre": n, "apellidos": "Corregido", "nacionalidad": "Chilena", "estado": true }))
        .await
        .exito();
    let r = c.get(&j, &format!("/api/autores/{id}")).await.exito();
    assert_eq!(r.data()["apellidos"], "Corregido");
    assert_eq!(r.data()["nacionalidad"], "Chilena");
}

/// PU-03-05 · Eliminar autor sin libros
/// Entrada: autor nuevo sin libros; DELETE
/// Esperado: 200 y la consulta posterior responde 404
#[tokio::test]
async fn pu_03_05_eliminar_autor_sin_libros() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let id = c.post(&j, "/api/autores", autor(&nombre_letras("Autor"))).await.exito().data()["id_autor"]
        .as_i64()
        .unwrap();
    c.delete(&j, &format!("/api/autores/{id}")).await.exito();
    c.get(&j, &format!("/api/autores/{id}")).await.error(StatusCode::NOT_FOUND, "no encontrado");
}

/// PU-03-06 · No se elimina un autor con libros
/// Entrada: DELETE del autor 2 (Gabriel García Márquez, tiene libros)
/// Esperado: 400 "tiene … título(s) asociado(s)" y el autor sigue existiendo
#[tokio::test]
async fn pu_03_06_no_eliminar_autor_con_libros() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.delete(&j, "/api/autores/2").await.error(StatusCode::BAD_REQUEST, "título(s) asociado(s)");
    c.get(&j, "/api/autores/2").await.exito();
}

/// PU-03-07 · Solo el Jefe escribe
/// Entrada: Bodega intenta POST /api/autores; Bodega consulta GET /api/autores
/// Esperado: 403 en el alta; 200 en la consulta
#[tokio::test]
async fn pu_03_07_solo_el_jefe_escribe() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let r = c.post(&b, "/api/autores", autor(&nombre_letras("Autor"))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
    c.get(&b, "/api/autores").await.exito();
}
