//! Incidencias por paquete incompleto (diagrama de procesos, paso 1).
//!
//! Al registrar una compra, si la factura dice más piezas de las que llegaron
//! se abre una incidencia con los faltantes. Bodega avisa al proveedor por
//! correo (borrador que arma la API y abre el cliente de correo del equipo) y,
//! cuando llegan los artículos restantes, los registra aquí: entran a bodega,
//! se suman a la compra original y la incidencia se cierra al completarse.

use sqlx::Row;

use crate::{
    db::{DbPool, row_get_f64, row_get_i32, row_get_i64},
    error::AppError,
    productos::{
        model::{CorreoBorrador, FaltanteIncidencia, FiltroIncidencias, IncidenciaCompra, ReposicionRequest},
        repo::{UBICACION_BODEGA, stock::{self, Tx}},
    },
};

macro_rules! select_incidencia {
    () => {
        "SELECT i.id_incidencia, i.id_compras_proveedor, i.estado, \
        DATE_FORMAT(i.fecha, '%Y-%m-%d %H:%i') AS fecha, \
        DATE_FORMAT(i.fecha_cierre, '%Y-%m-%d %H:%i') AS fecha_cierre, \
        c.factura, c.id_proveedor, p.nombre_proveedor, p.correo, p.persona_contacto, \
        TRIM(CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, ''))) AS recibio \
     FROM incidencia_compra i \
     JOIN compras_proveedor c ON c.id_compras_proveedor = i.id_compras_proveedor \
     JOIN proveedor p ON p.id_proveedor = c.id_proveedor \
     JOIN usuarios u ON u.id_usuarios = i.id_usuarios"
    };
}

/// Arma el correo al proveedor con la lista de lo que falta.
pub fn armar_correo(para: Option<String>, contacto: Option<&str>, inc: &IncidenciaCompra) -> CorreoBorrador {
    let (proveedor, factura, id_compra, fecha, id_incidencia, recibio, items) = (
        inc.proveedor.as_str(),
        inc.factura.as_deref(),
        inc.id_compra,
        inc.fecha.as_str(),
        inc.id_incidencia,
        inc.recibio.as_str(),
        &inc.items,
    );
    let doc = match factura.map(str::trim).filter(|f| !f.is_empty()) {
        Some(f) => format!("factura {}", f),
        None => format!("compra #{}", id_compra),
    };
    let saludo = contacto.map(str::trim).filter(|c| !c.is_empty()).unwrap_or(proveedor);
    let pendientes: Vec<&FaltanteIncidencia> = items.iter().filter(|i| i.pendiente > 0).collect();
    let total: i32 = pendientes.iter().map(|i| i.pendiente).sum();

    let mut cuerpo = format!(
        "Estimado(a) {saludo}:\n\n\
         Le informamos que el pedido de la {doc} ({proveedor}), recibido el {fecha} en la bodega de \
         Librería Sanborns Saltillo, llegó incompleto. Faltan los siguientes artículos:\n\n"
    );
    for i in &pendientes {
        cuerpo.push_str(&format!(
            "- {} (EAN {}): facturadas {}, recibidas {}, faltan {}\n",
            i.titulo, i.codigo_ean, i.cantidad_facturada, i.cantidad_recibida + i.cantidad_repuesta, i.pendiente
        ));
    }
    cuerpo.push_str(&format!(
        "\nTotal de piezas faltantes: {total}.\n\n\
         Le pedimos amablemente enviar los artículos restantes a la brevedad y confirmarnos la fecha de envío.\n\n\
         Referencia interna: incidencia #{id_incidencia}.\n\n\
         Atentamente,\n{recibio}\nBodega · Librería Sanborns Saltillo\n"
    ));

    CorreoBorrador {
        para: para.filter(|c| !c.trim().is_empty()),
        asunto: format!("Pedido incompleto · {} · {}", doc, proveedor),
        cuerpo,
    }
}

pub struct IncidenciaRepo;

impl IncidenciaRepo {
    /// Abre la incidencia dentro de la transacción de la compra.
    /// `faltantes` = (ean, facturada, recibida, costo_unitario).
    pub async fn crear_en_tx(
        tx: &mut Tx<'_>,
        id_compra: i64,
        actor_id: i64,
        faltantes: &[(i64, i32, i32, f64)],
    ) -> Result<i64, AppError> {
        let id = sqlx::query("INSERT INTO incidencia_compra (id_compras_proveedor, id_usuarios) VALUES (?, ?)")
            .bind(id_compra)
            .bind(actor_id)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?
            .last_insert_id() as i64;
        for (ean, facturada, recibida, costo) in faltantes {
            sqlx::query(
                "INSERT INTO detalle_incidencia_compra \
                 (id_incidencia, codigo_ean, cantidad_facturada, cantidad_recibida, costo_unitario) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(ean)
            .bind(facturada)
            .bind(recibida)
            .bind(costo)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
        }
        Ok(id)
    }

    pub async fn listar(pool: &DbPool, f: &FiltroIncidencias) -> Result<Vec<IncidenciaCompra>, AppError> {
        let mut q = sqlx::QueryBuilder::<sqlx::MySql>::new(select_incidencia!());
        if let Some(e) = f.estado.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
            q.push(" WHERE i.estado = ").push_bind(e.to_uppercase());
        }
        q.push(" ORDER BY i.estado = 'ABIERTA' DESC, i.fecha DESC LIMIT 200");
        let filas = q.build().fetch_all(pool).await.map_err(AppError::Database)?;
        let mut out = Vec::with_capacity(filas.len());
        for f in &filas {
            out.push(Self::armar(pool, f).await?);
        }
        Ok(out)
    }

