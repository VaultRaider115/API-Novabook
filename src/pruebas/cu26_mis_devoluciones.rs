//! CU-26 · Consultar mis devoluciones (RF26). Actor: Vendedor.
//! Solo las suyas, con estado, acta aprobada y motivo del rechazo.

use axum::http::StatusCode;

use super::*;

/// PU-26-01 · El Vendedor ve sus devoluciones con su estado
/// Entrada: el Vendedor registra una devolución; GET /api/vendedor/devoluciones
/// Esperado: 200; aparece como PENDIENTE
#[tokio::test]
async fn pu_26_01_ve_las_suyas() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    let r = c.get(&v, "/api/vendedor/devoluciones").await.exito();
    let d = r.data().as_array().unwrap().iter().find(|d| d["id_devolucion"] == id).cloned().expect("no aparece");
    assert_eq!(d["estado"], "PENDIENTE");
}

/// PU-26-02 · No ve las de otro vendedor
/// Entrada: otro Vendedor registra una devolución; el Vendedor de la semilla consulta las suyas
/// Esperado: la devolución del otro no aparece; el otro sí la ve en su lista
#[tokio::test]
async fn pu_26_02_no_ve_las_ajenas() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let otro = c.token(c.usuario_nuevo(3).await).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let ajena = c.devolucion(&otro, &p, 1).await;
    let mias = c.get(&v, "/api/vendedor/devoluciones").await.exito();
    assert!(!mias.data().as_array().unwrap().iter().any(|d| d["id_devolucion"] == ajena));
    let suyas = c.get(&otro, "/api/vendedor/devoluciones").await.exito();
    assert!(suyas.data().as_array().unwrap().iter().any(|d| d["id_devolucion"] == ajena));
}

/// PU-26-03 · Motivo del rechazo y filtro por estado
/// Entrada: devolución rechazada con "Faltan piezas del lote"; GET ?estado=RECHAZADO
/// Esperado: todas las filas son RECHAZADO y la devolución trae observaciones "Faltan piezas del lote"
#[tokio::test]
async fn pu_26_03_motivo_del_rechazo() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, id, false, "Faltan piezas del lote").await.exito();
    let r = c.get(&v, "/api/vendedor/devoluciones?estado=RECHAZADO").await.exito();
    let filas = r.data().as_array().unwrap();
    assert!(filas.iter().all(|d| d["estado"] == "RECHAZADO"));
    let d = filas.iter().find(|d| d["id_devolucion"] == id).expect("no aparece");
    assert_eq!(d["observaciones"], "Faltan piezas del lote");
}

/// PU-26-04 · Acta disponible al aprobarse
/// Entrada: devolución aprobada; el Vendedor descarga el acta desde Mis Devoluciones
/// Esperado: aparece como APROBADO y el acta es un PDF
#[tokio::test]
async fn pu_26_04_acta_al_aprobarse() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&v, "/api/vendedor/devoluciones?estado=APROBADO").await.exito();
    assert!(r.data().as_array().unwrap().iter().any(|d| d["id_devolucion"] == id));
    assert!(c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/libros")).await.bytes.starts_with(b"%PDF"));
}

/// PU-26-05 · Solo el Vendedor usa Mis Devoluciones
/// Entrada: Bodega hace GET /api/vendedor/devoluciones
/// Esperado: 403
#[tokio::test]
async fn pu_26_05_solo_vendedor() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    assert_eq!(c.get(&b, "/api/vendedor/devoluciones").await.status, StatusCode::FORBIDDEN);
}
