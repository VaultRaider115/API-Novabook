use sqlx::Row;

use crate::{
    db::{DbPool, row_get_i32},
    error::AppError,
    productos::model::{CrearEditorialRequest, Editorial},
};

pub struct EditorialRepo;

impl EditorialRepo {
    pub async fn listar(pool: &DbPool) -> Result<Vec<Editorial>, AppError> {
        let rows = sqlx::query("SELECT id_editorial, nombre_editorial, rfc FROM editorial ORDER BY nombre_editorial ASC")
            .fetch_all(pool)
            .await
            .map_err(AppError::Database)?;
        Ok(rows
            .into_iter()
            .map(|r| Editorial {
                id_editorial: row_get_i32(&r, "id_editorial"),
                nombre_editorial: r.get("nombre_editorial"),
                rfc: r.try_get::<Option<String>, _>("rfc").ok().flatten(),
            })
            .collect())
    }

    pub async fn crear(pool: &DbPool, req: &CrearEditorialRequest) -> Result<Editorial, AppError> {
        let res = sqlx::query("INSERT INTO editorial (nombre_editorial, rfc) VALUES (?, ?)")
            .bind(req.nombre_editorial.trim())
            .bind(&req.rfc)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;
        Ok(Editorial {
            id_editorial: res.last_insert_id() as i32,
            nombre_editorial: req.nombre_editorial.trim().to_string(),
            rfc: req.rfc.clone(),
        })
    }

    pub async fn actualizar(pool: &DbPool, id: i32, req: &CrearEditorialRequest) -> Result<Editorial, AppError> {
        let res = sqlx::query("UPDATE editorial SET nombre_editorial = ?, rfc = ? WHERE id_editorial = ?")
            .bind(req.nombre_editorial.trim())
            .bind(&req.rfc)
            .bind(id)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;
        if res.rows_affected() == 0 {
            // MySQL devuelve 0 también si no hubo cambios; confirmar que existe
            let existe = sqlx::query("SELECT 1 FROM editorial WHERE id_editorial = ?")
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(AppError::Database)?;
            if existe.is_none() {
                return Err(AppError::NotFound(format!("Editorial con ID {} no encontrada", id)));
            }
        }
        Ok(Editorial {
            id_editorial: id,
            nombre_editorial: req.nombre_editorial.trim().to_string(),
            rfc: req.rfc.clone(),
        })
    }

    pub async fn eliminar(pool: &DbPool, id: i32) -> Result<(), AppError> {
        let usada = sqlx::query("SELECT COUNT(*) AS c FROM revistas WHERE id_editorial = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(AppError::Database)?;
        if row_get_i32(&usada, "c") > 0 {
            return Err(AppError::Conflict(
                "No se puede eliminar: la editorial tiene revistas asociadas".to_string(),
            ));
        }
        let res = sqlx::query("DELETE FROM editorial WHERE id_editorial = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(AppError::Database)?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Editorial con ID {} no encontrada", id)));
        }
        Ok(())
    }
}
