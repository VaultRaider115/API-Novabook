use sqlx::Row;

use crate::{
    db::DbPool,
    error::AppError,
    productos::repo::stock::{self, nombre_ubicacion},
};

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
        if origen == destino {
            return Err(AppError::BadRequest("El origen y el destino no pueden ser el mismo".to_string()));
        }
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let prod = sqlx::query("SELECT tipo_producto FROM productos WHERE codigo_ean = ?")
            .bind(codigo_ean)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("El producto con EAN {} no existe", codigo_ean)))?;
        let tipo_db: String = prod.get("tipo_producto");
        if !tipo_db.eq_ignore_ascii_case(tipo) {
            return Err(AppError::BadRequest(format!(
                "El EAN {} corresponde a un producto de tipo {}, no {}",
                codigo_ean, tipo_db, tipo
            )));
        }

        let disponible = stock::stock_bloqueado(&mut tx, codigo_ean, origen).await?;
        if disponible < cantidad {
            return Err(AppError::BadRequest(format!(
                "Stock insuficiente en {}. Disponible: {}, Solicitado: {}",
                nombre_ubicacion(origen), disponible, cantidad
            )));
        }

        let id_destino = match mueble_destino {
            Some(m) => {
                let ok = sqlx::query("SELECT 1 FROM mueble WHERE id_mueble = ? AND id_ubicacion = ?")
                    .bind(m)
                    .bind(destino)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(AppError::Database)?;
                if ok.is_none() {
                    return Err(AppError::BadRequest(format!(
                        "El mueble {} no pertenece a {}", m, nombre_ubicacion(destino)
                    )));
                }
                m
            }
            None => stock::elegir_mueble(&mut tx, codigo_ean, destino, cantidad).await?,
        };

        let usados = stock::descontar(&mut tx, codigo_ean, origen, cantidad).await?;
        stock::sumar(&mut tx, codigo_ean, id_destino, cantidad).await?;

        for (mueble_origen, piezas) in usados {
            sqlx::query(
                "INSERT INTO movimientos (id_usuarios, codigo_ean, mueble_origen, mueble_destino, cantidad, observaciones) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(actor_id)
            .bind(codigo_ean)
            .bind(mueble_origen)
            .bind(id_destino)
            .bind(piezas)
            .bind(observaciones)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }

        let restante = disponible - cantidad;
        let nuevo_destino: i32 = {
            let rows = sqlx::query(
                "SELECT COALESCE(SUM(e.cantidad), 0) AS t FROM existencias e JOIN mueble m ON m.id_mueble = e.id_mueble \
                 WHERE e.codigo_ean = ? AND m.id_ubicacion = ?",
            )
            .bind(codigo_ean)
            .bind(destino)
            .fetch_one(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            crate::db::row_get_i32(&rows, "t")
        };

        tx.commit().await.map_err(AppError::Database)?;
        Ok((restante, nuevo_destino))
    }
}
