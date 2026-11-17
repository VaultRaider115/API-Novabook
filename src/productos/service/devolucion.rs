use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{
            AprobarDevolucionRequest, CrearDevolucionRequest, Devolucion, HistorialDevolucionQuery,
            TipoProducto,
        },
        repo::DevolucionRepo,
    },
    services::PdfService,
    usuarios::model::RolEnum,
};

pub struct DevolucionService;

/// Máximo de las observaciones de aprobación/rechazo (devolucion.observaciones VARCHAR(200)).
const MAX_OBSERVACIONES: usize = 200;

/// Máximo de piezas por hoja/lote de devolución a proveedor.
pub const LIMITE_PIEZAS_DEVOLUCION: i32 = 60;

impl DevolucionService {
    pub async fn crear_devolucion(
        pool: &DbPool,
        actor_id: i64,
        actor_nombre: &str,
        req: CrearDevolucionRequest,
    ) -> Result<Devolucion, AppError> {
        if req.items.is_empty() {
            return Err(AppError::BadRequest(
                "Debe especificar al menos un artículo para devolución".to_string(),
            ));
        }

        for item in &req.items {
            if item.codigo_ean <= 0 || item.cantidad <= 0 || item.id_motivo <= 0 {
                return Err(AppError::BadRequest(
                    "Cada artículo debe tener EAN, cantidad e id_motivo válidos".to_string(),
                ));
            }
        }

        let total: i32 = req.items.iter().map(|i| i.cantidad).sum();
        if total > LIMITE_PIEZAS_DEVOLUCION {
            return Err(AppError::BadRequest(format!(
                "Una devolución no puede exceder {} piezas (solicitadas: {})",
                LIMITE_PIEZAS_DEVOLUCION, total
            )));
        }

        let id_devolucion = DevolucionRepo::registrar_devolucion(
            pool,
            actor_id,
            req.id_proveedor,
            req.tipo_producto,
            &req.items,
        )
        .await?;
        let devolucion = DevolucionRepo::buscar_devolucion(pool, id_devolucion)
            .await?
            .ok_or_else(|| {
                AppError::Internal(format!(
                    "Devolución #{} no encontrada después de crearla",
                    id_devolucion
                ))
            })?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "CREAR DEVOLUCIÓN",
            &format!(
                "Solicitud de devolución #{} a {} · {} pza(s)",
                devolucion.folio,
                devolucion.nombre_proveedor.as_deref().unwrap_or("proveedor"),
                devolucion.total_piezas
            ),
        )
        .await;

        Ok(Devolucion {
            vendedor_nombre: Some(actor_nombre.to_string()),
            ..devolucion
        })
    }

    pub async fn generar_pdf_devolucion(
        pool: &DbPool,
        actor_id: i64,
        actor_rol: RolEnum,
        id_devolucion: i64,
        tipo: TipoProducto,
    ) -> Result<(Vec<u8>, String), AppError> {
        let devolucion = DevolucionRepo::buscar_devolucion(pool, id_devolucion)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Devolución #{} no encontrada", id_devolucion))
            })?;

        // El acta solo existe una vez aprobada y la ve su solicitante o la jefatura/gerencia.
        if !devolucion.estado.eq_ignore_ascii_case("APROBADO") {
            return Err(AppError::Conflict(format!(
                "El acta de la devolución #{} solo está disponible cuando está aprobada (estado: {})",
                id_devolucion, devolucion.estado
            )));
        }
        let privilegiado = matches!(actor_rol, RolEnum::JefeDepartamento | RolEnum::Gerente);
        if !privilegiado && devolucion.id_usuarios != actor_id {
            return Err(AppError::Forbidden(
                "Solo el solicitante o la jefatura pueden descargar esta acta".to_string(),
            ));
        }

        let pdf_bytes = PdfService::generar_pdf_devolucion(&devolucion, tipo);
        let filename = format!(
            "devolucion_{}_{}.pdf",
            id_devolucion,
            tipo.as_str().to_lowercase()
        );

        Ok((pdf_bytes, filename))
    }

    pub async fn consultar_historial_devoluciones(
        pool: &DbPool,
        params: HistorialDevolucionQuery,
    ) -> Result<Vec<Devolucion>, AppError> {
        DevolucionRepo::listar_historial_devoluciones(pool, &params).await
    }

    /// "Mis devoluciones": solo las solicitudes del usuario autenticado, para
    /// dar seguimiento y descargar el acta cuando el Jefe la apruebe.
    pub async fn consultar_mis_devoluciones(
        pool: &DbPool,
        actor_id: i64,
        mut params: HistorialDevolucionQuery,
    ) -> Result<Vec<Devolucion>, AppError> {
        params.id_usuarios = Some(actor_id);
        params.id_proveedor = None;
        DevolucionRepo::listar_historial_devoluciones(pool, &params).await
    }

    pub async fn aprobar_devolucion(
        pool: &DbPool,
        actor_id: i64,
        actor_nombre: &str,
        id_devolucion: i64,
        req: AprobarDevolucionRequest,
    ) -> Result<Devolucion, AppError> {
        let actual = DevolucionRepo::buscar_devolucion(pool, id_devolucion)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Devolución #{} no encontrada", id_devolucion))
            })?;

        let notas = req.notas.as_deref().map(str::trim).filter(|n| !n.is_empty());
        if !req.aprobar && notas.is_none() {
            return Err(AppError::BadRequest(
                "Debes indicar las observaciones del rechazo".to_string(),
            ));
        }
        // Columna devolucion.observaciones VARCHAR(200).
        if notas.is_some_and(|n| n.chars().count() > MAX_OBSERVACIONES) {
            return Err(AppError::BadRequest(format!(
                "Las observaciones no pueden exceder {} caracteres", MAX_OBSERVACIONES
            )));
        }

        let nuevo_estado = if req.aprobar {
            "APROBADO"
        } else {
            "RECHAZADO"
        };
        if !actual.estado.eq_ignore_ascii_case("PENDIENTE") {
            return Err(AppError::Conflict(format!(
                "La devolución #{} ya fue evaluada (estado: {})",
                id_devolucion, actual.estado
            )));
        }
        DevolucionRepo::actualizar_estado_devolucion(
            pool,
            id_devolucion,
            actor_id,
            nuevo_estado,
            req.aprobar,
            notas,
        )
            .await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            if req.aprobar { "APROBAR DEVOLUCIÓN" } else { "RECHAZAR DEVOLUCIÓN" },
            &match notas {
                Some(n) => format!(
                    "Devolución #{} {}. Observaciones: {}",
                    id_devolucion, if req.aprobar { "aprobada" } else { "rechazada" }, n
                ),
                None => format!(
                    "Devolución #{} {}",
                    id_devolucion, if req.aprobar { "aprobada" } else { "rechazada" }
                ),
            },
        )
        .await;

        Ok(Devolucion {
            estado: nuevo_estado.to_string(),
            autorizado_por: Some(actor_nombre.to_string()),
            ..actual
        })
    }
}
