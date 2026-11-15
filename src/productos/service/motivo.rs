use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{CrearMotivoRequest, Motivo},
        repo::MotivoRepo,
    },
};

pub struct MotivoService;

impl MotivoService {
    pub async fn listar_motivos(pool: &DbPool) -> Result<Vec<Motivo>, AppError> {
        MotivoRepo::listar_motivos(pool).await
    }

    fn validar(req: &CrearMotivoRequest) -> Result<(), AppError> {
        let nombre = req.nombre_motivo.trim();
        if nombre.is_empty() {
            return Err(AppError::BadRequest("El motivo es obligatorio".to_string()));
        }
        // Límites = tamaño de las columnas en libreria_unificada-definitivo.sql
        if nombre.chars().count() > 50 {
            return Err(AppError::BadRequest(
                "El nombre del motivo no puede exceder 50 caracteres".to_string(),
            ));
        }
        if nombre.chars().any(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest(
                "El nombre del motivo no puede contener números".to_string(),
            ));
        }
        if let Some(desc) = req.descripcion.as_deref()
            && desc.trim().chars().count() > 200
        {
            return Err(AppError::BadRequest(
                "La descripción no puede exceder 200 caracteres".to_string(),
            ));
        }
        Ok(())
    }

    fn normalizar(mut req: CrearMotivoRequest) -> CrearMotivoRequest {
        req.nombre_motivo = req.nombre_motivo.trim().to_string();
        req.descripcion = req
            .descripcion
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());
        req
    }

    pub async fn crear_motivo(
        pool: &DbPool,
        actor_id: i64,
        req: CrearMotivoRequest,
    ) -> Result<Motivo, AppError> {
        Self::validar(&req)?;
        let req = Self::normalizar(req);

        if MotivoRepo::existe_por_nombre(pool, &req.nombre_motivo, None).await? {
            return Err(AppError::Conflict(
                "Ya existe un motivo registrado con ese nombre".to_string(),
            ));
        }

        let m = MotivoRepo::crear_motivo(pool, &req).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "CREAR MOTIVO",
            &format!("Motivo registrado: {} (ID: {})", m.nombre_motivo, m.id_motivo),
        )
        .await;
        Ok(m)
    }

    pub async fn actualizar_motivo(
        pool: &DbPool,
        actor_id: i64,
        id: i32,
        req: CrearMotivoRequest,
    ) -> Result<Motivo, AppError> {
        Self::validar(&req)?;
        let req = Self::normalizar(req);

        if MotivoRepo::existe_por_nombre(pool, &req.nombre_motivo, Some(id)).await? {
            return Err(AppError::Conflict(
                "Ya existe otro motivo registrado con ese nombre".to_string(),
            ));
        }

        let m = MotivoRepo::actualizar_motivo(pool, id, &req).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "ACTUALIZAR MOTIVO",
            &format!("Motivo actualizado: {} (ID: {})", m.nombre_motivo, id),
        )
        .await;
        Ok(m)
    }

    pub async fn eliminar_motivo(pool: &DbPool, actor_id: i64, id: i32) -> Result<(), AppError> {
        MotivoRepo::eliminar_motivo(pool, id).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "ELIMINAR MOTIVO",
            &format!("Motivo eliminado con ID: {}", id),
        )
        .await;
        Ok(())
    }
}