use sqlx::Row;

use crate::{
    db::DbPool,
    error::AppError,
    productos::{
        model::{PartidaRequisicion, PartidaRequisicionResp},
        repo::stock::{self, Tx, nombre_ubicacion},
    },
};

/// Resultado de mover un producto dentro de una transacción.
struct TrasladoTx {
    restante: i32,
    nuevo_destino: i32,
    tipo_producto: String,
    ids_movimiento: Vec<i64>,
}

/// Requisición ya registrada (todas las partidas en la misma transacción).
pub struct RequisicionRegistrada {
    pub folio: String,
    pub fecha_hora: String,
    pub items: Vec<PartidaRequisicionResp>,
}

pub struct MovimientoRepo;

impl MovimientoRepo {
    pub async fn trasladar_stock_libro(
        pool: &DbPool,
        codigo_ean: i64,
        cantidad: i32,
        origen: i32,
        destino: i32,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<(i32, i32), AppError> {
        Self::trasladar(pool, "Libro", codigo_ean, cantidad, origen, destino, None, actor_id, observaciones).await
    }

    pub async fn trasladar_stock_revista(
        pool: &DbPool,
        codigo_ean: i64,
        cantidad: i32,
        origen: i32,
        destino: i32,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<(i32, i32), AppError> {
        Self::trasladar(pool, "Revista", codigo_ean, cantidad, origen, destino, None, actor_id, observaciones).await
    }

    /// Traslada piezas entre ubicaciones (Piso/Bodega) moviendo existencias entre muebles
    /// y registrando un movimiento por cada mueble de origen. Devuelve (restante_origen, nuevo_destino).
    #[allow(clippy::too_many_arguments)]
    pub async fn trasladar(
        pool: &DbPool,
        tipo: &str,
        codigo_ean: i64,
        cantidad: i32,
        origen: i32,
        destino: i32,
        mueble_destino: Option<i32>,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<(i32, i32), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;
        let r = Self::trasladar_en_tx(
            &mut tx, Some(tipo), codigo_ean, cantidad, origen, destino, mueble_destino, actor_id, observaciones,
        )
        .await?;
        tx.commit().await.map_err(AppError::Database)?;
        Ok((r.restante, r.nuevo_destino))
    }

    /// Registra varias partidas como un solo documento: si una falla (stock
    /// insuficiente, EAN inexistente) no se mueve nada. Todas las filas de
    /// `movimientos` llevan el folio en `observaciones` ("Requisición REQ-AAAA-n").
    pub async fn registrar_requisicion(
        pool: &DbPool,
        items: &[PartidaRequisicion],
        origen: i32,
        destino: i32,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<RequisicionRegistrada, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let mut ids: Vec<i64> = Vec::new();
        let mut partidas = Vec::with_capacity(items.len());
        for it in items {
            let r = Self::trasladar_en_tx(
                &mut tx, None, it.codigo_ean, it.cantidad, origen, destino, None, actor_id, None,
            )
            .await?;
            ids.extend(&r.ids_movimiento);

            let titulo: Option<String> = sqlx::query(
                "SELECT COALESCE(l.nombre_libro, r.nombre_revista) AS titulo FROM productos p \
                 LEFT JOIN libros l ON l.codigo_ean = p.codigo_ean \
                 LEFT JOIN revistas r ON r.codigo_ean = p.codigo_ean WHERE p.codigo_ean = ?",
            )
            .bind(it.codigo_ean)
            .fetch_one(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .get("titulo");

            partidas.push(PartidaRequisicionResp {
                codigo_ean: it.codigo_ean,
                tipo_producto: r.tipo_producto,
                titulo: titulo.unwrap_or_default(),
                cantidad: it.cantidad,
            });
        }

        // Folio = año + id del primer movimiento: único sin cambiar el esquema.
        let primero = *ids.first().ok_or_else(|| AppError::Internal("Requisición sin movimientos".to_string()))?;
        let ahora = sqlx::query(
            "SELECT DATE_FORMAT(NOW(), '%Y') AS anio, DATE_FORMAT(NOW(), '%Y-%m-%d %H:%i:%s') AS fecha",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::Database)?;
        let anio: String = ahora.get("anio");
        let fecha_hora: String = ahora.get("fecha");
        let folio = format!("REQ-{}-{}", anio, primero);

        let mut nota = format!("Requisición {}", folio);
        if let Some(obs) = observaciones.map(str::trim).filter(|o| !o.is_empty()) {
            nota.push_str(" · ");
            nota.push_str(obs);
        }
        let nota: String = nota.chars().take(200).collect(); // VARCHAR(200)

        let mut upd = sqlx::QueryBuilder::<sqlx::MySql>::new("UPDATE movimientos SET observaciones = ");
        upd.push_bind(&nota).push(" WHERE id_movimientos IN (");
        let mut sep = upd.separated(", ");
        for id in &ids {
            sep.push_bind(*id);
        }
        upd.push(")");
        upd.build().execute(&mut *tx).await.map_err(AppError::Database)?;

        tx.commit().await.map_err(AppError::Database)?;
        Ok(RequisicionRegistrada { folio, fecha_hora, items: partidas })
    }

    /// Núcleo del traslado dentro de una transacción ya abierta.
    /// `tipo = None` acepta el tipo que tenga el producto en la BD.
    #[allow(clippy::too_many_arguments)]
    async fn trasladar_en_tx(
        tx: &mut Tx<'_>,
        tipo: Option<&str>,
        codigo_ean: i64,
        cantidad: i32,
        origen: i32,
        destino: i32,
        mueble_destino: Option<i32>,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<TrasladoTx, AppError> {
        if origen == destino {
            return Err(AppError::BadRequest("El origen y el destino no pueden ser el mismo".to_string()));
        }

        let prod = sqlx::query("SELECT tipo_producto FROM productos WHERE codigo_ean = ?")
            .bind(codigo_ean)
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("El producto con EAN {} no existe", codigo_ean)))?;
        let tipo_db: String = prod.get("tipo_producto");
        if let Some(t) = tipo
            && !tipo_db.eq_ignore_ascii_case(t)
        {
            return Err(AppError::BadRequest(format!(
                "El EAN {} corresponde a un producto de tipo {}, no {}",
                codigo_ean, tipo_db, t
            )));
        }

        let disponible = stock::stock_bloqueado(tx, codigo_ean, origen).await?;
        if disponible < cantidad {
            return Err(AppError::BadRequest(format!(
                "Stock insuficiente de EAN {} en {}. Disponible: {}, Solicitado: {}",
                codigo_ean, nombre_ubicacion(origen), disponible, cantidad
            )));
        }

        let id_destino = match mueble_destino {
            Some(m) => {
                let ok = sqlx::query("SELECT 1 FROM mueble WHERE id_mueble = ? AND id_ubicacion = ?")
                    .bind(m)
                    .bind(destino)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(AppError::Database)?;
                if ok.is_none() {
                    return Err(AppError::BadRequest(format!(
                        "El mueble {} no pertenece a {}", m, nombre_ubicacion(destino)
                    )));
                }
                m
            }
            None => stock::elegir_mueble(tx, codigo_ean, destino, cantidad).await?,
        };

        let usados = stock::descontar(tx, codigo_ean, origen, cantidad).await?;
        stock::sumar(tx, codigo_ean, id_destino, cantidad).await?;

        let mut ids_movimiento = Vec::with_capacity(usados.len());
        for (mueble_origen, piezas) in usados {
            let res = sqlx::query(
                "INSERT INTO movimientos (id_usuarios, codigo_ean, mueble_origen, mueble_destino, cantidad, observaciones) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(actor_id)
            .bind(codigo_ean)
            .bind(mueble_origen)
            .bind(id_destino)
            .bind(piezas)
            .bind(observaciones)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
            ids_movimiento.push(res.last_insert_id() as i64);
        }

        let restante = disponible - cantidad;
        let nuevo_destino: i32 = {
            let rows = sqlx::query(
                "SELECT COALESCE(SUM(e.cantidad), 0) AS t FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
                 WHERE e.codigo_ean = ? AND m.id_ubicacion = ?",
            )
            .bind(codigo_ean)
            .bind(destino)
            .fetch_one(&mut **tx)
            .await
            .map_err(AppError::Database)?;
            crate::db::row_get_i32(&rows, "t")
        };

        Ok(TrasladoTx { restante, nuevo_destino, tipo_producto: tipo_db, ids_movimiento })
    }
}
