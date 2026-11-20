use crate::{
    db::{DbPool, existe_duplicado, log_bitacora},
    error::AppError,
    limites::{EDITORIAL_NOMBRE_MAX, RFC_MAX, validar_largo, validar_largo_opt},
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
        validar_largo("El nombre de la editorial", req.nombre_editorial.trim(), EDITORIAL_NOMBRE_MAX)?;
        validar_largo_opt("El RFC", req.rfc.as_deref().map(str::trim), RFC_MAX)?;
        Ok(())
    }

    /// Un solo registro por editorial (nombre sin repetir).
    async fn validar_unica(pool: &DbPool, nombre: &str, excluir_id: Option<i32>) -> Result<(), AppError> {
        if existe_duplicado(pool, "editorial", "id_editorial", &[("nombre_editorial", nombre)],
                            excluir_id.map(i64::from)).await? {
            return Err(AppError::Conflict(format!("Ya existe la editorial '{}'", nombre.trim())));
        }
        Ok(())
    }

    pub async fn crear(pool: &DbPool, actor_id: i64, req: CrearEditorialRequest) -> Result<Editorial, AppError> {
        Self::validar(&req)?;
        Self::validar_unica(pool, &req.nombre_editorial, None).await?;
        let e = EditorialRepo::crear(pool, &req).await?;
        let _ = log_bitacora(pool, actor_id, "CREAR EDITORIAL",
            &format!("Editorial registrada: {} (ID: {})", e.nombre_editorial, e.id_editorial)).await;
        Ok(e)
    }

    pub async fn actualizar(pool: &DbPool, actor_id: i64, id: i32, req: CrearEditorialRequest) -> Result<Editorial, AppError> {
        Self::validar(&req)?;
        Self::validar_unica(pool, &req.nombre_editorial, Some(id)).await?;
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
