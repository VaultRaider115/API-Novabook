//! Devolución de cliente con ticket (diagrama de procesos, paso 5).
//! Todo ocurre en una transacción: piezas a piso o a apartados, registro de la
//! devolución y, si es CAMBIO, la venta nueva.

use sqlx::Row;

use crate::{
    db::{DbPool, row_get_f64, row_get_i32, row_get_i64},
    error::AppError,
    productos::{
        model::{
            CrearDevolucionClienteRequest, DIAS_MAX_DEVOLUCION, DevolucionClienteResponse,
            ItemVentaRequest, PartidaDevolucionCliente, PartidaDevueltaResp, RegistrarVentaRequest,
            TicketItem, TicketResponse, TipoProducto,
        },
        repo::{
            UBICACION_TIENDA, VentaRepo,
            stock::{self, Tx},
        },
    },
};

pub struct DevolucionClienteRepo;

/// Redondeo a centavos (los importes se guardan en DECIMAL(10,2)).
fn centavos(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

impl DevolucionClienteRepo {
    /// Ticket con lo vendido y lo ya devuelto por producto. `None` si no existe.
    pub async fn buscar_ticket(pool: &DbPool, id_venta: i64) -> Result<Option<TicketResponse>, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let ticket = Self::cargar_ticket(&mut tx, id_venta, false).await?;
        tx.rollback().await.map_err(AppError::Database)?; // solo lectura
        Ok(ticket)
    }

    /// `bloquear = true` bloquea la venta (FOR UPDATE) para que dos devoluciones
    /// simultáneas del mismo ticket no superen lo vendido.
    async fn cargar_ticket(tx: &mut Tx<'_>, id_venta: i64, bloquear: bool) -> Result<Option<TicketResponse>, AppError> {
        // Consultas fijas: sqlx no acepta SQL armado en tiempo de ejecución.
        const SQL: &str = "SELECT v.id_ventas, DATE_FORMAT(v.fecha_hora, '%Y-%m-%d %H:%i:%s') AS fecha, \
             v.nombre_cliente, v.metodo_pago, v.total, DATEDIFF(NOW(), v.fecha_hora) AS dias \
             FROM ventas v WHERE v.id_ventas = ?";
        const SQL_BLOQUEO: &str = "SELECT v.id_ventas, DATE_FORMAT(v.fecha_hora, '%Y-%m-%d %H:%i:%s') AS fecha, \
             v.nombre_cliente, v.metodo_pago, v.total, DATEDIFF(NOW(), v.fecha_hora) AS dias \
             FROM ventas v WHERE v.id_ventas = ? FOR UPDATE";
        let Some(v) = sqlx::query(if bloquear { SQL_BLOQUEO } else { SQL })
            .bind(id_venta)
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::Database)?
        else {
            return Ok(None);
        };

        let filas = sqlx::query(
            "SELECT dv.codigo_ean, SUM(dv.cantidad) AS vendida, SUM(dv.subtotal) AS importe, \
               COALESCE(l.nombre_libro, r.nombre_revista, '') AS titulo, p.tipo_producto, \
               COALESCE((SELECT SUM(ddc.cantidad) FROM detalle_devolucion_cliente ddc \
                         JOIN devolucion_cliente dc ON dc.id_devolucion_cliente = ddc.id_devolucion_cliente \
                         WHERE dc.id_ventas = dv.id_ventas AND ddc.codigo_ean = dv.codigo_ean), 0) AS devuelta \
             FROM detalle_ventas dv \
             JOIN productos p ON p.codigo_ean = dv.codigo_ean \
             LEFT JOIN libros l ON l.codigo_ean = dv.codigo_ean \
             LEFT JOIN revistas r ON r.codigo_ean = dv.codigo_ean \
             WHERE dv.id_ventas = ? \
             GROUP BY dv.id_ventas, dv.codigo_ean, l.nombre_libro, r.nombre_revista, p.tipo_producto \
             ORDER BY MIN(dv.id_detalle)",
        )
        .bind(id_venta)
        .fetch_all(&mut **tx)
        .await
        .map_err(AppError::Database)?;

        let items = filas
            .iter()
            .map(|f| {
                let vendida = row_get_i32(f, "vendida");
                TicketItem {
                    codigo_ean: row_get_i64(f, "codigo_ean"),
                    titulo: f.get("titulo"),
                    tipo_producto: f.get("tipo_producto"),
                    cantidad_vendida: vendida,
                    cantidad_devuelta: row_get_i32(f, "devuelta"),
                    // Precio pagado (promedio si el EAN aparece en varios renglones).
                    precio_unitario: if vendida > 0 { centavos(row_get_f64(f, "importe") / vendida as f64) } else { 0.0 },
                }
            })
            .collect();

        let dias = row_get_i64(&v, "dias");
        Ok(Some(TicketResponse {
            id_venta: row_get_i64(&v, "id_ventas"),
            fecha_hora: v.get("fecha"),
            nombre_cliente: v.get("nombre_cliente"),
            metodo_pago: v.get("metodo_pago"),
            total: row_get_f64(&v, "total"),
            dias_transcurridos: dias,
            vigente: dias <= DIAS_MAX_DEVOLUCION,
            items,
        }))
    }

    /// `req.items` ya viene validado y agrupado por (EAN, con_defecto) desde el servicio.
    pub async fn registrar(
        pool: &DbPool,
        actor_id: i64,
        req: &CrearDevolucionClienteRequest,
        metodo_pago: &str,
    ) -> Result<DevolucionClienteResponse, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let ticket = Self::cargar_ticket(&mut tx, req.id_venta, true)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("El ticket #{} no existe", req.id_venta)))?;
        if !ticket.vigente {
            return Err(AppError::Conflict(format!(
                "El ticket #{} tiene {} días; solo se aceptan devoluciones de {} días o menos",
                ticket.id_venta, ticket.dias_transcurridos, DIAS_MAX_DEVOLUCION
            )));
        }

        // Validar contra lo vendido menos lo ya devuelto.
        let mut total_devuelto = 0.0;
        let mut devueltas = Vec::with_capacity(req.items.len());
        for p in &req.items {
            let linea = ticket.items.iter().find(|t| t.codigo_ean == p.codigo_ean).ok_or_else(|| {
                AppError::BadRequest(format!("El EAN {} no está en el ticket #{}", p.codigo_ean, ticket.id_venta))
            })?;
            let pedida: i32 = req.items.iter().filter(|x| x.codigo_ean == p.codigo_ean).map(|x| x.cantidad).sum();
            let disponible = linea.cantidad_vendida - linea.cantidad_devuelta;
            if pedida > disponible {
                return Err(AppError::BadRequest(format!(
                    "Del EAN {} se vendieron {} y ya se devolvieron {}; solo se pueden devolver {} más (se pidieron {})",
                    p.codigo_ean, linea.cantidad_vendida, linea.cantidad_devuelta, disponible.max(0), pedida
                )));
            }
            total_devuelto += linea.precio_unitario * p.cantidad as f64;
            devueltas.push(PartidaDevueltaResp {
                codigo_ean: p.codigo_ean,
                titulo: linea.titulo.clone(),
                cantidad: p.cantidad,
                precio_unitario: linea.precio_unitario,
                con_defecto: p.con_defecto,
            });
        }
        let total_devuelto = centavos(total_devuelto);
        let resolucion = req.resolucion.trim().to_uppercase();

        let id = sqlx::query(
            "INSERT INTO devolucion_cliente (id_ventas, id_usuarios, resolucion, total_devuelto, observaciones) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(req.id_venta)
        .bind(actor_id)
        .bind(&resolucion)
        .bind(total_devuelto)
        .bind(req.observaciones.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?
        .last_insert_id() as i64;

        // Piezas devueltas: sin defecto → piso; con defecto → apartados.
        for (p, d) in req.items.iter().zip(&devueltas) {
            sqlx::query(
                "INSERT INTO detalle_devolucion_cliente \
                 (id_devolucion_cliente, codigo_ean, cantidad, precio_unitario, con_defecto) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(p.codigo_ean)
            .bind(p.cantidad)
            .bind(d.precio_unitario)
            .bind(p.con_defecto)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            Self::reingresar(&mut tx, p).await?;
        }

        // CAMBIO: venta nueva en la misma transacción; debe ser de igual o mayor valor.
        let (mut total_nuevo, mut diferencia, mut id_venta_cambio) = (0.0, 0.0, None);
        if resolucion == "CAMBIO" {
            let venta = VentaRepo::registrar_en_tx(
                &mut tx,
                actor_id,
                &RegistrarVentaRequest {
                    cliente: ticket.nombre_cliente.clone(),
                    metodo_pago: metodo_pago.to_string(),
                    factura: None,
                    items: req
                        .items_cambio
                        .iter()
                        .map(|c| ItemVentaRequest {
                            codigo_ean: c.codigo_ean,
                            tipo_producto: TipoProducto::Libro, // el tipo real lo toma de la BD
                            cantidad: c.cantidad,
                        })
                        .collect(),
                },
            )
            .await?;
            total_nuevo = centavos(venta.total);
            if total_nuevo < total_devuelto {
                return Err(AppError::BadRequest(format!(
                    "El cambio debe ser por productos de igual o mayor precio: devuelve ${:.2} y se lleva ${:.2}",
                    total_devuelto, total_nuevo
                )));
            }
            diferencia = centavos(total_nuevo - total_devuelto);
            id_venta_cambio = Some(venta.id_venta);
            sqlx::query(
                "UPDATE devolucion_cliente SET id_venta_cambio = ?, diferencia_cobrada = ? \
                 WHERE id_devolucion_cliente = ?",
            )
            .bind(venta.id_venta)
            .bind(diferencia)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }

        let fecha: String = sqlx::query(
            "SELECT DATE_FORMAT(fecha_hora, '%Y-%m-%d %H:%i:%s') AS f FROM devolucion_cliente \
             WHERE id_devolucion_cliente = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::Database)?
        .get("f");

        tx.commit().await.map_err(AppError::Database)?;
        Ok(DevolucionClienteResponse {
            id_devolucion_cliente: id,
            id_venta: req.id_venta,
            reembolso: if resolucion == "REEMBOLSO" { total_devuelto } else { 0.0 },
            resolucion,
            fecha_hora: fecha,
            total_devuelto,
            total_nuevo,
            diferencia_cobrada: diferencia,
            id_venta_cambio,
            items: devueltas,
        })
    }

    async fn reingresar(tx: &mut Tx<'_>, p: &PartidaDevolucionCliente) -> Result<(), AppError> {
        if p.con_defecto {
            stock::sumar_apartado(tx, p.codigo_ean, p.cantidad).await
        } else {
            let mueble = stock::elegir_mueble(tx, p.codigo_ean, UBICACION_TIENDA, p.cantidad).await?;
            stock::sumar(tx, p.codigo_ean, mueble, p.cantidad).await
        }
    }
}
