//! CU-19 · Consultar movimiento diario (RF03, Historial de Movimientos /
//! bitácora). Actor: Jefe. Por día, por rango de fechas y por categoría.

use axum::http::StatusCode;
use serde_json::json;

use super::*;

async fn hoy(c: &Ctx) -> String {
    sqlx::query_scalar("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')").fetch_one(&c.pool).await.unwrap()
}

/// PU-19-01 · El movimiento del día incluye lo que se acaba de hacer
/// Entrada: una venta del Vendedor; GET /api/jefe/movimientos/diarios (sin fecha = hoy)
/// Esperado: 200; aparece "Venta completada Ticket #<n>" con el empleado y su rol
#[tokio::test]
async fn pu_19_01_movimiento_de_hoy() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 100.0, 3, 0).await;
    let ticket = c
        .post(&v, "/api/vendedor/ventas", json!({ "cliente": "Ana", "metodo_pago": "EFECTIVO", "items": [{ "codigo_ean": p.ean, "cantidad": 1 }] }))
        .await
        .exito()
        .data()["id_venta"]
        .as_i64()
        .unwrap();
    let r = c.get(&j, "/api/jefe/movimientos/diarios").await.exito();
    assert_eq!(r.data()["fecha"], "Hoy");
    let fila = r
        .data()["movimientos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["detalle"].as_str().unwrap_or("").contains(&format!("Ticket #{ticket} ")))
        .cloned()
        .expect("la venta no aparece en el movimiento del día");
    assert!(fila["usuario"].as_str().unwrap().contains("Aldo"));
    assert!(!fila["rol"].as_str().unwrap().is_empty());
}

/// PU-19-02 · Día sin movimientos
/// Entrada: ?fecha=2000-01-01
/// Esperado: 200 con 0 eventos y lista vacía
#[tokio::test]
async fn pu_19_02_dia_sin_movimientos() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    let r = c.get(&j, "/api/jefe/movimientos/diarios?fecha=2000-01-01").await.exito();
    assert_eq!(r.data()["total_eventos"], 0);
    assert!(r.data()["movimientos"].as_array().unwrap().is_empty());
}

/// PU-19-03 · Bitácora por rango y categoría
/// Entrada: una compra de Bodega; GET /api/jefe/bitacora?desde=hoy&hasta=hoy&categoria=COMPRA
/// Esperado: 200; todos los eventos son de categoría COMPRA y aparece la compra
#[tokio::test]
async fn pu_19_03_filtro_por_categoria() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 0).await;
    let factura = format!("F-{}", 8000 + unico());
    c.post(&b, "/api/bodega/compras", json!({ "id_proveedor": PLANETA, "factura": factura, "items": [{ "codigo_ean": p.ean, "cantidad": 1, "costo_unitario": 50.0 }] }))
        .await
        .exito();
    let d = hoy(&c).await;
    let r = c.get(&j, &format!("/api/jefe/bitacora?desde={d}&hasta={d}&categoria=COMPRA&page=0&per_page=100")).await.exito();
    let items = r.data()["items"].as_array().expect("la página no trae items");
    assert!(items.iter().all(|e| e["categoria"] == "COMPRA"), "hay eventos de otra categoría");
    assert!(items.iter().any(|e| e["detalle"].as_str().unwrap_or("").contains(&factura)));
}

/// PU-19-04 · Paginación de la bitácora
/// Entrada: per_page=2, páginas 0 y 1 del día; y la misma consulta con per_page=4
/// Esperado: cada página trae 2 eventos, el total no cambia y las dos páginas juntas son los primeros 4 eventos
#[tokio::test]
async fn pu_19_04_paginacion() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    for _ in 0..3 {
        c.login(JEFE).await;
    }
    let d = hoy(&c).await;
    let pagina = |page: u32, per: u32| format!("/api/jefe/bitacora?desde={d}&hasta={d}&page={page}&per_page={per}");
    let p0 = c.get(&j, &pagina(0, 2)).await.exito();
    let p1 = c.get(&j, &pagina(1, 2)).await.exito();
    let todo = c.get(&j, &pagina(0, 4)).await.exito();
    let a = p0.data()["items"].as_array().unwrap();
    let b = p1.data()["items"].as_array().unwrap();
    assert_eq!((a.len(), b.len()), (2, 2));
    assert_eq!(p0.data()["total"], todo.data()["total"]);
    let juntas: Vec<_> = a.iter().chain(b.iter()).cloned().collect();
    assert_eq!(&juntas, todo.data()["items"].as_array().unwrap());
}

/// PU-19-05 · Fechas inválidas
/// Entrada: desde posterior a hasta; fecha "21-11-2026"
/// Esperado: 400 "no puede ser anterior a la inicial"; 400 "formato YYYY-MM-DD"
#[tokio::test]
async fn pu_19_05_fechas_invalidas() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.get(&j, "/api/jefe/bitacora?desde=2026-11-21&hasta=2026-11-01")
        .await
        .error(StatusCode::BAD_REQUEST, "no puede ser anterior a la inicial");
    c.get(&j, "/api/jefe/bitacora?desde=21-11-2026&hasta=21-11-2026")
        .await
        .error(StatusCode::BAD_REQUEST, "YYYY-MM-DD");
}

/// PU-19-06 · Solo el Jefe consulta el movimiento diario
/// Entrada: el Gerente, Bodega y el Vendedor consultan /api/jefe/movimientos/diarios
/// Esperado: 403 para los tres
#[tokio::test]
async fn pu_19_06_solo_el_jefe() {
    let c = ctx().await;
    for usuario in [GERENTE, BODEGA, VENDEDOR] {
        let t = c.token(usuario).await;
        let r = c.get(&t, "/api/jefe/movimientos/diarios").await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", usuario.0);
    }
}
