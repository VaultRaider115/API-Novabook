use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    limites::{GENERO_MAX, validar_largo},
    productos::{
        model::{CrearGeneroRequest, Genero},
        repo::GeneroRepo,
    },
};

pub struct GeneroService;

impl GeneroService {
    pub async fn listar_generos(pool: &DbPool) -> Result<Vec<Genero>, AppError> {
        GeneroRepo::listar_generos(pool).await
    }

    fn validar(req: &CrearGeneroRequest) -> Result<(), AppError> {
        let nombre = req.genero_literario.trim();
        if nombre.is_empty() {
            return Err(AppError::BadRequest(
                "El nombre del género es obligatorio".to_string(),
            ));
        }
        validar_largo("El nombre del género", nombre, GENERO_MAX)?;
        if nombre.chars().any(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest(
                "El nombre del género no puede contener números".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn crear_genero(
        pool: &DbPool,
        actor_id: i64,
        req: CrearGeneroRequest,
    ) -> Result<Genero, AppError> {
        Self::validar(&req)?;
        let mut req = req;
        req.genero_literario = req.genero_literario.trim().to_string();

        if GeneroRepo::existe_por_nombre(pool, &req.genero_literario, None).await? {
            return Err(AppError::Conflict(
                "Ya existe un género registrado con ese nombre".to_string(),
            ));
        }

        let g = GeneroRepo::crear_genero(pool, &req).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "CREAR GENERO",
            &format!("Género registrado: {} (ID: {})", g.genero_literario, g.id_genero),
        )
        .await;
        Ok(g)
    }

    pub async fn actualizar_genero(
        pool: &DbPool,
        actor_id: i64,
        id: i32,
        req: CrearGeneroRequest,
    ) -> Result<Genero, AppError> {
        Self::validar(&req)?;
        let mut req = req;
        req.genero_literario = req.genero_literario.trim().to_string();

        if GeneroRepo::existe_por_nombre(pool, &req.genero_literario, Some(id)).await? {
            return Err(AppError::Conflict(
                "Ya existe otro género registrado con ese nombre".to_string(),
            ));
        }

        let g = GeneroRepo::actualizar_genero(pool, id, &req).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "ACTUALIZAR GENERO",
            &format!("Género actualizado: {} (ID: {})", g.genero_literario, id),
        )
        .await;
        Ok(g)
    }

    pub async fn eliminar_genero(pool: &DbPool, actor_id: i64, id: i32) -> Result<(), AppError> {
        GeneroRepo::eliminar_genero(pool, id).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "ELIMINAR GENERO",
            &format!("Género eliminado con ID: {}", id),
        )
        .await;
        Ok(())
    }
}
