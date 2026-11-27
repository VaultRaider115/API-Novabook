//! CU-32 · Tickets de venta (hito 2, CR-014; sin número propio en el
//! diagrama). Actor: Jefe. Lista por rango de fechas y búsqueda; detalle con
//! lo vendido y lo ya devuelto.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

async fn vender(c: &Ctx, cliente: &str, p: &Producto, cantidad: i32) -> i64 {
    let v = c.token(VENDEDOR).await;
    c.post(&v, "/api/vendedor/ventas", json!({
        "cliente": cliente, "metodo_pago": "TARJETA", "items": [{ "codigo_ean": p.ean, "cantidad": cantidad }]
    }))
    .await
    .exito()
    .data()["id_venta"]
    .as_i64()
    .unwrap()
}

/// PU-32-01 · Tickets de hoy
/// Entrada: venta de 2 piezas de $150 a "Cliente Prueba Uno"; GET /api/jefe/ventas (sin fechas)
/// Esperado: 200; el ticket aparece con vendedor, cliente, TARJETA, 1 título, 2 piezas y $300
#[tokio::test]
async fn pu_32_01_tickets_de_hoy() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 150.0, 5, 0).await;
    let t = vender(&c, "Cliente Prueba Uno", &p, 2).await;
    let r = c.get(&j, "/api/jefe/ventas").await.exito();
    let f = r.data().as_array().unwrap().iter().find(|x| x["id_venta"] == t).cloned().expect("no aparece");
    assert_eq!(f["nombre_cliente"], "Cliente Prueba Uno");
    assert_eq!(f["metodo_pago"], "TARJETA");
    assert_eq!((f["titulos"].as_i64(), f["piezas"].as_i64(), f["total"].as_f64()), (Some(1), Some(2), Some(300.0)));
    assert!(f["vendedor"].as_str().unwrap().contains("Aldo"));
}

/// PU-32-02 · Búsqueda por ticket, cliente y vendedor
/// Entrada: q = "#<ticket>", q = "Prueba Dos" y q = "Aldo"
/// Esperado: el primero devuelve solo ese ticket; el segundo lo incluye; el tercero también
#[tokio::test]
async fn pu_32_02_busqueda() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 150.0, 5, 0).await;
    let t = vender(&c, "Cliente Prueba Dos", &p, 1).await;
    let r = c.get(&j, &format!("/api/jefe/ventas?q=%23{t}")).await.exito();
    let filas = r.data().as_array().unwrap();
    assert_eq!(filas.len(), 1);
    assert_eq!(filas[0]["id_venta"], t);
    for q in ["Prueba%20Dos", "Aldo"] {
        let r = c.get(&j, &format!("/api/jefe/ventas?q={q}")).await.exito();
        assert!(r.data().as_array().unwrap().iter().any(|x| x["id_venta"] == t), "q={q}");
    }
}

/// PU-32-03 · Detalle con lo devuelto
/// Entrada: ticket de 2 piezas; el cliente devuelve 1; GET /api/jefe/tickets/{ticket} y la lista
/// Esperado: el detalle trae 2 vendidas y 1 devuelta; en la lista, piezas_devueltas = 1
#[tokio::test]
async fn pu_32_03_detalle_con_devolucion() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 150.0, 5, 0).await;
    let t = vender(&c, "Cliente Prueba Tres", &p, 2).await;
    c.post(&v, "/api/vendedor/devoluciones-cliente", json!({
        "id_venta": t, "resolucion": "REEMBOLSO", "metodo_pago": "EFECTIVO", "items": [{ "codigo_ean": p.ean, "cantidad": 1 }]
    }))
    .await
    .exito();
    let d = c.get(&j, &format!("/api/jefe/tickets/{t}")).await.exito();
    assert_eq!(d.data()["items"][0]["cantidad_vendida"], 2);
    assert_eq!(d.data()["items"][0]["cantidad_devuelta"], 1);
    let r = c.get(&j, &format!("/api/jefe/ventas?q=%23{t}")).await.exito();
    assert_eq!(r.data()[0]["piezas_devueltas"], 1);
}

/// PU-32-04 · Rango de fechas
/// Entrada: rango de 2000-01-01 a 2000-01-31; rango invertido; fecha "21-11-2026"
/// Esperado: 200 con lista vacía; 400 "no puede ser anterior a la inicial"; 400 "YYYY-MM-DD"
#[tokio::test]
async fn pu_32_04_rango_de_fechas() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.get(&j, "/api/jefe/ventas?desde=2000-01-01&hasta=2000-01-31").await.exito();
    assert!(r.data().as_array().unwrap().is_empty());
    c.get(&j, "/api/jefe/ventas?desde=2026-11-21&hasta=2026-11-01")
        .await
        .error(StatusCode::BAD_REQUEST, "no puede ser anterior a la inicial");
    c.get(&j, "/api/jefe/ventas?desde=21-11-2026")
        .await
        .error(StatusCode::BAD_REQUEST, "YYYY-MM-DD");
}

/// PU-32-05 · Búsqueda demasiado larga
/// Entrada: q de 61 caracteres (límite 60)
/// Esperado: 400 "no puede pasar de 60 caracteres"
#[tokio::test]
async fn pu_32_05_busqueda_larga() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.get(&j, &format!("/api/jefe/ventas?q={}", "a".repeat(61)))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 60 caracteres");
}

/// PU-32-06 · Solo el Jefe consulta los tickets
/// Entrada: Vendedor, Bodega y Gerente hacen GET /api/jefe/ventas
/// Esperado: 403 para los tres
#[tokio::test]
async fn pu_32_06_solo_el_jefe() {
    let c = ctx().await;
    for usuario in [VENDEDOR, BODEGA, GERENTE] {
        let t = c.token(usuario).await;
        assert_eq!(c.get(&t, "/api/jefe/ventas").await.status, StatusCode::FORBIDDEN, "{}", usuario.0);
    }
}
