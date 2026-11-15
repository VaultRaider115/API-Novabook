use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{CrearEditorialRequest, Editorial},
        repo::EditorialRepo,
    },
};

pub struct EditorialService;

impl EditorialService {
    pub async fn listar(pool: &DbPool) -> Result<Vec<Editorial>, AppError> {
        EditorialRepo::listar(pool).await
    }

    fn validar(req: &CrearEditorialRequest) -> Result<(), AppError> {
        if req.nombre_editorial.trim().is_empty() {
            return Err(AppError::BadRequest("El nombre de la editorial es obligatorio".to_string()));
        }
        if req.rfc.as_deref().is_some_and(|r| r.len() > 13) {
            return Err(AppError::BadRequest("El RFC no puede exceder 13 caracteres".to_string()));
        }
        Ok(())
    }

    pub async fn crear(pool: &DbPool, actor_id: i64, req: CrearEditorialRequest) -> Result<Editorial, AppError> {
        Self::validar(&req)?;
        let e = EditorialRepo::crear(pool, &req).await?;
        let _ = log_bitacora(pool, actor_id, "CREAR EDITORIAL",
            &format!("Editorial registrada: {} (ID: {})", e.nombre_editorial, e.id_editorial)).await;
        Ok(e)
    }

    pub async fn actualizar(pool: &DbPool, actor_id: i64, id: i32, req: CrearEditorialRequest) -> Result<Editorial, AppError> {
        Self::validar(&req)?;
        let e = EditorialRepo::actualizar(pool, id, &req).await?;
        let _ = log_bitacora(pool, actor_id, "ACTUALIZAR EDITORIAL",
            &format!("Editorial actualizada: {} (ID: {})", e.nombre_editorial, id)).await;
        Ok(e)
    }

    pub async fn eliminar(pool: &DbPool, actor_id: i64, id: i32) -> Result<(), AppError> {
        EditorialRepo::eliminar(pool, id).await?;
        let _ = log_bitacora(pool, actor_id, "ELIMINAR EDITORIAL", &format!("Editorial eliminada con ID: {}", id)).await;
        Ok(())
    }
}
