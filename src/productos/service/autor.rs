use crate::{
    db::{DbPool, existe_duplicado, log_bitacora},
    error::AppError,
    limites::{AUTOR_APELLIDOS_MAX, AUTOR_NOMBRE_MAX, validar_largo},
    productos::{
        model::{ActualizarAutorRequest, Autor, CrearAutorRequest},
        repo::AutorRepo,
    },
};

fn validate_author_text(value: &str, field: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!("{field} es obligatorio")));
    }
    if value.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(format!(
            "{field} no puede contener números"
        )));
    }
    Ok(())
}

/// Un solo registro por autor: no se repite la combinación nombre + apellidos.
async fn validar_unico(
    pool: &DbPool,
    nombre: &str,
    apellidos: &str,
    excluir_id: Option<i32>,
) -> Result<(), AppError> {
    let columnas = [("nombre", nombre), ("apellidos", apellidos)];
    if existe_duplicado(pool, "autores", "id_autores", &columnas, excluir_id.map(i64::from)).await? {
        return Err(AppError::Conflict(format!("Ya existe el autor {} {}", nombre, apellidos)));
    }
    Ok(())
}

pub struct AutorService;

impl AutorService {
    pub async fn listar_autores(pool: &DbPool) -> Result<Vec<Autor>, AppError> {
        AutorRepo::listar_autores(pool).await
    }

    pub async fn obtener_autor(pool: &DbPool, id: i32) -> Result<Autor, AppError> {
        AutorRepo::buscar_autor(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Autor con ID {} no encontrado", id)))
    }

    pub async fn crear_autor(
        pool: &DbPool,
        actor_id: i64,
        req: CrearAutorRequest,
    ) -> Result<Autor, AppError> {
        validate_author_text(&req.nombre, "El nombre del autor")?;
        validate_author_text(&req.apellidos, "Los apellidos del autor")?;
        validar_largo("El nombre del autor", req.nombre.trim(), AUTOR_NOMBRE_MAX)?;
        validar_largo("Los apellidos del autor", req.apellidos.trim(), AUTOR_APELLIDOS_MAX)?;

        let mut req = req;
        req.nombre = req.nombre.trim().to_string();
        req.apellidos = req.apellidos.trim().to_string();
        validar_unico(pool, &req.nombre, &req.apellidos, None).await?;

        let autor = AutorRepo::crear_autor(pool, &req).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "CREAR AUTOR",
            &format!(
                "Autor registrado: {} (ID: {})",
                autor.nombre_completo(),
                autor.id_autores
            ),
        )
        .await;

        Ok(autor)
    }

    pub async fn actualizar_autor(
        pool: &DbPool,
        actor_id: i64,
        id: i32,
        req: ActualizarAutorRequest,
    ) -> Result<Autor, AppError> {
        validate_author_text(&req.nombre, "El nombre del autor")?;
        validate_author_text(&req.apellidos, "Los apellidos del autor")?;
        validar_largo("El nombre del autor", req.nombre.trim(), AUTOR_NOMBRE_MAX)?;
        validar_largo("Los apellidos del autor", req.apellidos.trim(), AUTOR_APELLIDOS_MAX)?;

        let mut req = req;
        req.nombre = req.nombre.trim().to_string();
        req.apellidos = req.apellidos.trim().to_string();
        validar_unico(pool, &req.nombre, &req.apellidos, Some(id)).await?;

        let autor = AutorRepo::actualizar_autor(pool, id, &req).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "ACTUALIZAR AUTOR",
            &format!(
                "Autor actualizado: {} (ID: {})",
                autor.nombre_completo(),
                id
            ),
        )
        .await;

        Ok(autor)
    }

    pub async fn eliminar_autor(pool: &DbPool, actor_id: i64, id: i32) -> Result<(), AppError> {
        let autor = Self::obtener_autor(pool, id).await?;
        if autor.total_libros > 0 {
            return Err(AppError::BadRequest(format!(
                "No puedes eliminar al autor porque tiene {} título(s) asociado(s)",
                autor.total_libros
            )));
        }

        AutorRepo::eliminar_autor(pool, id).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "ELIMINAR AUTOR",
            &format!("Autor eliminado (ID: {})", id),
        )
        .await;

        Ok(())
    }
}