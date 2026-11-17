use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{
            ConteoRequest, CrearSolicitudEditorialRequest, DIAS_ANEJAMIENTO, InventarioDetalle, InventarioResumen,
            RevisionInventarioRequest, SolicitudEditorial,
        },
        repo::InventarioMensualRepo,
    },
};

pub struct InventarioMensualService;

const MAX_CONTEO: i32 = 99_999;
const MAX_OBSERVACIONES: usize = 200; // inventario.observaciones VARCHAR(200)
const MAX_REFERENCIA: usize = 150; // solicitud_editorial.referencia VARCHAR(150)
const MAX_EANS_SOLICITUD: usize = 200;

/// Valida y deja sin repetidos los EAN de una solicitud de editorial.
fn normalizar_solicitud(req: CrearSolicitudEditorialRequest) -> Result<CrearSolicitudEditorialRequest, AppError> {
    let referencia = req.referencia.trim().to_string();
    if referencia.is_empty() {
        return Err(AppError::BadRequest(
            "Indica la referencia del correo de la editorial (asunto o fecha)".to_string(),
        ));
    }
    if referencia.chars().count() > MAX_REFERENCIA {
        return Err(AppError::BadRequest(format!(
            "La referencia no puede exceder {} caracteres",
            MAX_REFERENCIA
        )));
    }
    let mut eans: Vec<i64> = Vec::new();
    for e in req.eans {
        if e <= 0 {
            return Err(AppError::BadRequest(format!("EAN inválido: {}", e)));
        }
        if !eans.contains(&e) {
            eans.push(e);
        }
    }
    if eans.is_empty() {
        return Err(AppError::BadRequest("La solicitud debe incluir al menos un EAN".to_string()));
    }
    if eans.len() > MAX_EANS_SOLICITUD {
        return Err(AppError::BadRequest(format!("Máximo {} EAN por solicitud", MAX_EANS_SOLICITUD)));
    }
    Ok(CrearSolicitudEditorialRequest { id_proveedor: req.id_proveedor, referencia, eans })
}

