//! Inventario mensual del piso de venta y solicitudes de editorial.

use sqlx::Row;

use crate::{
    db::{DbPool, row_get_i32, row_get_i64},
    error::AppError,
    productos::{
        model::{
            DetalleInventarioItem, DevolucionPendienteItem, InventarioDetalle, InventarioResumen, MOTIVO_ANEJAMIENTO,
            MOTIVO_SOLICITUD,
            SolicitudEditorial, SolicitudEditorialItem,
        },
        repo::{
            UBICACION_TIENDA,
            stock::{self, Tx},
        },
    },
};

pub struct InventarioMensualRepo;

// Las consultas armadas con format!() solo concatenan constantes de este archivo
// (sin datos del usuario), por eso se envuelven en sqlx::AssertSqlSafe.
/// Stock de piso de un producto (subconsulta reutilizable sobre `di.codigo_ean`).
const PISO_SQL: &str = "(SELECT COALESCE(SUM(e.cantidad), 0) FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
                         WHERE e.codigo_ean = di.codigo_ean AND m.id_ubicacion = 1)";

const RESUMEN_SELECT: &str = "SELECT i.id_inventario, CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, '')) AS contado_por, \
    DATE_FORMAT(i.fecha_inicio, '%Y-%m-%d %H:%i') AS fecha_inicio, DATE_FORMAT(i.fecha_cierre, '%Y-%m-%d %H:%i') AS fecha_cierre, \
    i.estado, i.dias_anejamiento, CONCAT(r.nombre, ' ', COALESCE(r.apellido_paterno, '')) AS revisado_por, \
    DATE_FORMAT(i.fecha_revision, '%Y-%m-%d %H:%i') AS fecha_revision, i.observaciones, \
    (SELECT COUNT(*) FROM detalle_inventario d WHERE d.id_inventario = i.id_inventario) AS total_productos, \
    (SELECT COALESCE(SUM(d.cantidad_contada), 0) FROM detalle_inventario d WHERE d.id_inventario = i.id_inventario) AS piezas_contadas, \
    (SELECT COUNT(*) FROM detalle_inventario d WHERE d.id_inventario = i.id_inventario \
       AND d.stock_sistema IS NOT NULL AND d.stock_sistema <> d.cantidad_contada) AS con_diferencia, \
    (SELECT COUNT(*) FROM detalle_inventario d WHERE d.id_inventario = i.id_inventario \
       AND d.motivo_devolucion IS NOT NULL) AS a_devolucion \
    FROM inventario i JOIN usuarios u ON u.id_usuarios = i.id_usuarios \
    LEFT JOIN usuarios r ON r.id_usuarios = i.id_usuario_revisa";

fn mapear_resumen(r: &sqlx::mysql::MySqlRow) -> InventarioResumen {
    InventarioResumen {
        id_inventario: row_get_i64(r, "id_inventario"),
        contado_por: r.get::<String, _>("contado_por").trim().to_string(),
        fecha_inicio: r.get("fecha_inicio"),
        fecha_cierre: r.get("fecha_cierre"),
        estado: r.get("estado"),
        dias_anejamiento: row_get_i32(r, "dias_anejamiento"),
        revisado_por: r.get::<Option<String>, _>("revisado_por").map(|s| s.trim().to_string()),
        fecha_revision: r.get("fecha_revision"),
        observaciones: r.get("observaciones"),
        total_productos: row_get_i64(r, "total_productos"),
        piezas_contadas: row_get_i64(r, "piezas_contadas"),
        con_diferencia: row_get_i64(r, "con_diferencia"),
        a_devolucion: row_get_i64(r, "a_devolucion"),
    }
}

impl InventarioMensualRepo {
    pub async fn listar(pool: &DbPool) -> Result<Vec<InventarioResumen>, AppError> {
        let sql = sqlx::AssertSqlSafe(format!("{RESUMEN_SELECT} ORDER BY i.id_inventario DESC"));
        let filas = sqlx::query(sql).fetch_all(pool).await.map_err(AppError::Database)?;
        Ok(filas.iter().map(mapear_resumen).collect())
    }

