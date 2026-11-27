//! CU-15 · Ventas (RF09, Punto de Venta). Actor: Vendedor.
//! Nombre del cliente (máx. 60) y método de pago EFECTIVO o TARJETA; se vende
//! al precio del catálogo y solo lo que hay en piso.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn venta(cliente: &str, metodo: &str, items: Value) -> Value {
    json!({ "cliente": cliente, "metodo_pago": metodo, "items": items })
}

/// PU-15-01 · Venta correcta
/// Entrada: libro de $320 con 5 en piso; cliente "Ana López", TARJETA, 2 piezas
/// Esperado: 200 con ticket, total $640, método TARJETA; el piso baja a 3 y la bodega no cambia
#[tokio::test]
async fn pu_15_01_venta_correcta() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 320.0, 5, 4).await;
    let r = c
        .post(&v, "/api/vendedor/ventas", venta("Ana López", "TARJETA", json!([{ "codigo_ean": p.ean, "tipo_producto": "libro", "cantidad": 2 }])))
        .await
        .exito();
    assert!(r.data()["id_venta"].as_i64().unwrap() > 0);
    assert_eq!(r.data()["total"], 640.0);
    assert_eq!(r.data()["metodo_pago"], "TARJETA");
    assert_eq!(r.data()["cliente"], "Ana López");
    assert_eq!((c.piso(p.ean).await, c.bodega(p.ean).await), (3, 4));
}

/// PU-15-02 · Venta de libros y revistas en un ticket
/// Entrada: libro de $200 x1 y revista de $95 x3 (ambos en piso), EFECTIVO
/// Esperado: 200; total $485, 4 artículos y cada partida al precio del catálogo
#[tokio::test]
async fn pu_15_02_ticket_mixto() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let l = c.libro(PLANETA, 200.0, 2, 0).await;
    let rv = c.revista(PLANETA_INFANTIL, 95.0, 5, 0).await;
    let r = c
        .post(&v, "/api/vendedor/ventas", venta("Público en General", "EFECTIVO", json!([
            { "codigo_ean": l.ean, "tipo_producto": "libro", "cantidad": 1 },
            { "codigo_ean": rv.ean, "tipo_producto": "revista", "cantidad": 3 }
        ])))
        .await
        .exito();
    assert_eq!(r.data()["total"], 485.0);
    assert_eq!(r.data()["total_articulos"], 4);
    let precios: Vec<f64> = r.data()["items"].as_array().unwrap().iter().map(|i| i["precio_unitario"].as_f64().unwrap()).collect();
    assert_eq!(precios, vec![200.0, 95.0]);
}

/// PU-15-03 · Cliente obligatorio y con límite
/// Entrada: cliente ""; cliente de 61 caracteres
/// Esperado: 400 "El nombre del cliente es obligatorio"; 400 "no puede pasar de 60 caracteres (tiene 61)"
#[tokio::test]
async fn pu_15_03_cliente() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 100.0, 5, 0).await;
    let items = json!([{ "codigo_ean": p.ean, "tipo_producto": "libro", "cantidad": 1 }]);
    c.post(&v, "/api/vendedor/ventas", venta("", "EFECTIVO", items.clone()))
        .await
        .error(StatusCode::BAD_REQUEST, "El nombre del cliente es obligatorio");
    c.post(&v, "/api/vendedor/ventas", venta(&"a".repeat(61), "EFECTIVO", items))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 60 caracteres (tiene 61)");
    assert_eq!(c.piso(p.ean).await, 5);
}

/// PU-15-04 · Método de pago
/// Entrada: método "TRANSFERENCIA" (retirado) y método "CHEQUE"
/// Esperado: 400 "Método de pago inválido … (usa EFECTIVO o TARJETA)" en ambos
#[tokio::test]
async fn pu_15_04_metodo_de_pago() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 100.0, 5, 0).await;
    for metodo in ["TRANSFERENCIA", "CHEQUE"] {
        c.post(&v, "/api/vendedor/ventas", venta("Ana", metodo, json!([{ "codigo_ean": p.ean, "tipo_producto": "libro", "cantidad": 1 }])))
            .await
            .error(StatusCode::BAD_REQUEST, "usa EFECTIVO o TARJETA");
    }
}

