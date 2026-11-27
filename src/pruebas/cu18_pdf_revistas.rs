//! CU-18 · PDF de devolución a proveedor de revistas (RF14).

use axum::http::StatusCode;

use super::*;

/// PU-18-01 · El solicitante descarga el acta de revistas aprobada
/// Entrada: devolución de 5 piezas de una revista de Planeta Mexico Infantil, aprobada; GET .../pdf/revistas
/// Esperado: 200, application/pdf, empieza con %PDF y trae el EAN, el nombre y "Edición mensual · No. 12"
#[tokio::test]
async fn pu_18_01_acta_de_revistas() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 5).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/revistas")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type, "application/pdf");
    assert!(r.bytes.starts_with(b"%PDF"));
    let t = String::from_utf8_lossy(&r.bytes).to_string();
    assert!(t.contains(&p.ean.to_string()), "el acta no trae el EAN");
    assert!(t.contains("Revista de prueba"), "el acta no trae el nombre");
    assert!(t.contains("No. 12"), "el acta no trae el número de edición");
}

/// PU-18-02 · Datos del proveedor en el acta
/// Entrada: acta de revistas aprobada
/// Esperado: trae el nombre del proveedor (Planeta Mexico Infantil) y el total de piezas
#[tokio::test]
async fn pu_18_02_datos_del_proveedor() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 10).await;
    let id = c.devolucion(&v, &p, 7).await;
    c.evaluar(&j, id, true, "").await.exito();
    let t = String::from_utf8_lossy(&c.get(&j, &format!("/api/jefe/devoluciones/{id}/pdf/revistas")).await.bytes).to_string();
    assert!(t.contains("Planeta Mexico Infantil"), "falta el proveedor");
    assert!(t.contains('7'), "falta el total de piezas");
}

/// PU-18-03 · La devolución de revistas descuenta primero apartados y luego bodega
/// Entrada: revista con 1 apartada y 6 en bodega; devolución de 4 aprobada
/// Esperado: apartados 0, bodega 3; el acta se descarga
#[tokio::test]
async fn pu_18_03_apartados_y_bodega() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 6).await;
    sqlx::query("INSERT INTO apartados (codigo_ean, cantidad) VALUES (?, 1)")
        .bind(p.ean)
        .execute(&c.pool)
        .await
        .unwrap();
    let id = c.devolucion(&v, &p, 4).await;
    c.evaluar(&j, id, true, "").await.exito();
    assert_eq!((c.apartados(p.ean).await, c.bodega(p.ean).await), (0, 3));
    assert!(c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/revistas")).await.bytes.starts_with(b"%PDF"));
}

/// PU-18-04 · Sin acta si no está aprobada
/// Entrada: acta de revistas de una devolución PENDIENTE
/// Esperado: 409 "solo está disponible cuando está aprobada"
#[tokio::test]
async fn pu_18_04_pendiente() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/revistas"))
        .await
        .error(StatusCode::CONFLICT, "solo está disponible cuando está aprobada");
}

/// PU-18-05 · El acta de revistas no se emite como acta de libros
/// Entrada: devolución de revistas aprobada; GET .../pdf/libros
/// Esperado: 4xx (el tipo de la ruta debe coincidir con el de la devolución)
#[tokio::test]
async fn pu_18_05_tipo_de_acta_coincide() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let j = c.token(JEFE).await;
    let p = c.revista(PLANETA_INFANTIL, 95.0, 0, 5).await;
    let id = c.devolucion(&v, &p, 1).await;
    c.evaluar(&j, id, true, "").await.exito();
    let r = c.get(&v, &format!("/api/vendedor/devoluciones/{id}/pdf/libros")).await;
    assert!(r.status.is_client_error(), "generó un acta de libros para revistas ({})", r.status);
}
