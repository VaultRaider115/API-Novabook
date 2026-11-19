//! Helpers de existencias por mueble (esquema unificado). Todos operan dentro de una transacción.

use crate::{
    db::row_get_i32,
    error::AppError,
};

pub type Tx<'a> = sqlx::Transaction<'a, sqlx::MySql>;

pub fn nombre_ubicacion(id_ubicacion: i32) -> &'static str {
    if id_ubicacion == super::UBICACION_BODEGA { "Bodega" } else { "Piso de Venta" }
}

/// Bloquea las filas de existencias del producto en la ubicación y devuelve el total disponible.
pub async fn stock_bloqueado(tx: &mut Tx<'_>, codigo_ean: i64, id_ubicacion: i32) -> Result<i32, AppError> {
    let rows = sqlx::query(
        "SELECT e.cantidad FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
         WHERE e.codigo_ean = ? AND m.id_ubicacion = ? FOR UPDATE",
    )
    .bind(codigo_ean)
    .bind(id_ubicacion)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::Database)?;
    Ok(rows.iter().map(|r| row_get_i32(r, "cantidad")).sum())
}

/// Descuenta `cantidad` repartiéndola entre los muebles de la ubicación (primero los de más piezas).
/// Devuelve los pares (id_mueble, piezas descontadas). El llamador debe haber verificado el stock.
pub async fn descontar(
    tx: &mut Tx<'_>,
    codigo_ean: i64,
    id_ubicacion: i32,
    cantidad: i32,
) -> Result<Vec<(i32, i32)>, AppError> {
    let rows = sqlx::query(
        "SELECT e.id_mueble, e.cantidad FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
         WHERE e.codigo_ean = ? AND m.id_ubicacion = ? AND e.cantidad > 0 \
         ORDER BY e.cantidad DESC FOR UPDATE",
    )
    .bind(codigo_ean)
    .bind(id_ubicacion)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::Database)?;

    let mut pendiente = cantidad;
    let mut usados = Vec::new();
    for r in rows {
        if pendiente == 0 {
            break;
        }
        let mueble = row_get_i32(&r, "id_mueble");
        let tiene = row_get_i32(&r, "cantidad");
        let toma = tiene.min(pendiente);
        sqlx::query("UPDATE existencias SET cantidad = cantidad - ? WHERE codigo_ean = ? AND id_mueble = ?")
            .bind(toma)
            .bind(codigo_ean)
            .bind(mueble)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
        usados.push((mueble, toma));
        pendiente -= toma;
    }
    if pendiente > 0 {
        return Err(AppError::BadRequest("Stock insuficiente".to_string()));
    }
    Ok(usados)
}

pub async fn sumar(tx: &mut Tx<'_>, codigo_ean: i64, id_mueble: i32, cantidad: i32) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO existencias (codigo_ean, id_mueble, cantidad) VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE cantidad = cantidad + VALUES(cantidad)",
    )
    .bind(codigo_ean)
    .bind(id_mueble)
    .bind(cantidad)
    .execute(&mut **tx)
    .await
    .map_err(AppError::Database)?;
    Ok(())
}

/// Mueble de la ubicación que recibirá `cantidad` piezas: el que ya contiene el producto,
/// luego uno del mismo género y por último cualquiera con espacio.
pub async fn elegir_mueble(
    tx: &mut Tx<'_>,
    codigo_ean: i64,
    id_ubicacion: i32,
    cantidad: i32,
) -> Result<i32, AppError> {
    let row = sqlx::query(
        "SELECT m.id_mueble, \
           m.capacidad - COALESCE((SELECT SUM(e2.cantidad) FROM existencias e2 WHERE e2.id_mueble = m.id_mueble), 0) AS libre, \
           (SELECT COUNT(*) FROM existencias e WHERE e.id_mueble = m.id_mueble AND e.codigo_ean = ?) AS tiene, \
           (SELECT COUNT(*) FROM productos_has_genero pg WHERE pg.codigo_ean = ? AND pg.id_genero = m.id_genero) AS genero_ok \
         FROM mueble m WHERE m.id_ubicacion = ? \
         HAVING libre >= ? \
         ORDER BY tiene DESC, genero_ok DESC, libre DESC LIMIT 1",
    )
    .bind(codigo_ean)
    .bind(codigo_ean)
    .bind(id_ubicacion)
    .bind(cantidad)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::Database)?;

    row.map(|r| row_get_i32(&r, "id_mueble")).ok_or_else(|| {
        AppError::Conflict(format!(
            "No hay un mueble en {} con espacio para {} pieza(s) del producto {}",
            nombre_ubicacion(id_ubicacion), cantidad, codigo_ean
        ))
    })
}