    /// Títulos que van a devolución con su stock y lo contado en este
    /// inventario. Misma regla que el cierre: lo que pidió una editorial
    /// (solicitud ABIERTA, con cualquier stock) o lo añejado (sin ventas ni
    /// compras en `dias` días) que hay en piso o ya se contó. Incluye los que el
    /// vendedor todavía no escanea, para que sepa qué le falta encontrar.
    pub async fn pendientes_devolucion(
        pool: &DbPool,
        id: i64,
        dias: i32,
    ) -> Result<Vec<DevolucionPendienteItem>, AppError> {
        let filas = sqlx::query(
            "SELECT * FROM ( \
               SELECT p.codigo_ean, COALESCE(l.nombre_libro, rv.nombre_revista, '') AS titulo, pr.nombre_proveedor, \
                 COALESCE((SELECT SUM(e.cantidad) FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
                           WHERE e.codigo_ean = p.codigo_ean AND m.id_ubicacion = 1), 0) AS en_piso, \
                 COALESCE((SELECT SUM(e.cantidad) FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
                           WHERE e.codigo_ean = p.codigo_ean AND m.id_ubicacion = 2), 0) AS en_bodega, \
                 COALESCE((SELECT a.cantidad FROM apartados a WHERE a.codigo_ean = p.codigo_ean), 0) AS apartadas, \
                 COALESCE((SELECT di.cantidad_contada FROM detalle_inventario di \
                           WHERE di.id_inventario = ? AND di.codigo_ean = p.codigo_ean), 0) AS encontradas, \
                 CASE \
                   WHEN EXISTS (SELECT 1 FROM detalle_solicitud_editorial ds \
                        JOIN solicitud_editorial s ON s.id_solicitud = ds.id_solicitud \
                        WHERE ds.codigo_ean = p.codigo_ean AND s.estado = 'ABIERTA') THEN ? \
                   WHEN NOT EXISTS (SELECT 1 FROM detalle_ventas dv JOIN ventas v ON v.id_ventas = dv.id_ventas \
                        WHERE dv.codigo_ean = p.codigo_ean AND v.fecha_hora >= NOW() - INTERVAL ? DAY) \
                    AND NOT EXISTS (SELECT 1 FROM detalles_compras dc \
                        JOIN compras_proveedor c ON c.id_compras_proveedor = dc.id_compras_proveedor \
                        WHERE dc.codigo_ean = p.codigo_ean AND c.fecha_hora >= NOW() - INTERVAL ? DAY) THEN ? \
                 END AS motivo_devolucion \
               FROM productos p \
               JOIN proveedor pr ON pr.id_proveedor = p.id_proveedor \
               LEFT JOIN libros l ON l.codigo_ean = p.codigo_ean \
               LEFT JOIN revistas rv ON rv.codigo_ean = p.codigo_ean \
             ) t \
             WHERE (t.motivo_devolucion = ? AND t.en_piso + t.en_bodega + t.apartadas > 0) \
                OR (t.motivo_devolucion = ? AND (t.en_piso > 0 OR t.encontradas > 0)) \
             ORDER BY t.titulo",
        )
        .bind(id)
        .bind(MOTIVO_SOLICITUD)
        .bind(dias)
        .bind(dias)
        .bind(MOTIVO_ANEJAMIENTO)
        .bind(MOTIVO_SOLICITUD)
        .bind(MOTIVO_ANEJAMIENTO)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;
        Ok(filas
            .iter()
            .map(|f| DevolucionPendienteItem {
                codigo_ean: row_get_i64(f, "codigo_ean"),
                titulo: f.get("titulo"),
                proveedor: f.get("nombre_proveedor"),
                motivo_devolucion: f.get("motivo_devolucion"),
                en_piso: row_get_i32(f, "en_piso"),
                en_bodega: row_get_i32(f, "en_bodega"),
                apartadas: row_get_i32(f, "apartadas"),
                encontradas: row_get_i32(f, "encontradas"),
                // Los totales los calcula el servicio (armar_pendientes).
                a_devolver: 0,
                faltantes: 0,
            })
            .collect())
    }

    /// Id del inventario ABIERTO (solo puede haber uno a la vez).
    pub async fn abierto(pool: &DbPool) -> Result<Option<i64>, AppError> {
        let fila = sqlx::query("SELECT id_inventario FROM inventario WHERE estado = 'ABIERTO' ORDER BY id_inventario LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(AppError::Database)?;
        Ok(fila.map(|r| row_get_i64(&r, "id_inventario")))
    }

    pub async fn obtener(pool: &DbPool, id: i64) -> Result<Option<InventarioDetalle>, AppError> {
        let sql = sqlx::AssertSqlSafe(format!("{RESUMEN_SELECT} WHERE i.id_inventario = ?"));
        let Some(r) = sqlx::query(sql).bind(id).fetch_optional(pool).await.map_err(AppError::Database)? else {
            return Ok(None);
        };
        let resumen = mapear_resumen(&r);
        let filas = sqlx::query(
            "SELECT di.codigo_ean, COALESCE(l.nombre_libro, rv.nombre_revista, '') AS titulo, p.tipo_producto, \
               pr.nombre_proveedor, di.cantidad_contada, di.stock_sistema, di.stock_apartado, \
               COALESCE(di.motivo_devolucion, CASE WHEN i.estado = 'ABIERTO' THEN CASE \
                 WHEN EXISTS (SELECT 1 FROM detalle_solicitud_editorial ds \
                      JOIN solicitud_editorial s ON s.id_solicitud = ds.id_solicitud \
                      WHERE ds.codigo_ean = di.codigo_ean AND s.estado = 'ABIERTA') THEN ? \
                 WHEN di.cantidad_contada > 0 \
                  AND NOT EXISTS (SELECT 1 FROM detalle_ventas dv JOIN ventas v ON v.id_ventas = dv.id_ventas \
                      WHERE dv.codigo_ean = di.codigo_ean AND v.fecha_hora >= NOW() - INTERVAL i.dias_anejamiento DAY) \
                  AND NOT EXISTS (SELECT 1 FROM detalles_compras dc \
                      JOIN compras_proveedor c ON c.id_compras_proveedor = dc.id_compras_proveedor \
                      WHERE dc.codigo_ean = di.codigo_ean AND c.fecha_hora >= NOW() - INTERVAL i.dias_anejamiento DAY) THEN ? \
               END END) AS motivo_devolucion \
             FROM detalle_inventario di \
             JOIN inventario i ON i.id_inventario = di.id_inventario \
             JOIN productos p ON p.codigo_ean = di.codigo_ean \
             JOIN proveedor pr ON pr.id_proveedor = p.id_proveedor \
             LEFT JOIN libros l ON l.codigo_ean = di.codigo_ean \
             LEFT JOIN revistas rv ON rv.codigo_ean = di.codigo_ean \
             WHERE di.id_inventario = ? \
             ORDER BY (di.motivo_devolucion IS NULL), pr.nombre_proveedor, titulo",
        )
        // Con el inventario ABIERTO el motivo aún no se guarda: se calcula con la
        // regla del cierre para que el vendedor separe esos títulos al contar.
        .bind(MOTIVO_SOLICITUD)
        .bind(MOTIVO_ANEJAMIENTO)
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;
        let items = filas
            .iter()
            .map(|f| {
                let contada = row_get_i32(f, "cantidad_contada");
                let sistema: Option<i32> = f.try_get::<Option<i32>, _>("stock_sistema").ok().flatten();
                DetalleInventarioItem {
                    codigo_ean: row_get_i64(f, "codigo_ean"),
                    titulo: f.get("titulo"),
                    tipo_producto: f.get("tipo_producto"),
                    proveedor: f.get("nombre_proveedor"),
                    cantidad_contada: contada,
                    stock_sistema: sistema,
                    diferencia: sistema.map(|s| contada - s),
                    stock_apartado: row_get_i32(f, "stock_apartado"),
                    motivo_devolucion: f.get("motivo_devolucion"),
                }
            })
            .collect();
        Ok(Some(InventarioDetalle { resumen, items }))
    }

    pub async fn abrir(pool: &DbPool, actor_id: i64, dias_anejamiento: i32) -> Result<i64, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let abierto = sqlx::query("SELECT id_inventario FROM inventario WHERE estado = 'ABIERTO' LIMIT 1 FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        if let Some(r) = abierto {
            return Err(AppError::Conflict(format!(
                "Ya hay un inventario abierto (#{}); continúa ese conteo",
                row_get_i64(&r, "id_inventario")
            )));
        }
        let id = sqlx::query("INSERT INTO inventario (id_usuarios, dias_anejamiento) VALUES (?, ?)")
            .bind(actor_id)
            .bind(dias_anejamiento)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .last_insert_id() as i64;
        tx.commit().await.map_err(AppError::Database)?;
        Ok(id)
    }

    async fn estado_bloqueado(tx: &mut Tx<'_>, id: i64) -> Result<String, AppError> {
        let fila = sqlx::query("SELECT estado FROM inventario WHERE id_inventario = ? FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("El inventario #{} no existe", id)))?;
        Ok(fila.get("estado"))
    }

    pub async fn registrar_conteo(pool: &DbPool, id: i64, codigo_ean: i64, cantidad: i32) -> Result<(), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let estado = Self::estado_bloqueado(&mut tx, id).await?;
        if estado != "ABIERTO" {
            return Err(AppError::Conflict(format!("El inventario #{} ya está {}; no se puede contar", id, estado)));
        }
        let existe = sqlx::query("SELECT 1 FROM productos WHERE codigo_ean = ?")
            .bind(codigo_ean)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        if existe.is_none() {
            return Err(AppError::NotFound(format!("El producto con EAN {} no existe", codigo_ean)));
        }
        sqlx::query(
            "INSERT INTO detalle_inventario (id_inventario, codigo_ean, cantidad_contada) VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE cantidad_contada = VALUES(cantidad_contada)",
        )
        .bind(id)
        .bind(codigo_ean)
        .bind(cantidad)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
        tx.commit().await.map_err(AppError::Database)?;
        Ok(())
    }

    /// Cierra el conteo: agrega con 0 lo que hay en piso o apartado y no se escaneó,
    /// guarda el stock del sistema y marca qué va a devolución.
    pub async fn cerrar(pool: &DbPool, id: i64) -> Result<(), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let estado = Self::estado_bloqueado(&mut tx, id).await?;
        if estado != "ABIERTO" {
            return Err(AppError::Conflict(format!("El inventario #{} ya está {}", id, estado)));
        }

        // Lo que el sistema dice que hay en piso o apartado y no se escaneó cuenta como 0.
        sqlx::query(
            "INSERT IGNORE INTO detalle_inventario (id_inventario, codigo_ean, cantidad_contada) \
             SELECT ?, x.codigo_ean, 0 FROM ( \
               SELECT e.codigo_ean FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
                WHERE m.id_ubicacion = 1 GROUP BY e.codigo_ean HAVING SUM(e.cantidad) > 0 \
               UNION SELECT a.codigo_ean FROM apartados a WHERE a.cantidad > 0) x",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        let sql = sqlx::AssertSqlSafe(format!(
            "UPDATE detalle_inventario di SET di.stock_sistema = {PISO_SQL}, \
             di.stock_apartado = COALESCE((SELECT a.cantidad FROM apartados a WHERE a.codigo_ean = di.codigo_ean), 0) \
             WHERE di.id_inventario = ?"
        ));
        sqlx::query(sql).bind(id).execute(&mut *tx).await.map_err(AppError::Database)?;

        // Paso 7: a devolución si la editorial lo pidió (solicitud ABIERTA) o si está
        // añejado (sin ventas en N días y última compra hace más de N días o nunca).
        sqlx::query(
            "UPDATE detalle_inventario di SET di.motivo_devolucion = ? \
             WHERE di.id_inventario = ? AND EXISTS ( \
               SELECT 1 FROM detalle_solicitud_editorial ds JOIN solicitud_editorial s ON s.id_solicitud = ds.id_solicitud \
                WHERE ds.codigo_ean = di.codigo_ean AND s.estado = 'ABIERTA')",
        )
        .bind(MOTIVO_SOLICITUD)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
        sqlx::query(
            "UPDATE detalle_inventario di JOIN inventario i ON i.id_inventario = di.id_inventario \
             SET di.motivo_devolucion = ? \
             WHERE di.id_inventario = ? AND di.motivo_devolucion IS NULL \
               AND (di.cantidad_contada > 0 OR di.stock_sistema > 0 OR di.stock_apartado > 0) \
               AND NOT EXISTS (SELECT 1 FROM detalle_ventas dv JOIN ventas v ON v.id_ventas = dv.id_ventas \
                    WHERE dv.codigo_ean = di.codigo_ean AND v.fecha_hora >= NOW() - INTERVAL i.dias_anejamiento DAY) \
               AND NOT EXISTS (SELECT 1 FROM detalles_compras dc \
                    JOIN compras_proveedor c ON c.id_compras_proveedor = dc.id_compras_proveedor \
                    WHERE dc.codigo_ean = di.codigo_ean AND c.fecha_hora >= NOW() - INTERVAL i.dias_anejamiento DAY)",
        )
        .bind(MOTIVO_ANEJAMIENTO)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        sqlx::query("UPDATE inventario SET estado = 'CERRADO', fecha_cierre = NOW() WHERE id_inventario = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        tx.commit().await.map_err(AppError::Database)?;
        Ok(())
    }

    /// Aprueba (ajusta piso con la diferencia medida al cierre) o rechaza.
    /// Devuelve (productos ajustados, piezas sumadas, piezas descontadas).
    pub async fn revisar(
        pool: &DbPool,
        id: i64,
        actor_id: i64,
        aprobar: bool,
        observaciones: Option<&str>,
    ) -> Result<(i32, i32, i32), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let estado = Self::estado_bloqueado(&mut tx, id).await?;
        if estado != "CERRADO" {
            return Err(AppError::Conflict(format!(
                "Solo se revisa un inventario CERRADO (el #{} está {})",
                id, estado
            )));
        }

        let (mut productos, mut sumadas, mut descontadas) = (0, 0, 0);
        if aprobar {
            let difs = sqlx::query(
                "SELECT codigo_ean, cantidad_contada - stock_sistema AS dif FROM detalle_inventario \
                 WHERE id_inventario = ? AND stock_sistema IS NOT NULL AND cantidad_contada <> stock_sistema",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            for f in difs {
                let ean = row_get_i64(&f, "codigo_ean");
                let dif = row_get_i32(&f, "dif");
                if dif > 0 {
                    // Hay más de lo que dice el sistema: se suma a un mueble de piso.
                    let mueble = stock::elegir_mueble(&mut tx, ean, UBICACION_TIENDA, dif).await?;
                    stock::sumar(&mut tx, ean, mueble, dif).await?;
                    sumadas += dif;
                } else {
                    // Falta: se descuenta, sin pasar de lo que hay ahora (pudo venderse
                    // algo entre el cierre y la aprobación).
                    let actual = stock::stock_bloqueado(&mut tx, ean, UBICACION_TIENDA).await?;
                    let quitar = (-dif).min(actual);
                    if quitar > 0 {
                        stock::descontar(&mut tx, ean, UBICACION_TIENDA, quitar).await?;
                        descontadas += quitar;
                    }
                }
                productos += 1;
            }
        }

        sqlx::query(
            "UPDATE inventario SET estado = ?, id_usuario_revisa = ?, fecha_revision = NOW(), observaciones = ? \
             WHERE id_inventario = ?",
        )
        .bind(if aprobar { "APROBADO" } else { "RECHAZADO" })
        .bind(actor_id)
        .bind(observaciones)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
        tx.commit().await.map_err(AppError::Database)?;
        Ok((productos, sumadas, descontadas))
    }

    // ------------------------------------------------------------------------
    // Solicitudes de editorial
    // ------------------------------------------------------------------------

    pub async fn listar_solicitudes(pool: &DbPool) -> Result<Vec<SolicitudEditorial>, AppError> {
        let filas = sqlx::query(
            "SELECT s.id_solicitud, s.id_proveedor, pr.nombre_proveedor, DATE_FORMAT(s.fecha, '%Y-%m-%d %H:%i') AS fecha, \
               s.referencia, s.estado, CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, '')) AS registrada_por \
             FROM solicitud_editorial s JOIN proveedor pr ON pr.id_proveedor = s.id_proveedor \
             JOIN usuarios u ON u.id_usuarios = s.id_usuarios \
             ORDER BY (s.estado = 'ATENDIDA'), s.id_solicitud DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;
        let mut res = Vec::with_capacity(filas.len());
        for f in filas {
            let id = row_get_i64(&f, "id_solicitud");
            let items = sqlx::query(
                "SELECT ds.codigo_ean, COALESCE(l.nombre_libro, r.nombre_revista, '') AS titulo, \
                   COALESCE((SELECT SUM(e.cantidad) FROM existencias e WHERE e.codigo_ean = ds.codigo_ean), 0) \
                   + COALESCE((SELECT a.cantidad FROM apartados a WHERE a.codigo_ean = ds.codigo_ean), 0) AS piezas \
                 FROM detalle_solicitud_editorial ds \
                 LEFT JOIN libros l ON l.codigo_ean = ds.codigo_ean LEFT JOIN revistas r ON r.codigo_ean = ds.codigo_ean \
                 WHERE ds.id_solicitud = ? ORDER BY titulo",
            )
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(AppError::Database)?
            .iter()
            .map(|i| SolicitudEditorialItem {
                codigo_ean: row_get_i64(i, "codigo_ean"),
                titulo: i.get("titulo"),
                piezas: row_get_i32(i, "piezas"),
            })
            .collect();
            res.push(SolicitudEditorial {
                id_solicitud: id,
                id_proveedor: row_get_i32(&f, "id_proveedor"),
                proveedor: f.get("nombre_proveedor"),
                fecha: f.get("fecha"),
                referencia: f.get("referencia"),
                estado: f.get("estado"),
                registrada_por: f.get::<String, _>("registrada_por").trim().to_string(),
                items,
            });
        }
        Ok(res)
    }

    pub async fn crear_solicitud(
        pool: &DbPool,
        actor_id: i64,
        id_proveedor: i32,
        referencia: &str,
        eans: &[i64],
    ) -> Result<i64, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        for ean in eans {
            let prov = sqlx::query("SELECT id_proveedor FROM productos WHERE codigo_ean = ?")
                .bind(ean)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::Database)?
                .ok_or_else(|| AppError::NotFound(format!("El producto con EAN {} no existe", ean)))?;
            if row_get_i32(&prov, "id_proveedor") != id_proveedor {
                return Err(AppError::BadRequest(format!(
                    "El EAN {} no es del proveedor de la solicitud; registra una solicitud por editorial",
                    ean
                )));
            }
        }
        let id = sqlx::query("INSERT INTO solicitud_editorial (id_proveedor, id_usuarios, referencia) VALUES (?, ?, ?)")
            .bind(id_proveedor)
            .bind(actor_id)
            .bind(referencia)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .last_insert_id() as i64;
        for ean in eans {
            sqlx::query("INSERT INTO detalle_solicitud_editorial (id_solicitud, codigo_ean) VALUES (?, ?)")
                .bind(id)
                .bind(ean)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
        }
        tx.commit().await.map_err(AppError::Database)?;
        Ok(id)
    }

    pub async fn atender_solicitud(pool: &DbPool, id: i64) -> Result<(), AppError> {
        let res = sqlx::query("UPDATE solicitud_editorial SET estado = 'ATENDIDA' WHERE id_solicitud = ? AND estado = 'ABIERTA'")
            .bind(id)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;
        if res.rows_affected() == 0 {
            return Err(AppError::Conflict(format!("La solicitud #{} no existe o ya fue atendida", id)));
        }
        Ok(())
    }
}