/// PU-15-05 · Sin stock en piso no se vende
/// Entrada: libro con 0 en piso y 10 en bodega; venta de 1; libro con 2 en piso, venta de 3
/// Esperado: 400 "Stock insuficiente en tienda" en ambos; nada cambia
#[tokio::test]
async fn pu_15_05_sin_stock_en_piso() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let a = c.libro(PLANETA, 100.0, 0, 10).await;
    let b = c.libro(PLANETA, 100.0, 2, 0).await;
    c.post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([{ "codigo_ean": a.ean, "tipo_producto": "libro", "cantidad": 1 }])))
        .await
        .error(StatusCode::BAD_REQUEST, "Stock insuficiente en tienda");
    c.post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([{ "codigo_ean": b.ean, "tipo_producto": "libro", "cantidad": 3 }])))
        .await
        .error(StatusCode::BAD_REQUEST, "Stock insuficiente en tienda");
    assert_eq!((c.bodega(a.ean).await, c.piso(b.ean).await), (10, 2));
}

/// PU-15-06 · Todo o nada
/// Entrada: ticket con un libro disponible (1 de 5) y otro sin piezas suficientes
/// Esperado: 400 y el libro disponible no se descuenta
#[tokio::test]
async fn pu_15_06_todo_o_nada() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let a = c.libro(PLANETA, 100.0, 5, 0).await;
    let b = c.libro(PLANETA, 100.0, 1, 0).await;
    let r = c
        .post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([
            { "codigo_ean": a.ean, "tipo_producto": "libro", "cantidad": 1 },
            { "codigo_ean": b.ean, "tipo_producto": "libro", "cantidad": 2 }
        ])))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.json);
    assert_eq!(c.piso(a.ean).await, 5);
}

/// PU-15-07 · Carrito vacío, cantidad 0 y producto inexistente
/// Entrada: items vacío; cantidad 0; EAN que no existe
/// Esperado: 400 "no puede estar vacío"; 400 "debe ser mayor a 0"; 404 "no existe"
#[tokio::test]
async fn pu_15_07_partidas_invalidas() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 100.0, 5, 0).await;
    c.post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([])))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede estar vacío");
    c.post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([{ "codigo_ean": p.ean, "tipo_producto": "libro", "cantidad": 0 }])))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a 0");
    c.post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([{ "codigo_ean": ean_unico(978), "tipo_producto": "libro", "cantidad": 1 }])))
        .await
        .error(StatusCode::NOT_FOUND, "no existe");
}

/// PU-15-08 · La venta queda en la bitácora
/// Entrada: venta correcta de 1 pieza
/// Esperado: la bitácora tiene "Venta completada Ticket #<n>"
#[tokio::test]
async fn pu_15_08_bitacora() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 100.0, 5, 0).await;
    let r = c
        .post(&v, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([{ "codigo_ean": p.ean, "tipo_producto": "libro", "cantidad": 1 }])))
        .await
        .exito();
    let ticket = r.data()["id_venta"].as_i64().unwrap();
    assert!(c.en_bitacora(&format!("Venta completada Ticket #{ticket} ")).await);
}

/// PU-15-09 · Solo el Vendedor vende
/// Entrada: Bodega y Jefe hacen POST /api/vendedor/ventas
/// Esperado: 403 para ambos
#[tokio::test]
async fn pu_15_09_solo_vendedor() {
    let c = ctx().await;
    let p = c.libro(PLANETA, 100.0, 5, 0).await;
    for usuario in [BODEGA, JEFE] {
        let t = c.token(usuario).await;
        let r = c
            .post(&t, "/api/vendedor/ventas", venta("Ana", "EFECTIVO", json!([{ "codigo_ean": p.ean, "tipo_producto": "libro", "cantidad": 1 }])))
            .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
    }
}
