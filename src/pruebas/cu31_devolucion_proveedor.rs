//! CU-31 · Registrar devolución a proveedor (paso previo a RF13/RF14; sin
//! número propio en el diagrama). Actor: Vendedor. Máximo 60 piezas, solo
//! productos de ese proveedor, del tipo del documento y con stock en bodega +
//! apartados; queda PENDIENTE hasta que el Jefe la evalúa.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// PU-31-01 · Registrar una devolución
/// Entrada: libro de Planeta con 10 en bodega; 3 piezas con motivo 1
/// Esperado: 200; queda PENDIENTE con 3 piezas y la bodega no cambia hasta la aprobación
#[tokio::test]
async fn pu_31_01_registrar() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let r = c
        .post(&v, "/api/vendedor/devoluciones", json!({
            "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [{ "codigo_ean": p.ean, "cantidad": 3, "id_motivo": 1 }]
        }))
        .await
        .exito();
    assert_eq!(r.data()["estado"], "PENDIENTE");
    assert_eq!(r.data()["total_piezas"], 3);
    assert_eq!(c.bodega(p.ean).await, 10);
}

/// PU-31-02 · Máximo 60 piezas
/// Entrada: devolución de 61 piezas (hay 100 en bodega)
/// Esperado: 400 "no puede exceder 60 piezas"
#[tokio::test]
async fn pu_31_02_maximo_60() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 100).await;
    c.post(&v, "/api/vendedor/devoluciones", json!({
        "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [{ "codigo_ean": p.ean, "cantidad": 61, "id_motivo": 1 }]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "no puede exceder 60 piezas");
}

/// PU-31-03 · Producto de otro proveedor
/// Entrada: devolución a Planeta con un libro de Penguin
/// Esperado: 400 "pertenece al proveedor"
#[tokio::test]
async fn pu_31_03_otro_proveedor() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PENGUIN, 200.0, 0, 10).await;
    c.post(&v, "/api/vendedor/devoluciones", json!({
        "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [{ "codigo_ean": p.ean, "cantidad": 1, "id_motivo": 1 }]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "pertenece al proveedor");
}

/// PU-31-04 · Stock insuficiente en bodega y apartados
/// Entrada: libro con 2 en bodega, 1 apartado y 9 en piso; devolución de 4
/// Esperado: 400 "Stock insuficiente en bodega y apartados" (lo de piso no cuenta)
#[tokio::test]
async fn pu_31_04_stock_insuficiente() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 9, 2).await;
    sqlx::query("INSERT INTO apartados (codigo_ean, cantidad) VALUES (?, 1)")
        .bind(p.ean)
        .execute(&c.pool)
        .await
        .unwrap();
    c.post(&v, "/api/vendedor/devoluciones", json!({
        "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [{ "codigo_ean": p.ean, "cantidad": 4, "id_motivo": 1 }]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "Stock insuficiente en bodega y apartados");
}

/// PU-31-05 · Tipo, motivo y partidas
/// Entrada: revista en una devolución de libros; motivo 999; devolución sin partidas
/// Esperado: 400 "no es de tipo"; 400 "El motivo con ID 999 no existe"; 400 "al menos un artículo"
#[tokio::test]
async fn pu_31_05_validaciones() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let rev = c.revista(PLANETA, 90.0, 0, 5).await;
    c.post(&v, "/api/vendedor/devoluciones", json!({
        "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [{ "codigo_ean": rev.ean, "cantidad": 1, "id_motivo": 1 }]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "no es de tipo");
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    c.post(&v, "/api/vendedor/devoluciones", json!({
        "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [{ "codigo_ean": p.ean, "cantidad": 1, "id_motivo": 999 }]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "El motivo con ID 999 no existe");
    c.post(&v, "/api/vendedor/devoluciones", json!({ "id_proveedor": PLANETA, "tipo_producto": "libro", "items": [] }))
        .await
        .error(StatusCode::BAD_REQUEST, "al menos un artículo");
}

/// PU-31-06 · Validar la partida al pulsar "Añadir"
/// Entrada: GET /api/vendedor/devoluciones/validar con un libro del proveedor; con uno de otra editorial
/// Esperado: 200 en el primero; 4xx "es de la editorial" en el segundo
#[tokio::test]
async fn pu_31_06_validar_al_anadir() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let propio = c.libro(PLANETA, 200.0, 0, 5).await;
    let ajeno = c.libro(PENGUIN, 200.0, 0, 5).await;
    c.get(&v, &format!("/api/vendedor/devoluciones/validar?id_proveedor={PLANETA}&codigo_ean={}&cantidad=2", propio.ean))
        .await
        .exito();
    let r = c
        .get(&v, &format!("/api/vendedor/devoluciones/validar?id_proveedor={PLANETA}&codigo_ean={}&cantidad=1", ajeno.ean))
        .await;
    assert!(r.status.is_client_error(), "{}", r.json);
    assert!(r.mensaje().contains("es de la editorial"), "{}", r.mensaje());
}