    pub async fn obtener(pool: &DbPool, id: i64) -> Result<IncidenciaCompra, AppError> {
        let fila = sqlx::query(concat!(select_incidencia!(), " WHERE i.id_incidencia = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("No existe la incidencia #{}", id)))?;
        Self::armar(pool, &fila).await
    }

    async fn armar(pool: &DbPool, f: &sqlx::mysql::MySqlRow) -> Result<IncidenciaCompra, AppError> {
        let id = row_get_i64(f, "id_incidencia");
        let items: Vec<FaltanteIncidencia> = sqlx::query(
            "SELECT d.codigo_ean, d.cantidad_facturada, d.cantidad_recibida, d.cantidad_repuesta, \
                    COALESCE(l.nombre_libro, r.nombre_revista, '') AS titulo \
             FROM detalle_incidencia_compra d \
             LEFT JOIN libros l ON l.codigo_ean = d.codigo_ean \
             LEFT JOIN revistas r ON r.codigo_ean = d.codigo_ean \
             WHERE d.id_incidencia = ? ORDER BY titulo",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?
        .iter()
        .map(|d| {
            let fac = row_get_i32(d, "cantidad_facturada");
            let rec = row_get_i32(d, "cantidad_recibida");
            let rep = row_get_i32(d, "cantidad_repuesta");
            FaltanteIncidencia {
                codigo_ean: row_get_i64(d, "codigo_ean"),
                titulo: d.get("titulo"),
                cantidad_facturada: fac,
                cantidad_recibida: rec,
                cantidad_repuesta: rep,
                pendiente: (fac - rec - rep).max(0),
            }
        })
        .collect();

        let contacto: Option<String> = f.try_get("persona_contacto").ok().flatten();
        let mut inc = IncidenciaCompra {
            id_incidencia: id,
            id_compra: row_get_i64(f, "id_compras_proveedor"),
            factura: f.try_get("factura").ok().flatten(),
            estado: f.get("estado"),
            fecha: f.get("fecha"),
            fecha_cierre: f.try_get("fecha_cierre").ok().flatten(),
            id_proveedor: row_get_i32(f, "id_proveedor"),
            proveedor: f.get("nombre_proveedor"),
            recibio: f.get("recibio"),
            piezas_pendientes: items.iter().map(|i| i.pendiente).sum(),
            items,
            correo: CorreoBorrador { para: None, asunto: String::new(), cuerpo: String::new() },
        };
        inc.correo = armar_correo(f.try_get("correo").ok().flatten(), contacto.as_deref(), &inc);
        Ok(inc)
    }

    /// Llegaron (todos o parte de) los faltantes. Devuelve las piezas recibidas
    /// y si la incidencia quedó cerrada.
    pub async fn reponer(
        pool: &DbPool,
        id: i64,
        req: &ReposicionRequest,
    ) -> Result<(i32, bool), AppError> {
        if req.items.is_empty() {
            return Err(AppError::BadRequest("Captura al menos un artículo recibido".to_string()));
        }
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let inc = sqlx::query("SELECT estado, id_compras_proveedor FROM incidencia_compra WHERE id_incidencia = ? FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("No existe la incidencia #{}", id)))?;
        let estado: String = inc.get("estado");
        if estado != "ABIERTA" {
            return Err(AppError::Conflict(format!("La incidencia #{} ya está cerrada", id)));
        }
        let id_compra = row_get_i64(&inc, "id_compras_proveedor");

        let mut piezas = 0;
        let mut importe = 0.0;
        let mut vistos = std::collections::HashSet::new();
        for it in &req.items {
            if !vistos.insert(it.codigo_ean) {
                return Err(AppError::BadRequest(format!("El EAN {} viene repetido", it.codigo_ean)));
            }
            if it.cantidad <= 0 {
                return Err(AppError::BadRequest(format!(
                    "La cantidad recibida del EAN {} debe ser mayor a 0", it.codigo_ean
                )));
            }
            let d = sqlx::query(
                "SELECT cantidad_facturada - cantidad_recibida - cantidad_repuesta AS pendiente, \
                        CAST(costo_unitario AS DOUBLE) AS costo \
                 FROM detalle_incidencia_compra WHERE id_incidencia = ? AND codigo_ean = ? FOR UPDATE",
            )
            .bind(id)
            .bind(it.codigo_ean)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::BadRequest(format!("El EAN {} no está entre los faltantes de la incidencia #{}", it.codigo_ean, id))
            })?;
            let pendiente = row_get_i32(&d, "pendiente");
            if it.cantidad > pendiente {
                return Err(AppError::BadRequest(format!(
                    "Del EAN {} solo faltan {} pieza(s); se capturaron {}",
                    it.codigo_ean, pendiente.max(0), it.cantidad
                )));
            }
            let costo = row_get_f64(&d, "costo");

            let mueble = match it.id_mueble {
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
                None => stock::elegir_mueble(&mut tx, it.codigo_ean, UBICACION_BODEGA, it.cantidad).await?,
            };
            stock::sumar(&mut tx, it.codigo_ean, mueble, it.cantidad).await?;

            // Se suma a la compra original para que el costo del acta (paso 8) la tome.
            let subtotal = costo * it.cantidad as f64;
            sqlx::query(
                "INSERT INTO detalles_compras (id_compras_proveedor, codigo_ean, cantidad, subtotal) VALUES (?, ?, ?, ?)",
            )
            .bind(id_compra)
            .bind(it.codigo_ean)
            .bind(it.cantidad)
            .bind(subtotal)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            sqlx::query(
                "UPDATE detalle_incidencia_compra SET cantidad_repuesta = cantidad_repuesta + ? \
                 WHERE id_incidencia = ? AND codigo_ean = ?",
            )
            .bind(it.cantidad)
            .bind(id)
            .bind(it.codigo_ean)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            piezas += it.cantidad;
            importe += subtotal;
        }

        sqlx::query("UPDATE compras_proveedor SET total = total + ? WHERE id_compras_proveedor = ?")
            .bind(importe)
            .bind(id_compra)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;

        let restante = sqlx::query(
            "SELECT COALESCE(SUM(cantidad_facturada - cantidad_recibida - cantidad_repuesta), 0) AS r \
             FROM detalle_incidencia_compra WHERE id_incidencia = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::Database)?;
        let cerrada = row_get_i32(&restante, "r") <= 0;
        if cerrada {
            sqlx::query("UPDATE incidencia_compra SET estado = 'CERRADA', fecha_cierre = NOW() WHERE id_incidencia = ?")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
        }
        tx.commit().await.map_err(AppError::Database)?;
        Ok((piezas, cerrada))
    }
}

#[cfg(test)]
mod tests {
    use super::armar_correo;
    use crate::productos::model::{CorreoBorrador, FaltanteIncidencia, IncidenciaCompra};

    fn inc(factura: Option<&str>, items: Vec<FaltanteIncidencia>) -> IncidenciaCompra {
        IncidenciaCompra {
            id_incidencia: 3,
            id_compra: 7,
            factura: factura.map(String::from),
            estado: "ABIERTA".into(),
            fecha: "2026-11-15 10:00".into(),
            fecha_cierre: None,
            id_proveedor: 1,
            proveedor: "Planeta".into(),
            recibio: "Juan Pérez".into(),
            piezas_pendientes: 0,
            items,
            correo: CorreoBorrador { para: None, asunto: String::new(), cuerpo: String::new() },
        }
    }

    fn f(ean: i64, fac: i32, rec: i32, rep: i32) -> FaltanteIncidencia {
        FaltanteIncidencia {
            codigo_ean: ean,
            titulo: format!("Libro {}", ean),
            cantidad_facturada: fac,
            cantidad_recibida: rec,
            cantidad_repuesta: rep,
            pendiente: fac - rec - rep,
        }
    }

    #[test]
    fn correo_lista_solo_lo_pendiente() {
        let c = armar_correo(
            Some("ventas@editorial.mx".into()),
            Some("Lic. Ana"),
            &inc(Some("F-1"), vec![f(9781, 20, 15, 0), f(9782, 10, 4, 6)]),
        );
        assert_eq!(c.para.as_deref(), Some("ventas@editorial.mx"));
        assert!(c.asunto.contains("factura F-1"));
        assert!(c.cuerpo.starts_with("Estimado(a) Lic. Ana:"));
        assert!(c.cuerpo.contains("EAN 9781): facturadas 20, recibidas 15, faltan 5"));
        assert!(!c.cuerpo.contains("EAN 9782"));
        assert!(c.cuerpo.contains("Total de piezas faltantes: 5."));
        assert!(c.cuerpo.contains("incidencia #3"));
    }

    #[test]
    fn sin_factura_usa_el_numero_de_compra_y_el_proveedor() {
        let c = armar_correo(Some("  ".into()), None, &inc(None, vec![f(1, 2, 1, 0)]));
        assert!(c.para.is_none());
        assert!(c.asunto.contains("compra #7"));
        assert!(c.cuerpo.starts_with("Estimado(a) Planeta:"));
    }
}
