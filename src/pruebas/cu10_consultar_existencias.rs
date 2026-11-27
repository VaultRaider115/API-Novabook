//! CU-10 · Consultar existencias (RF10). Actores: Vendedor y Bodega (y el
//! Jefe al capturar solicitudes de editorial). Piso, bodega y total por título.

use axum::http::StatusCode;
use serde_json::Value;

use super::*;

fn fila(r: &Resp, ean: i64) -> Option<&Value> {
    r.data().as_array().unwrap().iter().find(|x| x["codigo_ean"] == ean)
}

/// PU-10-01 · Búsqueda por EAN
/// Entrada: libro con 7 piezas en piso y 12 en bodega, precio $310; GET /api/vendedor/existencias?q=<EAN>
/// Esperado: 200 con piso 7, bodega 12, total 19, precio $310, tipo Libro y su proveedor
#[tokio::test]
async fn pu_10_01_busqueda_por_ean() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let p = c.libro(PLANETA, 310.0, 7, 12).await;
    let r = c.get(&v, &format!("/api/vendedor/existencias?q={}", p.ean)).await.exito();
    let f = fila(&r, p.ean).expect("no aparece el producto");
    assert_eq!(f["stock_tienda"], 7);
    assert_eq!(f["stock_bodega"], 12);
    assert_eq!(f["stock_total"], 19);
    assert_eq!(f["precio"], 310.0);
    assert_eq!(f["id_proveedor"], PLANETA);
    assert_eq!(f["titulo"], p.titulo.as_str());
}

/// PU-10-02 · Búsqueda por título
/// Entrada: parte del título de una revista de prueba ("Revista de prueba <EAN>")
/// Esperado: 200 y la revista aparece con tipo Revista
#[tokio::test]
async fn pu_10_02_busqueda_por_titulo() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let p = c.revista(PLANETA_INFANTIL, 90.0, 2, 3).await;
    let q = format!("de prueba {}", p.ean);
    let r = c.get(&b, &format!("/api/bodega/existencias?q={}", q.replace(' ', "%20"))).await.exito();
    let f = fila(&r, p.ean).expect("no aparece la revista");
    assert!(f["tipo_producto"].as_str().unwrap().eq_ignore_ascii_case("revista"));
}

/// PU-10-03 · Filtro por tipo
/// Entrada: ?tipo=revista
/// Esperado: 200 y todas las filas son revistas
#[tokio::test]
async fn pu_10_03_filtro_por_tipo() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    c.revista(PLANETA_INFANTIL, 90.0, 1, 1).await;
    let r = c.get(&v, "/api/vendedor/existencias?tipo=revista").await.exito();
    let filas = r.data().as_array().unwrap();
    assert!(!filas.is_empty());
    assert!(filas.iter().all(|x| x["tipo_producto"].as_str().unwrap().eq_ignore_ascii_case("revista")));
}

/// PU-10-04 · Sin resultados
/// Entrada: búsqueda de un EAN válido que no existe
/// Esperado: 200 con lista vacía
#[tokio::test]
async fn pu_10_04_sin_resultados() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let r = c.get(&v, &format!("/api/vendedor/existencias?q={}", ean_unico(978))).await.exito();
    assert!(r.data().as_array().unwrap().is_empty());
}

/// PU-10-05 · Límites de la búsqueda
/// Entrada: texto de 31 caracteres (límite 30); EAN de 14 dígitos
/// Esperado: 400 "no puede pasar de 30 caracteres"; 400 "Un EAN tiene 13 dígitos"
#[tokio::test]
async fn pu_10_05_limites_de_busqueda() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    c.get(&v, &format!("/api/vendedor/existencias?q={}", "a".repeat(31)))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 30 caracteres");
    c.get(&v, "/api/vendedor/existencias?q=97860738320141")
        .await
        .error(StatusCode::BAD_REQUEST, "Un EAN tiene 13 dígitos");
}

/// PU-10-06 · Búsqueda sin inyección SQL
/// Entrada: q = "' OR 1=1 --"
/// Esperado: 200 con lista vacía (el texto se busca literal; no devuelve todo el catálogo)
#[tokio::test]
async fn pu_10_06_sin_inyeccion_sql() {
    let c = ctx().await;
    let v = c.token(VENDEDOR).await;
    let r = c.get(&v, "/api/vendedor/existencias?q=%27%20OR%201%3D1%20--").await.exito();
    assert!(r.data().as_array().unwrap().is_empty(), "{}", r.json);
}

/// PU-10-07 · Quién consulta
/// Entrada: Vendedor, Bodega y Jefe en su ruta de existencias; el Gerente en la del Vendedor
/// Esperado: 200 para los tres primeros; 403 para el Gerente
#[tokio::test]
async fn pu_10_07_permisos() {
    let c = ctx().await;
    for (usuario, ruta) in [
        (VENDEDOR, "/api/vendedor/existencias"),
        (BODEGA, "/api/bodega/existencias"),
        (JEFE, "/api/jefe/existencias"),
    ] {
        let t = c.token(usuario).await;
        c.get(&t, ruta).await.exito();
    }
    let g = c.token(GERENTE).await;
    let r = c.get(&g, "/api/vendedor/existencias").await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.json);
}
