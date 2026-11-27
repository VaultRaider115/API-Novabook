//! CU-08 · Registrar Revistas (RF06, Alta de Mercancía). Actor: Bodega.
//! EAN-13 con prefijo 977 (ISSN), editorial obligatoria, SKU de 6 a 7 dígitos.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn revista(ean: i64, sku: i64) -> Value {
    json!({
        "codigo_ean": ean, "sku": sku, "nombre_revista": "Revista de prueba Abr 2026", "id_proveedor": PLANETA_INFANTIL,
        "id_editorial": 1, "numero_edicion": 346, "periodicidad": "Mensual", "generos": [GENERO_PRUEBAS],
        "precio": 120.0, "cantidad": 0, "id_mueble": MUEBLE_BODEGA
    })
}

/// PU-08-01 · Alta correcta de una revista
/// Entrada: EAN 977 nuevo, SKU, nombre, proveedor Planeta Mexico Infantil, editorial 1, edición 346, "Mensual", precio $120
/// Esperado: 200; queda como Revista con su editorial, número de edición y periodicidad
#[tokio::test]
async fn pu_08_01_alta_correcta() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let ean = ean_unico(977);
    c.post(&b, "/api/bodega/revistas", revista(ean, sku_unico())).await.exito();
    let fila = sqlx::query(
        "SELECT p.tipo_producto, r.id_editorial, r.numero_edicion, r.periodicidad FROM productos p \
         JOIN revistas r ON r.codigo_ean = p.codigo_ean WHERE p.codigo_ean = ?",
    )
    .bind(ean)
    .fetch_one(&c.pool)
    .await
    .unwrap();
    use sqlx::Row;
    assert_eq!(fila.get::<String, _>("tipo_producto"), "Revista");
    assert_eq!(fila.get::<i32, _>("id_editorial"), 1);
    assert_eq!(fila.get::<i32, _>("numero_edicion"), 346);
    assert_eq!(fila.get::<String, _>("periodicidad"), "Mensual");
}

/// PU-08-02 · EAN de libro en el alta de revistas
/// Entrada: EAN 978 válido
/// Esperado: 400 "corresponde a un Libro"
#[tokio::test]
async fn pu_08_02_ean_de_libro() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/revistas", revista(ean_unico(978), sku_unico()))
        .await
        .error(StatusCode::BAD_REQUEST, "corresponde a un Libro");
}

/// PU-08-03 · Dígito verificador incorrecto
/// Entrada: EAN 977 con el último dígito cambiado
/// Esperado: 400 "Dígito verificador inválido"
#[tokio::test]
async fn pu_08_03_digito_verificador() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let bueno = ean_unico(977);
    let malo = bueno - bueno % 10 + (bueno % 10 + 1) % 10;
    c.post(&b, "/api/bodega/revistas", revista(malo, sku_unico()))
        .await
        .error(StatusCode::BAD_REQUEST, "Dígito verificador inválido");
}

/// PU-08-04 · La editorial es obligatoria
/// Entrada: alta sin id_editorial
/// Esperado: 400 "La editorial (id_editorial) es obligatoria"
#[tokio::test]
async fn pu_08_04_editorial_obligatoria() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let mut r = revista(ean_unico(977), sku_unico());
    r.as_object_mut().unwrap().remove("id_editorial");
    c.post(&b, "/api/bodega/revistas", r)
        .await
        .error(StatusCode::BAD_REQUEST, "editorial (id_editorial) es obligatoria");
}

/// PU-08-05 · Revista ya registrada y SKU repetido
/// Entrada: EAN 9771234567010 (National Geographic, de la semilla); SKU de otra revista ya registrada
/// Esperado: 409 "ya está registrado"; 409 "ya está asignado"
#[tokio::test]
async fn pu_08_05_duplicados() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/revistas", revista(9_771_234_567_010, sku_unico()))
        .await
        .error(StatusCode::CONFLICT, "ya está registrado");
    let otra = c.revista(PLANETA_INFANTIL, 90.0, 0, 0).await;
    c.post(&b, "/api/bodega/revistas", revista(ean_unico(977), otra.sku))
        .await
        .error(StatusCode::CONFLICT, "ya está asignado");
}

/// PU-08-06 · Precio y nombre
/// Entrada: precio $0; nombre de 151 caracteres
/// Esperado: 400 "debe ser mayor a $0"; 400 "no puede pasar de 150 caracteres"
#[tokio::test]
async fn pu_08_06_precio_y_nombre() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let mut r = revista(ean_unico(977), sku_unico());
    r["precio"] = json!(0);
    c.post(&b, "/api/bodega/revistas", r).await.error(StatusCode::BAD_REQUEST, "debe ser mayor a $0");
    let mut r = revista(ean_unico(977), sku_unico());
    r["nombre_revista"] = json!("R".repeat(151));
    c.post(&b, "/api/bodega/revistas", r)
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 150 caracteres");
}

/// PU-08-07 · Solo Bodega da de alta
/// Entrada: el Vendedor hace POST /api/bodega/revistas
/// Esperado: 403
#[tokio::test]
async fn pu_08_07_solo_bodega() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let r = c.post(&v, "/api/bodega/revistas", revista(ean_unico(977), sku_unico())).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
}
