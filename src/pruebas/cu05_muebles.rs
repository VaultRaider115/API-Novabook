//! CU-05 · Catálogo de Muebles y Estantería (RF21). Escribe el Jefe.
//! Código P-… en Piso de Venta (ubicación 1) y B-… en Bodega (2); la capacidad
//! no baja de lo ocupado.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn mueble(ubicacion: i32, codigo: &str, capacidad: i32) -> Value {
    json!({
        "id_genero": GENERO_PRUEBAS, "id_ubicacion": ubicacion, "codigo_mueble": codigo,
        "nombre_mueble": nombre_letras("Mueble"), "capacidad": capacidad
    })
}

/// Código único con el prefijo del área ("P-5017").
fn codigo(prefijo: &str) -> String {
    format!("{prefijo}-{}", 5000 + unico())
}

/// PU-05-01 · El Jefe registra un mueble de piso y uno de bodega
/// Entrada: P-#### en Piso de Venta (capacidad 120) y B-#### en Bodega (capacidad 400)
/// Esperado: 200 para ambos, con su área, capacidad y 0 piezas ocupadas
#[tokio::test]
async fn pu_05_01_alta_de_muebles() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.post(&j, "/api/muebles", mueble(1, &codigo("P"), 120)).await.exito();
    assert_eq!(r.data()["id_ubicacion"], 1);
    assert_eq!(r.data()["capacidad"], 120);
    assert_eq!(r.data()["capacidad_actual"], 0);
    let r = c.post(&j, "/api/muebles", mueble(2, &codigo("B"), 400)).await.exito();
    assert_eq!(r.data()["id_ubicacion"], 2);
}

/// PU-05-02 · El prefijo del código depende del área
/// Entrada: código B-… en Piso de Venta; código P-… en Bodega; código X-… en Bodega
/// Esperado: 400 "debe empezar con" en los tres casos
#[tokio::test]
async fn pu_05_02_prefijo_segun_area() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    for (ubic, cod) in [(1, codigo("B")), (2, codigo("P")), (2, codigo("X"))] {
        c.post(&j, "/api/muebles", mueble(ubic, &cod, 100))
            .await
            .error(StatusCode::BAD_REQUEST, "debe empezar con");
    }
}

/// PU-05-03 · Código repetido
/// Entrada: el código P-011 (ya existe en la semilla)
/// Esperado: 409 "Ya existe un mueble con el código"
#[tokio::test]
async fn pu_05_03_codigo_repetido() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.post(&j, "/api/muebles", mueble(1, "P-011", 100))
        .await
        .error(StatusCode::CONFLICT, "Ya existe un mueble con el código");
}

/// PU-05-04 · Capacidad fuera de rango y área inválida
/// Entrada: capacidad 0; capacidad 100000; área 5
/// Esperado: 400 "entre 1 y 99,999" (dos veces) y 400 "Área inválida"
#[tokio::test]
async fn pu_05_04_capacidad_y_area() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    for cap in [0, 100_000] {
        c.post(&j, "/api/muebles", mueble(1, &codigo("P"), cap))
            .await
            .error(StatusCode::BAD_REQUEST, "entre 1 y 99,999");
    }
    c.post(&j, "/api/muebles", mueble(5, &codigo("P"), 10))
        .await
        .error(StatusCode::BAD_REQUEST, "Área inválida");
}

/// PU-05-05 · La capacidad no baja de lo ocupado
/// Entrada: mueble de bodega nuevo con 30 piezas guardadas; PUT con capacidad 10
/// Esperado: 400 "no puede ser menor a las 30 piezas"; con capacidad 30 sí se acepta
#[tokio::test]
async fn pu_05_05_capacidad_no_baja_de_lo_ocupado() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let cod = codigo("B");
    let m = c.post(&j, "/api/muebles", mueble(2, &cod, 100)).await.exito();
    let id = m.data()["id_mueble"].as_i64().unwrap();
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    c.poner_stock(p.ean, id as i32, 30).await;
    let mut cambio = mueble(2, &cod, 10);
    cambio["nombre_mueble"] = m.data()["nombre_mueble"].clone();
    c.put(&j, &format!("/api/muebles/{id}"), cambio.clone())
        .await
        .error(StatusCode::BAD_REQUEST, "no puede ser menor a las 30 piezas");
    cambio["capacidad"] = json!(30);
    c.put(&j, &format!("/api/muebles/{id}"), cambio).await.exito();
}

/// PU-05-06 · Con piezas no cambia de área
/// Entrada: mueble de bodega con piezas; PUT a Piso de Venta (con código P-…)
/// Esperado: 400 "No puedes cambiar el área"
#[tokio::test]
async fn pu_05_06_no_cambia_area_con_piezas() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let m = c.post(&j, "/api/muebles", mueble(2, &codigo("B"), 100)).await.exito();
    let id = m.data()["id_mueble"].as_i64().unwrap();
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    c.poner_stock(p.ean, id as i32, 5).await;
    let mut cambio = mueble(1, &codigo("P"), 100);
    cambio["nombre_mueble"] = m.data()["nombre_mueble"].clone();
    c.put(&j, &format!("/api/muebles/{id}"), cambio)
        .await
        .error(StatusCode::BAD_REQUEST, "No puedes cambiar el área");
}

/// PU-05-07 · Eliminar mueble vacío y no eliminar uno con piezas
/// Entrada: DELETE de un mueble nuevo vacío; DELETE de un mueble con 5 piezas
/// Esperado: 200 en el vacío; 400 "todavía tiene 5 pieza(s)" en el otro
#[tokio::test]
async fn pu_05_07_eliminar_mueble() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let vacio = c.post(&j, "/api/muebles", mueble(1, &codigo("P"), 50)).await.exito().data()["id_mueble"]
        .as_i64()
        .unwrap();
    c.delete(&j, &format!("/api/muebles/{vacio}")).await.exito();
    let lleno = c.post(&j, "/api/muebles", mueble(2, &codigo("B"), 50)).await.exito().data()["id_mueble"]
        .as_i64()
        .unwrap();
    let p = c.libro(PLANETA, 100.0, 0, 0).await;
    c.poner_stock(p.ean, lleno as i32, 5).await;
    c.delete(&j, &format!("/api/muebles/{lleno}"))
        .await
        .error(StatusCode::BAD_REQUEST, "todavía tiene 5 pieza(s)");
}

/// PU-05-08 · Solo el Jefe escribe
/// Entrada: Bodega intenta POST /api/muebles y consulta GET /api/muebles
/// Esperado: 403 en el alta; 200 en la consulta
#[tokio::test]
async fn pu_05_08_solo_el_jefe_escribe() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let r = c.post(&b, "/api/muebles", mueble(2, &codigo("B"), 50)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
    c.get(&b, "/api/muebles").await.exito();
}
