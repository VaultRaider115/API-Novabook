//! CU-16 · Aprobar devoluciones (RF04). Actor: Jefe de Departamento.
//! PENDIENTE → APROBADO / RECHAZADO; al aprobar sale el stock (primero
//! apartados, luego bodega); el rechazo exige observaciones.

use axum::http::StatusCode;

use super::*;

/// PU-16-01 · El Jefe aprueba una devolución
/// Entrada: devolución PENDIENTE de 4 piezas de un libro con 10 en bodega; aprobar con nota "Entregar el lunes"
/// Esperado: 200; estado APROBADO, la nota queda en observaciones y la bodega baja a 6
#[tokio::test]
async fn pu_16_01_aprobar() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 4).await;
    assert_eq!(c.bodega(p.ean).await, 10, "la bodega no debe cambiar mientras está pendiente");
    c.evaluar(&j, id, true, "Entregar el lunes").await.exito();
    let estado: String = sqlx::query_scalar("SELECT estado FROM devolucion WHERE id_devolucion = ?")
        .bind(id)
        .fetch_one(&c.pool)
        .await
        .unwrap();
    assert_eq!(estado, "APROBADO");
    let obs: Option<String> = sqlx::query_scalar("SELECT observaciones FROM devolucion WHERE id_devolucion = ?")
        .bind(id)
        .fetch_one(&c.pool)
        .await
        .unwrap();
    assert_eq!(obs.as_deref(), Some("Entregar el lunes"));
    assert_eq!(c.bodega(p.ean).await, 6);
}

/// PU-16-02 · Al aprobar se descuentan primero los apartados
/// Entrada: libro con 2 apartados (con defecto) y 10 en bodega; devolución de 3 aprobada
/// Esperado: apartados 0 y bodega 9
#[tokio::test]
async fn pu_16_02_primero_apartados() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    sqlx::query("INSERT INTO apartados (codigo_ean, cantidad) VALUES (?, 2)")
        .bind(p.ean)
        .execute(&c.pool)
        .await
        .unwrap();
    let id = c.devolucion(&v, &p, 3).await;
    c.evaluar(&j, id, true, "").await.exito();
    assert_eq!((c.apartados(p.ean).await, c.bodega(p.ean).await), (0, 9));
}

/// PU-16-03 · Rechazo con observaciones
/// Entrada: rechazar sin observaciones; luego rechazar con "Faltan piezas del lote"
/// Esperado: 400 "Debes indicar las observaciones del rechazo"; luego 200, estado RECHAZADO y la bodega sin cambio
#[tokio::test]
async fn pu_16_03_rechazo() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 2).await;
    c.evaluar(&j, id, false, "  ")
        .await
        .error(StatusCode::BAD_REQUEST, "observaciones del rechazo");
    c.evaluar(&j, id, false, "Faltan piezas del lote").await.exito();
    let estado: String = sqlx::query_scalar("SELECT estado FROM devolucion WHERE id_devolucion = ?")
        .bind(id)
        .fetch_one(&c.pool)
        .await
        .unwrap();
    assert_eq!(estado, "RECHAZADO");
    assert_eq!(c.bodega(p.ean).await, 5);
    assert!(c.en_bitacora("Faltan piezas del lote").await, "el motivo del rechazo debe ir a la bitácora");
}

/// PU-16-04 · Una devolución se evalúa una sola vez
/// Entrada: devolución aprobada; se intenta aprobar y rechazar otra vez
/// Esperado: 409 "ya fue evaluada" en ambos y la bodega no vuelve a bajar
#[tokio::test]
async fn pu_16_04_solo_una_vez() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 2).await;
    c.evaluar(&j, id, true, "").await.exito();
    c.evaluar(&j, id, true, "").await.error(StatusCode::CONFLICT, "ya fue evaluada");
    c.evaluar(&j, id, false, "Otra vez").await.error(StatusCode::CONFLICT, "ya fue evaluada");
    assert_eq!(c.bodega(p.ean).await, 8);
}

/// PU-16-05 · Devolución que no existe
/// Entrada: aprobar la devolución 999999
/// Esperado: 404 "no encontrada"
#[tokio::test]
async fn pu_16_05_no_existe() {
    let c = ctx().await;
    let j = c.token(JEFE).await;
    c.evaluar(&j, 999_999, true, "").await.error(StatusCode::NOT_FOUND, "no encontrada");
}

/// PU-16-06 · Solo el Jefe aprueba
/// Entrada: el Gerente y el Vendedor intentan aprobar una devolución pendiente
/// Esperado: 403 para ambos y la devolución sigue PENDIENTE
#[tokio::test]
async fn pu_16_06_solo_el_jefe() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    for usuario in [GERENTE, VENDEDOR] {
        let t = c.token(usuario).await;
        let r = c.evaluar(&t, id, true, "").await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{}: {}", usuario.0, r.json);
    }
    let estado: String = sqlx::query_scalar("SELECT estado FROM devolucion WHERE id_devolucion = ?")
        .bind(id)
        .fetch_one(&c.pool)
        .await
        .unwrap();
    assert_eq!(estado, "PENDIENTE");
}

/// PU-16-07 · El Jefe ve el detalle antes de decidir
/// Entrada: historial del Jefe filtrado por estado PENDIENTE
/// Esperado: la devolución aparece con su lista de títulos (EAN, título, piezas y motivo)
#[tokio::test]
async fn pu_16_07_detalle_antes_de_decidir() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.libro(PLANETA, 200.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 2).await;
    let r = c.get(&j, "/api/jefe/devoluciones/historial?estado=PENDIENTE").await.exito();
    let d = r.data().as_array().unwrap().iter().find(|d| d["id_devolucion"] == id).cloned().expect("no aparece");
    let item = &d["items"][0];
    assert_eq!(item["codigo_ean"], p.ean);
    assert_eq!(item["titulo"], p.titulo.as_str());
    assert_eq!(item["cantidad"], 2);
    assert!(item["motivo"].as_str().is_some());
}
