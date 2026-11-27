//! CU-04 · Catálogo de Géneros (RF19). Escribe el Jefe; los demás consultan.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

async fn crear(c: &Ctx, t: &str, n: &str) -> i64 {
    c.post(t, "/api/generos", json!({ "genero_literario": n })).await.exito().data()["id_genero"]
        .as_i64()
        .unwrap()
}

/// PU-04-01 · El Jefe registra un género
/// Entrada: nombre único (solo letras)
/// Esperado: 200 con el id; aparece en la lista con 0 libros y 0 revistas
#[tokio::test]
async fn pu_04_01_alta_de_genero() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre_letras("Genero");
    let id = crear(&c, &j, &n).await;
    let lista = c.get(&j, "/api/generos").await.exito();
    let g = lista.data().as_array().unwrap().iter().find(|g| g["id_genero"] == id).cloned().unwrap();
    assert_eq!(g["genero_literario"], n.as_str());
    assert_eq!(g["total_libros"], 0);
}

/// PU-04-02 · Género repetido
/// Entrada: el mismo nombre otra vez, en minúsculas y con espacios alrededor
/// Esperado: 409 "Ya existe un género registrado con ese nombre"
#[tokio::test]
async fn pu_04_02_genero_repetido() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let n = nombre_letras("Genero");
    crear(&c, &j, &n).await;
    c.post(&j, "/api/generos", json!({ "genero_literario": format!("  {}  ", n.to_lowercase()) }))
        .await
        .error(StatusCode::CONFLICT, "Ya existe un género");
}

/// PU-04-03 · Nombre vacío o con números
/// Entrada: "" y "Novela 2"
/// Esperado: 400 "es obligatorio" y 400 "no puede contener números"
#[tokio::test]
async fn pu_04_03_validaciones() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/generos", json!({ "genero_literario": "" }))
        .await
        .error(StatusCode::BAD_REQUEST, "es obligatorio");
    c.post(&j, "/api/generos", json!({ "genero_literario": "Novela 2" }))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede contener números");
}

/// PU-04-04 · Modificar un género
/// Entrada: género nuevo; PUT con otro nombre único; luego PUT con el nombre de otro género existente
/// Esperado: 200 con el nombre nuevo; 409 "Ya existe otro género"
#[tokio::test]
async fn pu_04_04_modificar_genero() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let id = crear(&c, &j, &nombre_letras("Genero")).await;
    let otro = nombre_letras("Genero");
    crear(&c, &j, &otro).await;
    let nuevo = nombre_letras("Genero");
    let r = c.put(&j, &format!("/api/generos/{id}"), json!({ "genero_literario": nuevo })).await.exito();
    assert_eq!(r.data()["genero_literario"], nuevo.as_str());
    c.put(&j, &format!("/api/generos/{id}"), json!({ "genero_literario": otro }))
        .await
        .error(StatusCode::CONFLICT, "Ya existe otro género");
}

/// PU-04-05 · Eliminar un género sin uso
/// Entrada: género nuevo; DELETE
/// Esperado: 200 y ya no aparece en la lista
#[tokio::test]
async fn pu_04_05_eliminar_genero() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let id = crear(&c, &j, &nombre_letras("Genero")).await;
    c.delete(&j, &format!("/api/generos/{id}")).await.exito();
    let lista = c.get(&j, "/api/generos").await.exito();
    assert!(!lista.data().as_array().unwrap().iter().any(|g| g["id_genero"] == id));
}

/// PU-04-06 · No se elimina un género en uso
/// Entrada: DELETE del género de pruebas (tiene productos y muebles)
/// Esperado: 400 "está en uso"
#[tokio::test]
async fn pu_04_06_no_eliminar_genero_en_uso() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.libro(PLANETA, 100.0, 0, 0).await;
    c.delete(&j, &format!("/api/generos/{GENERO_PRUEBAS}"))
        .await
        .error(StatusCode::BAD_REQUEST, "está en uso");
}

/// PU-04-07 · Solo el Jefe escribe
/// Entrada: el Vendedor intenta POST /api/generos y consulta GET /api/generos
/// Esperado: 403 en el alta; 200 en la consulta
#[tokio::test]
async fn pu_04_07_solo_el_jefe_escribe() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let r = c.post(&v, "/api/generos", json!({ "genero_literario": nombre_letras("Genero") })).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
    c.get(&v, "/api/generos").await.exito();
}
