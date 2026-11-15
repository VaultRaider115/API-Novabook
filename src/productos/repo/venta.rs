use sqlx::Row;

use crate::{
    db::{DbPool, row_get_f64},
    error::AppError,
    productos::{
        model::{ItemVenta, RegistrarVentaRequest, TipoProducto, VentaResponse},
        repo::{UBICACION_TIENDA, stock::{self, Tx}},
    },
};

pub struct VentaRepo;

impl VentaRepo {
    pub async fn registrar_venta_transaccional(
        pool: &DbPool,
        actor_id: i64,
        req: &RegistrarVentaRequest,
    ) -> Result<VentaResponse, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let venta = Self::registrar_en_tx(&mut tx, actor_id, req).await?;
        tx.commit().await.map_err(AppError::Database)?;
        Ok(venta)
    }

    /// Registra la venta dentro de una transacción ya abierta (la usa también el
    /// cambio de una devolución de cliente, que debe ser todo o nada).
    pub async fn registrar_en_tx(
        tx: &mut Tx<'_>,
        actor_id: i64,
        req: &RegistrarVentaRequest,
    ) -> Result<VentaResponse, AppError> {
        let nombre_cli = if req.cliente.trim().is_empty() {
            "Público en General"
        } else {
            req.cliente.trim()
        };
        let metodo_pago = req.metodo_pago.trim().to_uppercase();

        let insert_res = sqlx::query(
            "INSERT INTO ventas (id_usuarios, total, nombre_cliente, factura, metodo_pago) VALUES (?, 0.0, ?, ?, ?)",
        )
        .bind(actor_id)
        .bind(nombre_cli)
        .bind(&req.factura)
        .bind(&metodo_pago)
        .execute(&mut **tx)
        .await
        .map_err(AppError::Database)?;

        let id_venta = insert_res.last_insert_id() as i64;
        let mut total_venta = 0.0;
        let mut total_articulos = 0;
        let mut items_procesados = Vec::new();

        for item in &req.items {
            if item.cantidad <= 0 {
                return Err(AppError::BadRequest("La cantidad vendida debe ser mayor a 0".to_string()));
            }

            let row = sqlx::query(
                "SELECT p.precio, p.tipo_producto, COALESCE(l.nombre_libro, r.nombre_revista) AS nombre \
                 FROM productos p \
                 LEFT JOIN libros l ON l.codigo_ean = p.codigo_ean \
                 LEFT JOIN revistas r ON r.codigo_ean = p.codigo_ean \
                 WHERE p.codigo_ean = ?",
            )
            .bind(item.codigo_ean)
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("El producto con EAN {} no existe", item.codigo_ean)))?;

            let nombre: String = row.try_get("nombre").unwrap_or_else(|_| format!("EAN {}", item.codigo_ean));
            let precio = row_get_f64(&row, "precio");
            let tipo_str: String = row.get("tipo_producto");
            let tipo = if tipo_str.eq_ignore_ascii_case("revista") { TipoProducto::Revista } else { TipoProducto::Libro };

            let disponible = stock::stock_bloqueado(tx, item.codigo_ean, UBICACION_TIENDA).await?;
            if disponible < item.cantidad {
                return Err(AppError::BadRequest(format!(
                    "Stock insuficiente en tienda para '{}'. Disponible: {}, Solicitado: {}",
                    nombre, disponible, item.cantidad
                )));
            }
            stock::descontar(tx, item.codigo_ean, UBICACION_TIENDA, item.cantidad).await?;

            let subtotal = precio * (item.cantidad as f64);
            total_venta += subtotal;
            total_articulos += item.cantidad;

            sqlx::query(
                "INSERT INTO detalle_ventas (id_ventas, codigo_ean, cantidad, precio_unitario, subtotal) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id_venta)
            .bind(item.codigo_ean)
            .bind(item.cantidad)
            .bind(precio)
            .bind(subtotal)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;

            items_procesados.push(ItemVenta {
                codigo_ean: item.codigo_ean,
                tipo_producto: tipo,
                nombre_producto: nombre,
                cantidad: item.cantidad,
                precio_unitario: precio,
                subtotal,
            });
        }

        sqlx::query("UPDATE ventas SET total = ? WHERE id_ventas = ?")
            .bind(total_venta)
            .bind(id_venta)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;

        let fecha: String = sqlx::query("SELECT DATE_FORMAT(fecha_hora, '%Y-%m-%d %H:%i:%s') AS f FROM ventas WHERE id_ventas = ?")
            .bind(id_venta)
            .fetch_one(&mut **tx)
            .await
            .map_err(AppError::Database)?
            .get("f");

        Ok(VentaResponse {
            id_venta,
            id_vendedor: actor_id,
            cliente: nombre_cli.to_string(),
            metodo_pago,
            factura: req.factura.clone(),
            fecha_hora: fecha,
            total: total_venta,
            total_articulos,
            items: items_procesados,
            mensaje: "Venta registrada y procesada exitosamente".to_string(),
        })
    }
}
