//! CU-21 · Consultar historial de devoluciones (RF02). Actores: Jefe y
//! Gerente (este último solo lectura).

use axum::http::StatusCode;

use super::*;

/// PU-21-01 · El Gerente consulta el historial
/// Entrada: devolución aprobada; GET /api/gerente/devoluciones/historial
/// Esperado: 200; la devolución aparece con estado APROBADO, proveedor y sus partidas
#[tokio::test]
async fn pu_21_01_gerente_consulta() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let g = c.token(GERENTE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 2).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&g, "/api/gerente/devoluciones/historial").await.exito();
    let d = r.data().as_array().unwrap().iter().find(|d| d["id_devolucion"] == id).cloned().expect("no aparece");
    assert_eq!(d["estado"], "APROBADO");
    assert_eq!(d["id_proveedor"], PLANETA);
    assert_eq!(d["items"].as_array().unwrap().len(), 1);
}

/// PU-21-02 · Filtro por estado
/// Entrada: una devolución pendiente y otra rechazada; historial con ?estado=RECHAZADO
/// Esperado: todas las filas son RECHAZADO; aparece la rechazada y no la pendiente
#[tokio::test]
async fn pu_21_02_filtro_por_estado() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let pendiente = c.devolucion(&v, &p, 1).await;
    let rechazada = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, rechazada, false, "No procede").await.exito();
    let r = c.get(&j, "/api/jefe/devoluciones/historial?estado=RECHAZADO").await.exito();
    let filas = r.data().as_array().unwrap();
    assert!(filas.iter().all(|d| d["estado"] == "RECHAZADO"));
    assert!(filas.iter().any(|d| d["id_devolucion"] == rechazada));
    assert!(!filas.iter().any(|d| d["id_devolucion"] == pendiente));
}

/// PU-21-03 · Filtro por proveedor
/// Entrada: devolución de Penguin; historial con ?id_proveedor=3
/// Esperado: todas las filas son del proveedor 3 y aparece la devolución
#[tokio::test]
async fn pu_21_03_filtro_por_proveedor() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PENGUIN, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    let r = c.get(&j, &format!("/api/jefe/devoluciones/historial?id_proveedor={PENGUIN}")).await.exito();
    let filas = r.data().as_array().unwrap();
    assert!(filas.iter().all(|d| d["id_proveedor"] == PENGUIN));
    assert!(filas.iter().any(|d| d["id_devolucion"] == id));
}

/// PU-21-04 · El Gerente consulta pero no aprueba
/// Entrada: el Gerente hace PUT /api/jefe/devoluciones/{id}/aprobar
/// Esperado: 403
#[tokio::test]
async fn pu_21_04_gerente_solo_lectura() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let g = c.token(GERENTE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    assert_eq!(c.evaluar(&g, id, true, "").await.status, StatusCode::FORBIDDEN);
}

/// PU-21-05 · Vendedor y Bodega no ven el historial general
/// Entrada: GET /api/gerente/devoluciones/historial y /api/jefe/devoluciones/historial como Vendedor y Bodega
/// Esperado: 403 en los cuatro casos
#[tokio::test]
async fn pu_21_05_otros_roles() {
    let c = ctx().await;
    for usuario in [VENDEDOR, BODEGA] {
        let t = c.token(usuario).await;
        for ruta in ["/api/gerente/devoluciones/historial", "/api/jefe/devoluciones/historial"] {
            assert_eq!(c.get(&t, ruta).await.status, StatusCode::FORBIDDEN, "{} en {ruta}", usuario.0);
        }
    }
}