// ----------------------------------------------------------------------------
// Stock "Apartado": libros con defecto que regresó un cliente (tabla apartados).
// No se venden ni se trasladan; solo salen al devolverlos a la editorial.
// ----------------------------------------------------------------------------

/// Piezas apartadas de un producto, bloqueando la fila para la transacción.
pub async fn apartado_bloqueado(tx: &mut Tx<'_>, codigo_ean: i64) -> Result<i32, AppError> {
    let fila = sqlx::query("SELECT cantidad FROM apartados WHERE codigo_ean = ? FOR UPDATE")
        .bind(codigo_ean)
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::Database)?;
    Ok(fila.map(|r| crate::db::row_get_i32(&r, "cantidad")).unwrap_or(0))
}

pub async fn sumar_apartado(tx: &mut Tx<'_>, codigo_ean: i64, cantidad: i32) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO apartados (codigo_ean, cantidad) VALUES (?, ?) \
         ON DUPLICATE KEY UPDATE cantidad = cantidad + VALUES(cantidad)",
    )
    .bind(codigo_ean)
    .bind(cantidad)
    .execute(&mut **tx)
    .await
    .map_err(AppError::Database)?;
    Ok(())
}

/// Descuenta hasta `cantidad` piezas apartadas y devuelve cuántas descontó.
pub async fn descontar_apartado(tx: &mut Tx<'_>, codigo_ean: i64, cantidad: i32) -> Result<i32, AppError> {
    let hay = apartado_bloqueado(tx, codigo_ean).await?;
    let quitar = hay.min(cantidad).max(0);
    if quitar > 0 {
        sqlx::query("UPDATE apartados SET cantidad = cantidad - ? WHERE codigo_ean = ?")
            .bind(quitar)
            .bind(codigo_ean)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
    }
    Ok(quitar)
}

/// Piezas de un producto en un mueble concreto, bloqueando la fila.
pub async fn en_mueble_bloqueado(tx: &mut Tx<'_>, codigo_ean: i64, id_mueble: i32) -> Result<i32, AppError> {
    let fila = sqlx::query("SELECT cantidad FROM existencias WHERE codigo_ean = ? AND id_mueble = ? FOR UPDATE")
        .bind(codigo_ean)
        .bind(id_mueble)
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::Database)?;
    Ok(fila.map(|r| row_get_i32(&r, "cantidad")).unwrap_or(0))
}

/// Descuenta piezas de un mueble concreto. El llamador verifica que alcancen.
pub async fn descontar_de_mueble(tx: &mut Tx<'_>, codigo_ean: i64, id_mueble: i32, cantidad: i32) -> Result<(), AppError> {
    sqlx::query("UPDATE existencias SET cantidad = cantidad - ? WHERE codigo_ean = ? AND id_mueble = ? AND cantidad >= ?")
        .bind(cantidad)
        .bind(codigo_ean)
        .bind(id_mueble)
        .bind(cantidad)
        .execute(&mut **tx)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}

/// Espacio libre de un mueble (`None` si no tiene capacidad definida).
pub async fn libre_en_mueble(tx: &mut Tx<'_>, id_mueble: i32) -> Result<Option<i32>, AppError> {
    let fila = sqlx::query(
        "SELECT m.capacidad - COALESCE((SELECT SUM(e.cantidad) FROM existencias e WHERE e.id_mueble = m.id_mueble), 0) AS libre \
         FROM mueble m WHERE m.id_mueble = ?",
    )
    .bind(id_mueble)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::Database)?;
    Ok(crate::db::row_get_opt_i32(&fila, "libre"))
}
