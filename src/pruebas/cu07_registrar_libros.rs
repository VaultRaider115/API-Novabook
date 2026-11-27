//! CU-07 · Registrar Libros (RF05, Alta de Mercancía). Actor: Bodega.
//! EAN-13 con dígito verificador y prefijo 978/979; SKU de 6 a 7 dígitos,
//! obligatorio y sin repetirse; solo EAN nuevos; precio mayor a $0.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::*;

fn libro(ean: i64, sku: Value, precio: f64) -> Value {
    json!({
        "codigo_ean": ean, "sku": sku, "nombre_libro": "Pedro Páramo (prueba)", "id_proveedor": PLANETA,
        "generos": [GENERO_PRUEBAS], "autores": [2], "precio": precio, "cantidad": 0, "id_mueble": MUEBLE_BODEGA
    })
}

/// PU-07-01 · Alta correcta de un libro nuevo
/// Entrada: EAN 978 nuevo y válido, SKU de 7 dígitos, título, proveedor Planeta, género y autor, precio $280, mueble de bodega
/// Esperado: 200; el producto queda como Libro con su precio, género y autor, y sin piezas (entran por Compras)
#[tokio::test]
async fn pu_07_01_alta_correcta() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let ean = ean_unico(978);
    c.post(&b, "/api/bodega/libros", libro(ean, json!(sku_unico()), 280.0)).await.exito();
    let tipo: String = sqlx::query_scalar("SELECT tipo_producto FROM productos WHERE codigo_ean = ?")
        .bind(ean)
        .fetch_one(&c.pool)
        .await
        .unwrap();
    assert_eq!(tipo, "Libro");
    assert_eq!(c.escalar_i64("SELECT COUNT(*) AS n FROM autores_has_libros WHERE codigo_ean = ?", ean).await, 1);
    assert_eq!(c.escalar_i64("SELECT COUNT(*) AS n FROM productos_has_genero WHERE codigo_ean = ?", ean).await, 1);
    assert_eq!(c.bodega(ean).await + c.piso(ean).await, 0);
}

/// PU-07-02 · Dígito verificador incorrecto
/// Entrada: un EAN válido con el último dígito cambiado
/// Esperado: 400 "Dígito verificador inválido" y no se registra
#[tokio::test]
async fn pu_07_02_digito_verificador_incorrecto() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let bueno = ean_unico(978);
    let malo = bueno - bueno % 10 + (bueno % 10 + 1) % 10;
    c.post(&b, "/api/bodega/libros", libro(malo, json!(sku_unico()), 100.0))
        .await
        .error(StatusCode::BAD_REQUEST, "Dígito verificador inválido");
    assert_eq!(c.escalar_i64("SELECT COUNT(*) AS n FROM productos WHERE codigo_ean = ?", malo).await, 0);
}

/// PU-07-03 · Prefijo que no es de libro
/// Entrada: EAN de revista (977) en el alta de libros; EAN con prefijo 123 (1234567890128)
/// Esperado: 400 "corresponde a una Revista"; 400 "no admitido"
#[tokio::test]
async fn pu_07_03_prefijo_incorrecto() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/libros", libro(ean_unico(977), json!(sku_unico()), 100.0))
        .await
        .error(StatusCode::BAD_REQUEST, "corresponde a una Revista");
    c.post(&b, "/api/bodega/libros", libro(1_234_567_890_128, json!(sku_unico()), 100.0))
        .await
        .error(StatusCode::BAD_REQUEST, "no admitido");
}

/// PU-07-04 · EAN que ya está registrado
/// Entrada: EAN 9786073832014 (Cien Años de Soledad, de la semilla)
/// Esperado: 409 "ya está registrado como … usa Compras a Proveedor"
#[tokio::test]
async fn pu_07_04_ean_ya_registrado() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let r = c
        .post(&b, "/api/bodega/libros", libro(9_786_073_832_014, json!(sku_unico()), 100.0))
        .await
        .error(StatusCode::CONFLICT, "ya está registrado");
    assert!(r.mensaje().contains("Compras a Proveedor"), "{}", r.mensaje());
}

/// PU-07-05 · Reglas del SKU
/// Entrada: sin SKU; SKU de 5 dígitos (12345); SKU repetido (el de un libro ya registrado)
/// Esperado: 400 "El SKU es obligatorio"; 400 "debe tener de 6 a 7 dígitos"; 409 "ya está asignado"
#[tokio::test]
async fn pu_07_05_reglas_del_sku() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/libros", libro(ean_unico(978), Value::Null, 100.0))
        .await
        .error(StatusCode::BAD_REQUEST, "El SKU es obligatorio");
    c.post(&b, "/api/bodega/libros", libro(ean_unico(978), json!(12345), 100.0))
        .await
        .error(StatusCode::BAD_REQUEST, "de 6 a 7 dígitos");
    let existente = c.libro(PLANETA, 100.0, 0, 0).await;
    c.post(&b, "/api/bodega/libros", libro(ean_unico(978), json!(existente.sku), 100.0))
        .await
        .error(StatusCode::CONFLICT, "ya está asignado");
}

/// PU-07-06 · Precio de venta
/// Entrada: precio $0 y precio $100,000 (tope $99,999.99)
/// Esperado: 400 "debe ser mayor a $0" y 400 "no puede pasar de $99999.99"
#[tokio::test]
async fn pu_07_06_precio_de_venta() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    c.post(&b, "/api/bodega/libros", libro(ean_unico(978), json!(sku_unico()), 0.0))
        .await
        .error(StatusCode::BAD_REQUEST, "debe ser mayor a $0");
    c.post(&b, "/api/bodega/libros", libro(ean_unico(978), json!(sku_unico()), 100_000.0))
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de $");
}

/// PU-07-07 · Título demasiado largo
/// Entrada: título de 151 caracteres (límite 150)
/// Esperado: 400 "no puede pasar de 150 caracteres"
#[tokio::test]
async fn pu_07_07_titulo_largo() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let mut l = libro(ean_unico(978), json!(sku_unico()), 100.0);
    l["nombre_libro"] = json!("T".repeat(151));
    c.post(&b, "/api/bodega/libros", l)
        .await
        .error(StatusCode::BAD_REQUEST, "no puede pasar de 150 caracteres");
}

/// PU-07-08 · Mueble que no es de bodega
/// Entrada: alta con el mueble de piso P-900
/// Esperado: 400 "no pertenece a la ubicación"
#[tokio::test]
async fn pu_07_08_mueble_de_piso() {
    let c = ctx().await;
    let b = c.token(BODEGA).await;
    let mut l = libro(ean_unico(978), json!(sku_unico()), 100.0);
    l["id_mueble"] = json!(MUEBLE_PISO);
    l["cantidad"] = json!(1);
    c.post(&b, "/api/bodega/libros", l)
        .await
        .error(StatusCode::BAD_REQUEST, "no pertenece a la ubicación");
}

/// PU-07-09 · Solo Bodega da de alta
/// Entrada: el Vendedor y el Jefe hacen POST /api/bodega/libros
/// Esperado: 403 para ambos
#[tokio::test]
async fn pu_07_09_solo_bodega() {
    let c = ctx().await;
    for usuario in [VENDEDOR, JEFE] {
        let t = c.token(usuario).await;
        let r = c.post(&t, "/api/bodega/libros", libro(ean_unico(978), json!(sku_unico()), 100.0)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{}: {}", usuario.0, r.json);
    }
}
