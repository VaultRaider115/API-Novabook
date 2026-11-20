use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    limites::{OBSERVACIONES_MAX, REFERENCIA_SOLICITUD_MAX, validar_largo, validar_largo_opt},
    productos::{
        model::{
            ConteoRequest, CrearSolicitudEditorialRequest, DIAS_ANEJAMIENTO, DevolucionPendiente,
            DevolucionPendienteItem, InventarioDetalle, InventarioResumen, RevisionInventarioRequest,
            SolicitudEditorial,
        },
        repo::InventarioMensualRepo,
    },
};

pub struct InventarioMensualService;

const MAX_CONTEO: i32 = 99_999;
const MAX_EANS_SOLICITUD: usize = 200;

/// Valida y deja sin repetidos los EAN de una solicitud de editorial.
fn normalizar_solicitud(req: CrearSolicitudEditorialRequest) -> Result<CrearSolicitudEditorialRequest, AppError> {
    let referencia = req.referencia.trim().to_string();
    if referencia.is_empty() {
        return Err(AppError::BadRequest(
            "Indica la referencia del correo de la editorial (asunto o fecha)".to_string(),
        ));
    }
    validar_largo("La referencia", &referencia, REFERENCIA_SOLICITUD_MAX)?;
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

/// Completa cada título (total a devolver y faltantes) y suma el encabezado.
/// Los títulos con piezas por encontrar van primero.
fn armar_pendientes(id_inventario: i64, mut items: Vec<DevolucionPendienteItem>) -> DevolucionPendiente {
    for it in &mut items {
        it.a_devolver = it.en_piso + it.en_bodega + it.apartadas;
        it.faltantes = (it.en_piso - it.encontradas).max(0);
    }
    items.sort_by(|a, b| (b.faltantes > 0).cmp(&(a.faltantes > 0)).then_with(|| a.titulo.cmp(&b.titulo)));
    DevolucionPendiente {
        id_inventario,
        titulos: items.len() as i32,
        titulos_con_faltantes: items.iter().filter(|i| i.faltantes > 0).count() as i32,
        piezas_a_devolver: items.iter().map(|i| i.a_devolver).sum(),
        en_piso: items.iter().map(|i| i.en_piso).sum(),
        // Si encontró más de lo que dice el sistema, no cuenta de más en el total.
        encontradas: items.iter().map(|i| i.encontradas.min(i.en_piso)).sum(),
        faltantes: items.iter().map(|i| i.faltantes).sum(),
        items,
    }
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

    /// Títulos que van a devolución en este inventario: cuánto hay que
    /// regresar y cuántas piezas de piso faltan por encontrar.
    pub async fn pendientes_devolucion(pool: &DbPool, id: i64) -> Result<DevolucionPendiente, AppError> {
        let detalle = Self::detalle(pool, id).await?;
        let items = InventarioMensualRepo::pendientes_devolucion(pool, id, detalle.resumen.dias_anejamiento).await?;
        Ok(armar_pendientes(id, items))
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
        validar_largo_opt("Las observaciones", obs.as_deref(), OBSERVACIONES_MAX)?;
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
    use super::{armar_pendientes, normalizar_solicitud};
    use crate::productos::model::{CrearSolicitudEditorialRequest, DevolucionPendienteItem};

    fn item(titulo: &str, piso: i32, bodega: i32, apartadas: i32, encontradas: i32) -> DevolucionPendienteItem {
        DevolucionPendienteItem {
            codigo_ean: 1,
            titulo: titulo.into(),
            proveedor: "Planeta".into(),
            motivo_devolucion: "SOLICITUD_EDITORIAL".into(),
            en_piso: piso,
            en_bodega: bodega,
            apartadas,
            a_devolver: 0,
            encontradas,
            faltantes: 0,
        }
    }

    #[test]
    fn pendientes_cuentan_total_a_devolver_y_faltantes() {
        // 3 de 5 encontrados en piso; además 10 en bodega y 1 apartado.
        let r = armar_pendientes(7, vec![item("Momo", 5, 10, 1, 3)]);
        let m = &r.items[0];
        assert_eq!(m.a_devolver, 16);
        assert_eq!(m.faltantes, 2);
        assert_eq!((r.titulos, r.titulos_con_faltantes, r.piezas_a_devolver), (1, 1, 16));
        assert_eq!((r.en_piso, r.encontradas, r.faltantes), (5, 3, 2));
    }

    #[test]
    fn pendientes_no_cuentan_de_mas_ni_faltantes_negativos() {
        // Encontró 7 y el sistema dice 5: faltan 0 y en el total cuentan 5.
        let r = armar_pendientes(7, vec![item("Momo", 5, 0, 0, 7), item("Sin piso", 0, 4, 0, 0)]);
        assert!(r.items.iter().all(|i| i.faltantes == 0));
        assert_eq!((r.encontradas, r.faltantes, r.titulos_con_faltantes), (5, 0, 0));
        assert_eq!(r.piezas_a_devolver, 9);
    }

    #[test]
    fn pendientes_con_faltantes_van_primero() {
        let r = armar_pendientes(7, vec![item("A completo", 2, 0, 0, 2), item("B falta", 3, 0, 0, 1)]);
        assert_eq!(r.items[0].titulo, "B falta");
        assert_eq!(r.items[1].titulo, "A completo");
    }

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
