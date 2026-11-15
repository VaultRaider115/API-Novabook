use sqlx::Row;

use crate::{
    db::{DbPool, row_get_i32, row_get_i64},
    error::AppError,
    productos::model::{CrearMotivoRequest, Motivo},
};

pub struct MotivoRepo;

impl MotivoRepo {
    pub async fn listar_motivos(pool: &DbPool) -> Result<Vec<Motivo>, AppError> {
        let rows = sqlx::query(
            "SELECT m.id_motivo, m.nombre_motivo, m.descripcion, \
             (SELECT COUNT(*) FROM detalle_devolucion dd WHERE dd.id_motivo = m.id_motivo) AS total_devoluciones \
             FROM motivo m ORDER BY m.nombre_motivo ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        Ok(rows
            .into_iter()
            .map(|r| Motivo {
                id_motivo: row_get_i32(&r, "id_motivo"),
                nombre_motivo: r.get("nombre_motivo"),
                descripcion: r.get("descripcion"),
                total_devoluciones: row_get_i64(&r, "total_devoluciones"),
            })
            .collect())
    }

    pub async fn existe_por_nombre(
        pool: &DbPool,
        nombre: &str,
        excluir_id: Option<i32>,
    ) -> Result<bool, AppError> {
        let row = match excluir_id {
            Some(id) => {
                sqlx::query(
                    "SELECT id_motivo FROM motivo WHERE LOWER(TRIM(nombre_motivo)) = LOWER(?) AND id_motivo <> ? LIMIT 1",
                )
                .bind(nombre.trim())
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(AppError::Database)?
            }
            None => {
                sqlx::query(
                    "SELECT id_motivo FROM motivo WHERE LOWER(TRIM(nombre_motivo)) = LOWER(?) LIMIT 1",
                )
                .bind(nombre.trim())
                .fetch_optional(pool)
                .await
                .map_err(AppError::Database)?
            }
        };

        Ok(row.is_some())
    }

    pub async fn crear_motivo(pool: &DbPool, req: &CrearMotivoRequest) -> Result<Motivo, AppError> {
        let res = sqlx::query("INSERT INTO motivo (nombre_motivo, descripcion) VALUES (?, ?)")
            .bind(&req.nombre_motivo)
            .bind(&req.descripcion)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;

        Ok(Motivo {
            id_motivo: res.last_insert_id() as i32,
            nombre_motivo: req.nombre_motivo.clone(),
            descripcion: req.descripcion.clone(),
            total_devoluciones: 0,
        })
    }

    pub async fn actualizar_motivo(
        pool: &DbPool,
        id: i32,
        req: &CrearMotivoRequest,
    ) -> Result<Motivo, AppError> {
        let res = sqlx::query("UPDATE motivo SET nombre_motivo = ?, descripcion = ? WHERE id_motivo = ?")
            .bind(&req.nombre_motivo)
            .bind(&req.descripcion)
            .bind(id)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Motivo con ID {} no encontrado",
                id
            )));
        }

        let row = sqlx::query(
            "SELECT COUNT(*) AS total_devoluciones FROM detalle_devolucion WHERE id_motivo = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

        Ok(Motivo {
            id_motivo: id,
            nombre_motivo: req.nombre_motivo.clone(),
            descripcion: req.descripcion.clone(),
            total_devoluciones: row_get_i64(&row, "total_devoluciones"),
        })
    }

    pub async fn eliminar_motivo(pool: &DbPool, id: i32) -> Result<(), AppError> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS cant FROM detalle_devolucion WHERE id_motivo = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

        let cant = row_get_i64(&row, "cant");
        if cant > 0 {
            return Err(AppError::BadRequest(format!(
                "No es posible eliminar el motivo (ID {}): ya está registrado en {} devolución(es).",
                id, cant
            )));
        }

        let res = sqlx::query("DELETE FROM motivo WHERE id_motivo = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Motivo con ID {} no encontrado",
                id
            )));
        }
        Ok(())
    }
}