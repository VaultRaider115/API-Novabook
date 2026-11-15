use sqlx::Row;

use crate::{
    db::{DbPool, row_get_f64, row_get_i32, row_get_i64},
    error::AppError,
    productos::{
        model::{CompraResponse, ItemCompraDto, RegistrarCompraRequest, TipoProducto},
        repo::UBICACION_BODEGA,
    },
};

pub struct CompraRepo;

impl CompraRepo {
    pub async fn registrar_compra_transaccional(
        pool: &DbPool,
        actor_id: i64,
        req: &RegistrarCompraRequest,
    ) -> Result<CompraResponse, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let prov_row = sqlx::query("SELECT nombre_proveedor FROM proveedor WHERE id_proveedor = ?")
            .bind(req.id_proveedor)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::NotFound(format!("Proveedor con ID {} no encontrado", req.id_proveedor))
            })?;
        let prov_nombre: String = prov_row.get("nombre_proveedor");

        let insert_res = sqlx::query(
            "INSERT INTO compras_proveedor (id_proveedor, id_usuarios, total, factura) VALUES (?, ?, 0.0, ?)",
        )
        .bind(req.id_proveedor)
        .bind(actor_id)
        .bind(&req.factura)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        let id_compra = insert_res.last_insert_id() as i64;
        let mut total_compra = 0.0;
        let mut total_articulos = 0;
        let mut items_procesados = Vec::new();

        for item in &req.items {
            if item.cantidad <= 0 {
                return Err(AppError::BadRequest("La cantidad comprada debe ser mayor a 0".to_string()));
            }
            if item.costo_unitario < 0.0 {
                return Err(AppError::BadRequest("El costo unitario no puede ser negativo".to_string()));
            }

            // El producto debe existir y ser del proveedor de la compra
            let prod = sqlx::query(
                "SELECT p.id_proveedor, COALESCE(l.nombre_libro, r.nombre_revista) AS nombre \
                 FROM productos p \
                 LEFT JOIN libros l ON l.codigo_ean = p.codigo_ean \
                 LEFT JOIN revistas r ON r.codigo_ean = p.codigo_ean \
                 WHERE p.codigo_ean = ? FOR UPDATE",
            )
            .bind(item.codigo_ean)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "El producto EAN {} no está registrado; regístralo primero en Alta en Almacén",
                    item.codigo_ean
                ))
            })?;

            if row_get_i32(&prod, "id_proveedor") != req.id_proveedor {
                return Err(AppError::BadRequest(format!(
                    "El producto EAN {} no pertenece al proveedor '{}'",
                    item.codigo_ean, prov_nombre
                )));
            }
            let nombre_prod: String = prod.try_get("nombre").unwrap_or_else(|_| format!("EAN {}", item.codigo_ean));

            // Mueble destino en bodega
            let id_mueble = match item.id_mueble {
                Some(m) => {
                    let ok = sqlx::query("SELECT 1 FROM mueble WHERE id_mueble = ? AND id_ubicacion = ?")
                        .bind(m)
                        .bind(UBICACION_BODEGA)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(AppError::Database)?;
                    if ok.is_none() {
                        return Err(AppError::BadRequest(format!("El mueble {} no es un mueble de bodega", m)));
                    }
                    m
                }
                None => crate::productos::repo::stock::elegir_mueble(&mut tx, item.codigo_ean, UBICACION_BODEGA, item.cantidad).await?,
            };

            sqlx::query(
                "INSERT INTO existencias (codigo_ean, id_mueble, cantidad) VALUES (?, ?, ?) \
                 ON DUPLICATE KEY UPDATE cantidad = cantidad + VALUES(cantidad)",
            )
            .bind(item.codigo_ean)
            .bind(id_mueble)
            .bind(item.cantidad)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;

            let subtotal = item.costo_unitario * (item.cantidad as f64);
            total_compra += subtotal;
            total_articulos += item.cantidad;

            let ins_det = sqlx::query(
                "INSERT INTO detalles_compras (id_compras_proveedor, codigo_ean, cantidad, subtotal) VALUES (?, ?, ?, ?)",
            )
            .bind(id_compra)
            .bind(item.codigo_ean)
            .bind(item.cantidad)
            .bind(subtotal)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;

            items_procesados.push(ItemCompraDto {
                id_detalle_compra: ins_det.last_insert_id() as i64,
                codigo_ean: item.codigo_ean,
                tipo_producto: item.tipo_producto,
                nombre_producto: nombre_prod,
                cantidad: item.cantidad,
                costo_unitario: item.costo_unitario,
                subtotal,
            });
        }

        sqlx::query("UPDATE compras_proveedor SET total = ? WHERE id_compras_proveedor = ?")
            .bind(total_compra)
            .bind(id_compra)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;

        let fecha: String = sqlx::query(
            "SELECT DATE_FORMAT(fecha_hora, '%Y-%m-%d %H:%i:%s') AS f FROM compras_proveedor WHERE id_compras_proveedor = ?",
        )
        .bind(id_compra)
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::Database)?
        .get("f");

        tx.commit().await.map_err(AppError::Database)?;

        Ok(CompraResponse {
            id_compra,
            id_proveedor: req.id_proveedor,
            nombre_proveedor: prov_nombre,
            id_usuarios: actor_id,
            comprador_nombre: None,
            fecha_hora: fecha,
            total: total_compra,
            total_articulos,
            observaciones: req.observaciones.clone(),
            factura: req.factura.clone(),
            items: items_procesados,
            mensaje: "Compra a proveedor registrada e inventario en bodega actualizado".to_string(),
        })
    }

    pub async fn listar_compras(pool: &DbPool) -> Result<Vec<CompraResponse>, AppError> {
        let rows = sqlx::query(
            "SELECT c.id_compras_proveedor AS id_compra, c.id_proveedor, p.nombre_proveedor, c.id_usuarios, \
                    CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, '')) AS comprador_nombre, \
                    DATE_FORMAT(c.fecha_hora, '%Y-%m-%d %H:%i:%s') AS fecha_hora, \
                    c.total, c.factura \
             FROM compras_proveedor c \
             JOIN proveedor p ON c.id_proveedor = p.id_proveedor \
             JOIN usuarios u ON c.id_usuarios = u.id_usuarios \
             ORDER BY c.id_compras_proveedor DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        let mut list = Vec::new();
        for r in rows {
            let id_compra = row_get_i64(&r, "id_compra");
            let det_rows = sqlx::query(
                "SELECT d.id_detalles_c, d.codigo_ean, d.cantidad, d.subtotal, pr.tipo_producto, \
                        COALESCE(l.nombre_libro, rv.nombre_revista) AS nombre \
                 FROM detalles_compras d \
                 JOIN productos pr ON pr.codigo_ean = d.codigo_ean \
                 LEFT JOIN libros l ON l.codigo_ean = d.codigo_ean \
                 LEFT JOIN revistas rv ON rv.codigo_ean = d.codigo_ean \
                 WHERE d.id_compras_proveedor = ?",
            )
            .bind(id_compra)
            .fetch_all(pool)
            .await
            .map_err(AppError::Database)?;

            let mut total_articulos = 0;
            let items: Vec<ItemCompraDto> = det_rows
                .into_iter()
                .map(|dr| {
                    let cant = row_get_i32(&dr, "cantidad");
                    total_articulos += cant;
                    let tipo_str: String = dr.get("tipo_producto");
                    let tipo = if tipo_str.eq_ignore_ascii_case("revista") {
                        TipoProducto::Revista
                    } else {
                        TipoProducto::Libro
                    };
                    let subtotal = row_get_f64(&dr, "subtotal");
                    let ean = row_get_i64(&dr, "codigo_ean");
                    ItemCompraDto {
                        id_detalle_compra: row_get_i64(&dr, "id_detalles_c"),
                        codigo_ean: ean,
                        tipo_producto: tipo,
                        nombre_producto: dr.try_get::<Option<String>, _>("nombre").ok().flatten().unwrap_or_else(|| format!("Artículo {}", ean)),
                        cantidad: cant,
                        costo_unitario: if cant > 0 { subtotal / cant as f64 } else { 0.0 },
                        subtotal,
                    }
                })
                .collect();

            list.push(CompraResponse {
                id_compra,
                id_proveedor: row_get_i32(&r, "id_proveedor"),
                nombre_proveedor: r.get("nombre_proveedor"),
                id_usuarios: row_get_i64(&r, "id_usuarios"),
                comprador_nombre: r.get("comprador_nombre"),
                fecha_hora: r.get("fecha_hora"),
                total: row_get_f64(&r, "total"),
                total_articulos,
                observaciones: None,
                factura: r.try_get::<Option<String>, _>("factura").ok().flatten(),
                items,
                mensaje: "OK".to_string(),
            });
        }

        Ok(list)
    }
}
