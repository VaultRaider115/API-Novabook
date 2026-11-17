use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{CrearMuebleRequest, Mueble},
        repo::MuebleRepo,
    },
};

pub struct MuebleService;

impl MuebleService {
    pub async fn listar_muebles(pool: &DbPool) -> Result<Vec<Mueble>, AppError> {
        MuebleRepo::listar_muebles(pool).await
    }

    fn validar_y_normalizar(mut req: CrearMuebleRequest) -> Result<CrearMuebleRequest, AppError> {
        let codigo = req.codigo_mueble.trim().to_uppercase();
        if codigo.is_empty() {
            return Err(AppError::BadRequest(
                "El código del mueble es obligatorio".to_string(),
            ));
        }
        // Límites = tamaño de las columnas en libreria_unificada-definitivo.sql
        if codigo.chars().count() > 10 {
            return Err(AppError::BadRequest(
                "El código del mueble no puede exceder 10 caracteres".to_string(),
            ));
        }
        if !codigo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(AppError::BadRequest(
                "El código del mueble solo puede contener letras, números y guiones (-)".to_string(),
            ));
        }

        let nombre = req.nombre_mueble.trim().to_string();
        if nombre.is_empty() {
            return Err(AppError::BadRequest(
                "El nombre del mueble es obligatorio".to_string(),
            ));
        }
        if nombre.chars().count() > 50 {
            return Err(AppError::BadRequest(
                "El nombre del mueble no puede exceder 50 caracteres".to_string(),
            ));
        }

        if req.id_ubicacion != 1 && req.id_ubicacion != 2 {
            return Err(AppError::BadRequest(
                "Área inválida (debe ser Piso de Venta o Bodega)".to_string(),
            ));
        }
        // El código empieza con P- (Piso de Venta) o B- (Bodega), según el área.
        let prefijo = if req.id_ubicacion == 1 { "P-" } else { "B-" };
        if !codigo.starts_with(prefijo) || codigo.len() == prefijo.len() {
            return Err(AppError::BadRequest(format!(
                "El código de un mueble de {} debe empezar con \"{}\" seguido del número (ej. {}011)",
                if req.id_ubicacion == 1 { "Piso de Venta" } else { "Bodega" },
                prefijo,
                prefijo
            )));
        }

        let cap = req.capacidad.unwrap_or(0);
        if !(1..=99_999).contains(&cap) {
            return Err(AppError::BadRequest(
                "La capacidad máxima debe estar entre 1 y 99,999 piezas".to_string(),
            ));
        }

        if let Some(desc) = req.descripcion.as_deref()
            && desc.trim().chars().count() > 200
        {
            return Err(AppError::BadRequest(
                "La descripción no puede exceder 200 caracteres".to_string(),
            ));
        }

        req.codigo_mueble = codigo;
        req.nombre_mueble = nombre;
        req.capacidad = Some(cap);
        req.descripcion = req
            .descripcion
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());

        Ok(req)
    }

    pub async fn crear_mueble(
        pool: &DbPool,
        actor_id: i64,
        req: CrearMuebleRequest,
    ) -> Result<Mueble, AppError> {
        let req = Self::validar_y_normalizar(req)?;

        if MuebleRepo::existe_codigo(pool, &req.codigo_mueble, None).await? {
            return Err(AppError::Conflict(format!(
                "Ya existe un mueble con el código '{}'",
                req.codigo_mueble
            )));
        }
        if MuebleRepo::existe_nombre(pool, &req.nombre_mueble, None).await? {
            return Err(AppError::Conflict(format!(
                "Ya existe un mueble con el nombre '{}'",
                req.nombre_mueble
            )));
        }

        let m = MuebleRepo::crear_mueble(pool, &req).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "CREAR MUEBLE",
            &format!("Mueble registrado: ID {} (género: {})", m.id_mueble, m.genero_literario),
        )
        .await;
        Ok(m)
    }

    pub async fn actualizar_mueble(
        pool: &DbPool,
        actor_id: i64,
        id: i32,
        req: CrearMuebleRequest,
    ) -> Result<Mueble, AppError> {
        let req = Self::validar_y_normalizar(req)?;
        let actual = MuebleRepo::buscar_mueble(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Mueble con ID {} no encontrado", id)))?;

        if MuebleRepo::existe_codigo(pool, &req.codigo_mueble, Some(id)).await? {
            return Err(AppError::Conflict(format!(
                "Ya existe otro mueble con el código '{}'",
                req.codigo_mueble
            )));
        }
        if MuebleRepo::existe_nombre(pool, &req.nombre_mueble, Some(id)).await? {
            return Err(AppError::Conflict(format!(
                "Ya existe otro mueble con el nombre '{}'",
                req.nombre_mueble
            )));
        }

        if actual.capacidad_actual > 0 {
            let cap = req.capacidad.unwrap_or(0);
            if cap < actual.capacidad_actual {
                return Err(AppError::BadRequest(format!(
                    "La capacidad máxima ({}) no puede ser menor a las {} piezas que ya tiene el mueble",
                    cap, actual.capacidad_actual
                )));
            }
            if req.id_ubicacion != actual.id_ubicacion {
                return Err(AppError::BadRequest(
                    "No puedes cambiar el área de un mueble que tiene piezas almacenadas".to_string(),
                ));
            }
            if req.id_genero != actual.id_genero {
                return Err(AppError::BadRequest(
                    "No puedes cambiar el género de un mueble que tiene piezas almacenadas".to_string(),
                ));
            }
        }

        let m = MuebleRepo::actualizar_mueble(pool, id, &req).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "ACTUALIZAR MUEBLE",
            &format!("Mueble ID {} ahora asignado al género: {}", id, m.genero_literario),
        )
        .await;
        Ok(m)
    }

    pub async fn eliminar_mueble(pool: &DbPool, actor_id: i64, id: i32) -> Result<(), AppError> {
        MuebleRepo::eliminar_mueble(pool, id).await?;
        let _ = log_bitacora(
            pool,
            actor_id,
            "ELIMINAR MUEBLE",
            &format!("Mueble eliminado con ID: {}", id),
        )
        .await;
        Ok(())
    }
}