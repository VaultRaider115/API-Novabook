//! CU-22 · Devolución de cliente (RF22). Actor: Vendedor.
//! Ticket de 15 días o menos; no más de lo vendido; reembolso al precio del
//! ticket; sin defecto regresa a piso, con defecto va a apartados; cambio solo
//! por igual o mayor valor pagando la diferencia.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

/// Vende `cantidad` piezas del producto y devuelve el número de ticket.
async fn vender(c: &Ctx, v: &str, p: &Producto, cantidad: i32) -> i64 {
    c.post(v, "/api/vendedor/ventas", json!({
        "cliente": "Ana López", "metodo_pago": "EFECTIVO",
        "items": [{ "codigo_ean": p.ean, "tipo_producto": p.tipo_json(), "cantidad": cantidad }]
    }))
    .await
    .exito()
    .data()["id_venta"]
    .as_i64()
    .unwrap()
}

fn reembolso(ticket: i64, ean: i64, cantidad: i32, defecto: bool) -> Value {
    json!({
        "id_venta": ticket, "resolucion": "REEMBOLSO", "metodo_pago": "EFECTIVO",
        "items": [{ "codigo_ean": ean, "cantidad": cantidad, "con_defecto": defecto }]
    })
}

/// PU-22-01 · Consulta del ticket
/// Entrada: venta de 2 piezas de un libro de $300; GET /api/vendedor/tickets/{ticket}
/// Esperado: 200; cliente, total $600, vigente (0 días), 2 vendidas y 0 devueltas
#[tokio::test]
async fn pu_22_01_consultar_ticket() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 2).await;
    let r = c.get(&v, &format!("/api/vendedor/tickets/{t}")).await.exito();
    assert_eq!(r.data()["nombre_cliente"], "Ana López");
    assert_eq!(r.data()["total"], 600.0);
    assert_eq!(r.data()["vigente"], true);
    assert_eq!(r.data()["items"][0]["cantidad_vendida"], 2);
    assert_eq!(r.data()["items"][0]["cantidad_devuelta"], 0);
}

/// PU-22-02 · Reembolso sin defecto
/// Entrada: ticket de 2 piezas de $300 (piso queda en 3); devolver 1 sin defecto
/// Esperado: 200, reembolso $300 y la pieza regresa a piso (4)
#[tokio::test]
async fn pu_22_02_reembolso_regresa_a_piso() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 2).await;
    let r = c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(t, p.ean, 1, false)).await.exito();
    assert_eq!(r.data()["reembolso"], 300.0);
    assert_eq!(c.piso(p.ean).await, 4);
}

/// PU-22-03 · Reembolso con defecto
/// Entrada: devolver 1 pieza con defecto
/// Esperado: 200; la pieza va a apartados (1) y el piso no sube
#[tokio::test]
async fn pu_22_03_con_defecto_va_a_apartados() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 2).await;
    c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(t, p.ean, 1, true)).await.exito();
    assert_eq!((c.apartados(p.ean).await, c.piso(p.ean).await), (1, 3));
}

/// PU-22-04 · No más de lo vendido
/// Entrada: ticket de 2 piezas; devolver 1 y luego 2 más
/// Esperado: la primera 200; la segunda 400 "solo se pueden devolver 1 más"
#[tokio::test]
async fn pu_22_04_no_mas_de_lo_vendido() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 2).await;
    c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(t, p.ean, 1, false)).await.exito();
    c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(t, p.ean, 2, false))
        .await
        .error(StatusCode::BAD_REQUEST, "solo se pueden devolver 1 más");
}

/// PU-22-05 · Ticket de más de 15 días
/// Entrada: ticket con fecha de hace 16 días
/// Esperado: 409 "solo se aceptan devoluciones de 15 días o menos"
#[tokio::test]
async fn pu_22_05_ticket_vencido() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 1).await;
    sqlx::query("UPDATE ventas SET fecha_hora = NOW() - INTERVAL 16 DAY WHERE id_ventas = ?")
        .bind(t)
        .execute(&c.pool)
        .await
        .unwrap();
    c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(t, p.ean, 1, false))
        .await
        .error(StatusCode::CONFLICT, "15 días o menos");
}

/// PU-22-06 · Producto que no está en el ticket y ticket inexistente
/// Entrada: devolver un EAN que no se vendió en el ticket; ticket 999999
/// Esperado: 400 "no está en el ticket"; 404 "no existe"
#[tokio::test]
async fn pu_22_06_producto_o_ticket_invalido() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let otro = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 1).await;
    c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(t, otro.ean, 1, false))
        .await
        .error(StatusCode::BAD_REQUEST, "no está en el ticket");
    c.post(&v, "/api/vendedor/devoluciones-cliente", reembolso(999_999, p.ean, 1, false))
        .await
        .error(StatusCode::NOT_FOUND, "no existe");
}

/// PU-22-07 · Cambio por un producto más barato
/// Entrada: devolver 1 libro de $300 a cambio de una revista de $90
/// Esperado: 400 "El cambio debe ser por productos de igual o mayor precio"
#[tokio::test]
async fn pu_22_07_cambio_por_menor_precio() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let barato = c.revista(PLANETA_INFANTIL, 90.0, 5, 0).await;
    let t = vender(&c, &v, &p, 1).await;
    c.post(&v, "/api/vendedor/devoluciones-cliente", json!({
        "id_venta": t, "resolucion": "CAMBIO", "metodo_pago": "EFECTIVO",
        "items": [{ "codigo_ean": p.ean, "cantidad": 1 }],
        "items_cambio": [{ "codigo_ean": barato.ean, "cantidad": 1 }]
    }))
    .await
    .error(StatusCode::BAD_REQUEST, "igual o mayor precio");
}

/// PU-22-08 · Cambio por un producto de mayor precio
/// Entrada: devolver 1 libro de $300 a cambio de otro de $350
/// Esperado: 200; se crea una venta nueva ligada y se cobra la diferencia de $50; el libro nuevo sale de piso
#[tokio::test]
async fn pu_22_08_cambio_paga_diferencia() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let caro = c.libro(PLANETA, 350.0, 5, 0).await;
    let t = vender(&c, &v, &p, 1).await;
    let r = c
        .post(&v, "/api/vendedor/devoluciones-cliente", json!({
            "id_venta": t, "resolucion": "CAMBIO", "metodo_pago": "EFECTIVO",
            "items": [{ "codigo_ean": p.ean, "cantidad": 1 }],
            "items_cambio": [{ "codigo_ean": caro.ean, "cantidad": 1 }]
        }))
        .await
        .exito();
    assert!(r.data()["id_venta_cambio"].as_i64().unwrap() > t);
    assert_eq!(r.data()["diferencia_cobrada"], 50.0);
    assert_eq!(c.piso(caro.ean).await, 4);
}

/// PU-22-09 · Método de reembolso
/// Entrada: reembolso con método TRANSFERENCIA
/// Esperado: 400 "usa EFECTIVO o TARJETA"
#[tokio::test]
async fn pu_22_09_metodo_de_reembolso() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 300.0, 5, 0).await;
    let t = vender(&c, &v, &p, 1).await;
    let mut d = reembolso(t, p.ean, 1, false);
    d["metodo_pago"] = json!("TRANSFERENCIA");
    c.post(&v, "/api/vendedor/devoluciones-cliente", d)
        .await
        .error(StatusCode::BAD_REQUEST, "usa EFECTIVO o TARJETA");
}
