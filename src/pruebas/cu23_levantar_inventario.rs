//! CU-23 · Levantar inventario mensual (RF23). Actor: Vendedor.
//! Uno abierto a la vez; se escanea (conteo por título) y se cierra; al cerrar
//! se guarda el piso del sistema y se marca a devolución lo que pidió una
//! editorial o lo añejado. Estas pruebas usan `ctx_exclusivo`.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

/// Abre un inventario (cerrando antes uno que haya quedado abierto) y da su id.
pub(super) async fn abrir(c: &Ctx, v: &str) -> i64 {
    let actual = c.get(v, "/api/vendedor/inventarios/actual").await;
    if let Some(id) = actual.data()["resumen"]["id_inventario"].as_i64() {
        c.post(v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await;
    }
    c.post(v, "/api/vendedor/inventarios", json!({})).await.exito().data()["resumen"]["id_inventario"]
        .as_i64()
        .expect("abrir no devolvió id_inventario")
}

/// Estado del inventario en una respuesta de detalle.
pub(super) fn estado(r: &Resp) -> &Value {
    &r.data()["resumen"]["estado"]
}

fn detalle(r: &Resp, ean: i64) -> Option<&Value> {
    r.data()["items"].as_array()?.iter().find(|x| x["codigo_ean"] == ean)
}

/// PU-23-01 · Abrir inventario y solo uno a la vez
/// Entrada: POST /api/vendedor/inventarios dos veces seguidas
/// Esperado: el primero 200 (ABIERTO); el segundo 409 "Ya hay un inventario abierto"
#[tokio::test]
async fn pu_23_01_abrir_uno_a_la_vez() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let id = abrir(&c, &v).await;
    c.post(&v, "/api/vendedor/inventarios", json!({}))
        .await
        .error(StatusCode::CONFLICT, "Ya hay un inventario abierto");
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
}

/// PU-23-02 · Registrar conteos
/// Entrada: libro con 6 en piso; conteo de 5 piezas; el inventario actual
/// Esperado: 200; el detalle del inventario abierto trae el libro con 5 contadas
#[tokio::test]
async fn pu_23_02_registrar_conteo() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 6, 0).await;
    let id = abrir(&c, &v).await;
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": 5 }))
        .await
        .exito();
    let r = c.get(&v, "/api/vendedor/inventarios/actual").await.exito();
    let f = detalle(&r, p.ean).expect("el conteo no aparece en el inventario");
    assert_eq!(f["cantidad_contada"], 5);
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
}

/// PU-23-03 · Conteos inválidos
/// Entrada: cantidad -1; EAN que no existe
/// Esperado: 400 "La cantidad contada debe estar entre 0 y …"; 404 "no existe"
#[tokio::test]
async fn pu_23_03_conteos_invalidos() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 2, 0).await;
    let id = abrir(&c, &v).await;
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": -1 }))
        .await
        .error(StatusCode::BAD_REQUEST, "debe estar entre 0 y");
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": ean_unico(978), "cantidad": 1 }))
        .await
        .error(StatusCode::NOT_FOUND, "no existe");
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
}

/// PU-23-04 · Cerrar guarda el piso del sistema y marca lo que pidió la editorial
/// Entrada: libro con 4 en piso y una solicitud ABIERTA de su editorial; se cuentan 3 y se cierra
/// Esperado: estado CERRADO; el libro queda con stock del sistema 4, 3 contadas y motivo SOLICITUD_EDITORIAL
#[tokio::test]
async fn pu_23_04_cerrar_marca_devolucion() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 4, 0).await;
    c.post(&j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PLANETA, "referencia": "Correo de prueba", "eans": [p.ean] }))
        .await
        .exito();
    let id = abrir(&c, &v).await;
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": 3 }))
        .await
        .exito();
    let r = c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
    assert_eq!(estado(&r), "CERRADO");
    let f = detalle(&r, p.ean).expect("el libro no aparece al cerrar");
    assert_eq!(f["stock_sistema"], 4);
    assert_eq!(f["cantidad_contada"], 3);
    assert_eq!(f["motivo_devolucion"], "SOLICITUD_EDITORIAL");
}

/// PU-23-05 · Mientras cuenta, el Vendedor ve qué separar
/// Entrada: libro con solicitud ABIERTA de su editorial; se cuentan 2; GET /inventarios/{id}/devolucion
/// Esperado: el libro aparece para devolución con 2 encontradas
#[tokio::test]
async fn pu_23_05_para_devolucion_en_tiempo_real() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PENGUIN, 200.0, 3, 1).await;
    c.post(&j, "/api/jefe/solicitudes-editorial", json!({ "id_proveedor": PENGUIN, "referencia": "Correo de prueba", "eans": [p.ean] }))
        .await
        .exito();
    let id = abrir(&c, &v).await;
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": 2 }))
        .await
        .exito();
    let r = c.get(&v, &format!("/api/vendedor/inventarios/{id}/devolucion")).await.exito();
    let lista = r.data()["items"].as_array().expect("sin lista de títulos");
    let f = lista.iter().find(|x| x["codigo_ean"] == p.ean).expect("el libro no aparece para devolución");
    assert_eq!(f["encontradas"], 2);
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
}

/// PU-23-06 · No se cuenta en un inventario cerrado
/// Entrada: inventario cerrado; PUT de un conteo
/// Esperado: 409 "no se puede contar"
#[tokio::test]
async fn pu_23_06_no_contar_cerrado() {
    let c = ctx_exclusivo().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 2, 0).await;
    let id = abrir(&c, &v).await;
    c.post(&v, &format!("/api/vendedor/inventarios/{id}/cerrar"), json!({})).await.exito();
    c.put(&v, &format!("/api/vendedor/inventarios/{id}/conteos"), json!({ "codigo_ean": p.ean, "cantidad": 1 }))
        .await
        .error(StatusCode::CONFLICT, "no se puede contar");
}