impl InventarioMensualService {
    async fn detalle(pool: &DbPool, id: i64) -> Result<InventarioDetalle, AppError> {
        InventarioMensualRepo::obtener(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("El inventario #{} no existe", id)))
    }

    // --- Vendedor -----------------------------------------------------------

    /// Inventario ABIERTO en curso, si lo hay.
    pub async fn actual(pool: &DbPool) -> Result<Option<InventarioDetalle>, AppError> {
        match InventarioMensualRepo::abierto(pool).await? {
            Some(id) => Ok(Some(Self::detalle(pool, id).await?)),
            None => Ok(None),
        }
    }

    pub async fn abrir(pool: &DbPool, actor_id: i64) -> Result<InventarioDetalle, AppError> {
        let id = InventarioMensualRepo::abrir(pool, actor_id, DIAS_ANEJAMIENTO).await?;
        let _ = log_bitacora(pool, actor_id, "INVENTARIO MENSUAL ABIERTO",
            &format!("Inicio del conteo del inventario mensual #{}", id)).await;
        Self::detalle(pool, id).await
    }

    pub async fn registrar_conteo(pool: &DbPool, id: i64, req: ConteoRequest) -> Result<(), AppError> {
        if req.codigo_ean <= 0 {
            return Err(AppError::BadRequest("Código EAN inválido".to_string()));
        }
        if !(0..=MAX_CONTEO).contains(&req.cantidad) {
            return Err(AppError::BadRequest(format!("La cantidad contada debe estar entre 0 y {}", MAX_CONTEO)));
        }
        InventarioMensualRepo::registrar_conteo(pool, id, req.codigo_ean, req.cantidad).await
    }

    pub async fn cerrar(pool: &DbPool, actor_id: i64, id: i64) -> Result<InventarioDetalle, AppError> {
        InventarioMensualRepo::cerrar(pool, id).await?;
        let d = Self::detalle(pool, id).await?;
        let _ = log_bitacora(pool, actor_id, "INVENTARIO MENSUAL CERRADO", &format!(
            "Inventario #{}: {} producto(s), {} con diferencia, {} a devolución",
            id, d.resumen.total_productos, d.resumen.con_diferencia, d.resumen.a_devolucion
        )).await;
        Ok(d)
    }

    // --- Jefe -----------------------------------------------------------------

    pub async fn listar(pool: &DbPool) -> Result<Vec<InventarioResumen>, AppError> {
        InventarioMensualRepo::listar(pool).await
    }

    pub async fn obtener(pool: &DbPool, id: i64) -> Result<InventarioDetalle, AppError> {
        Self::detalle(pool, id).await
    }

    pub async fn revisar(
        pool: &DbPool,
        actor_id: i64,
        id: i64,
        req: RevisionInventarioRequest,
    ) -> Result<InventarioDetalle, AppError> {
        let obs = req.observaciones.map(|o| o.trim().to_string()).filter(|o| !o.is_empty());
        if !req.aprobar && obs.is_none() {
            return Err(AppError::BadRequest("Indica por qué se rechaza el inventario".to_string()));
        }
        if obs.as_ref().is_some_and(|o| o.chars().count() > MAX_OBSERVACIONES) {
            return Err(AppError::BadRequest(format!(
                "Las observaciones no pueden exceder {} caracteres",
                MAX_OBSERVACIONES
            )));
        }
        let (productos, sumadas, descontadas) =
            InventarioMensualRepo::revisar(pool, id, actor_id, req.aprobar, obs.as_deref()).await?;
        let detalle = if req.aprobar {
            format!("Inventario #{} aprobado: ajuste de piso en {} producto(s) (+{} / -{} pzas)",
                id, productos, sumadas, descontadas)
        } else {
            format!("Inventario #{} rechazado. Observaciones: {}", id, obs.as_deref().unwrap_or(""))
        };
        let _ = log_bitacora(pool, actor_id,
            if req.aprobar { "AJUSTE DE INVENTARIO" } else { "INVENTARIO MENSUAL RECHAZADO" }, &detalle).await;
        Self::detalle(pool, id).await
    }

    pub async fn listar_solicitudes(pool: &DbPool) -> Result<Vec<SolicitudEditorial>, AppError> {
        InventarioMensualRepo::listar_solicitudes(pool).await
    }

    pub async fn crear_solicitud(
        pool: &DbPool,
        actor_id: i64,
        req: CrearSolicitudEditorialRequest,
    ) -> Result<Vec<SolicitudEditorial>, AppError> {
        let req = normalizar_solicitud(req)?;
        let id = InventarioMensualRepo::crear_solicitud(pool, actor_id, req.id_proveedor, &req.referencia, &req.eans).await?;
        let _ = log_bitacora(pool, actor_id, "SOLICITUD DE DEVOLUCIÓN DE EDITORIAL", &format!(
            "Solicitud #{} ({} título(s)): {}", id, req.eans.len(), req.referencia
        )).await;
        Self::listar_solicitudes(pool).await
    }

    pub async fn atender_solicitud(pool: &DbPool, actor_id: i64, id: i64) -> Result<(), AppError> {
        InventarioMensualRepo::atender_solicitud(pool, id).await?;
        let _ = log_bitacora(pool, actor_id, "SOLICITUD DE DEVOLUCIÓN DE EDITORIAL",
            &format!("Solicitud #{} marcada como atendida", id)).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::normalizar_solicitud;
    use crate::productos::model::CrearSolicitudEditorialRequest;

    fn req(referencia: &str, eans: Vec<i64>) -> CrearSolicitudEditorialRequest {
        CrearSolicitudEditorialRequest { id_proveedor: 1, referencia: referencia.into(), eans }
    }

    #[test]
    fn solicitud_quita_repetidos_y_espacios() {
        let r = normalizar_solicitud(req("  Correo 14/11  ", vec![9786073832014, 9786073832014, 9788497776639])).unwrap();
        assert_eq!(r.referencia, "Correo 14/11");
        assert_eq!(r.eans, vec![9786073832014, 9788497776639]);
    }

    #[test]
    fn solicitud_exige_referencia_y_eans_validos() {
        assert!(normalizar_solicitud(req("   ", vec![1])).is_err());
        assert!(normalizar_solicitud(req("Correo", vec![])).is_err());
        assert!(normalizar_solicitud(req("Correo", vec![0])).is_err());
        assert!(normalizar_solicitud(req(&"x".repeat(151), vec![1])).is_err());
    }
}
