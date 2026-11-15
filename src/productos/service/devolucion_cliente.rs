use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    productos::{
        model::{
            CrearDevolucionClienteRequest, DevolucionClienteResponse, PartidaCambio,
            PartidaDevolucionCliente, TicketResponse,
        },
        repo::DevolucionClienteRepo,
    },
};

pub struct DevolucionClienteService;

const MAX_PARTIDAS: usize = 100;
const MAX_OBSERVACIONES: usize = 200;
const METODOS_PAGO: [&str; 3] = ["EFECTIVO", "TARJETA", "TRANSFERENCIA"];

/// Valida la petición y la deja normalizada (resolución en mayúsculas,
/// partidas agrupadas). Devuelve también el método de pago del cambio.
fn normalizar(mut req: CrearDevolucionClienteRequest) -> Result<(CrearDevolucionClienteRequest, String), AppError> {
    let resolucion = req.resolucion.trim().to_uppercase();
    if resolucion != "REEMBOLSO" && resolucion != "CAMBIO" {
        return Err(AppError::BadRequest("La resolución debe ser REEMBOLSO o CAMBIO".to_string()));
    }
    if req.items.is_empty() {
        return Err(AppError::BadRequest("Indica al menos un producto a devolver".to_string()));
    }
    if req.items.len() > MAX_PARTIDAS || req.items_cambio.len() > MAX_PARTIDAS {
        return Err(AppError::BadRequest(format!("Máximo {} partidas por devolución", MAX_PARTIDAS)));
    }
    if resolucion == "REEMBOLSO" && !req.items_cambio.is_empty() {
        return Err(AppError::BadRequest(
            "En un reembolso no se entregan productos nuevos; usa CAMBIO".to_string(),
        ));
    }
    if resolucion == "CAMBIO" && req.items_cambio.is_empty() {
        return Err(AppError::BadRequest("Indica los productos que se lleva el cliente".to_string()));
    }

    // Devueltos: se agrupan por (EAN, con_defecto): del mismo título puede haber
    // piezas buenas (a piso) y dañadas (a apartados).
    let mut items: Vec<PartidaDevolucionCliente> = Vec::new();
    for it in req.items {
        if it.codigo_ean <= 0 || it.cantidad <= 0 {
            return Err(AppError::BadRequest(format!(
                "Partida inválida (EAN {}, cantidad {}): la cantidad debe ser mayor a 0",
                it.codigo_ean, it.cantidad
            )));
        }
        match items.iter_mut().find(|p| p.codigo_ean == it.codigo_ean && p.con_defecto == it.con_defecto) {
            Some(p) => p.cantidad += it.cantidad,
            None => items.push(it),
        }
    }
    let mut cambio: Vec<PartidaCambio> = Vec::new();
    for it in req.items_cambio {
        if it.codigo_ean <= 0 || it.cantidad <= 0 {
            return Err(AppError::BadRequest(format!(
                "Producto de cambio inválido (EAN {}, cantidad {})",
                it.codigo_ean, it.cantidad
            )));
        }
        match cambio.iter_mut().find(|p| p.codigo_ean == it.codigo_ean) {
            Some(p) => p.cantidad += it.cantidad,
            None => cambio.push(it),
        }
    }

    let metodo = req
        .metodo_pago
        .as_deref()
        .map(|m| m.trim().to_uppercase())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "EFECTIVO".to_string());
    if !METODOS_PAGO.contains(&metodo.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Método de pago inválido: {} (usa EFECTIVO, TARJETA o TRANSFERENCIA)",
            metodo
        )));
    }

    let observaciones = req.observaciones.map(|o| o.trim().to_string()).filter(|o| !o.is_empty());
    if observaciones.as_ref().is_some_and(|o| o.chars().count() > MAX_OBSERVACIONES) {
        return Err(AppError::BadRequest(format!(
            "Las observaciones no pueden exceder {} caracteres",
            MAX_OBSERVACIONES
        )));
    }

    req.resolucion = resolucion;
    req.items = items;
    req.items_cambio = cambio;
    req.observaciones = observaciones;
    Ok((req, metodo))
}

impl DevolucionClienteService {
    pub async fn consultar_ticket(pool: &DbPool, id_venta: i64) -> Result<TicketResponse, AppError> {
        DevolucionClienteRepo::buscar_ticket(pool, id_venta)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("El ticket #{} no existe", id_venta)))
    }

    pub async fn registrar(
        pool: &DbPool,
        actor_id: i64,
        req: CrearDevolucionClienteRequest,
    ) -> Result<DevolucionClienteResponse, AppError> {
        let (req, metodo) = normalizar(req)?;
        let res = DevolucionClienteRepo::registrar(pool, actor_id, &req, &metodo).await?;

        let piezas: i32 = res.items.iter().map(|i| i.cantidad).sum();
        let con_defecto: i32 = res.items.iter().filter(|i| i.con_defecto).map(|i| i.cantidad).sum();
        let detalle = match res.id_venta_cambio {
            Some(nuevo) => format!(
                "Cambio Ticket #{} → Ticket #{} · {} pza(s) ({} con defecto) · diferencia ${:.2}",
                res.id_venta, nuevo, piezas, con_defecto, res.diferencia_cobrada
            ),
            None => format!(
                "Devolución de cliente Ticket #{} · {} pza(s) ({} con defecto) · reembolso ${:.2}",
                res.id_venta, piezas, con_defecto, res.reembolso
            ),
        };
        let _ = log_bitacora(pool, actor_id, "DEVOLUCIÓN DE CLIENTE", &detalle).await;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::normalizar;
    use crate::productos::model::{CrearDevolucionClienteRequest, PartidaCambio, PartidaDevolucionCliente};

    fn req(resolucion: &str, items: Vec<(i64, i32, bool)>, cambio: Vec<(i64, i32)>) -> CrearDevolucionClienteRequest {
        CrearDevolucionClienteRequest {
            id_venta: 1,
            resolucion: resolucion.into(),
            items: items
                .into_iter()
                .map(|(e, c, d)| PartidaDevolucionCliente { codigo_ean: e, cantidad: c, con_defecto: d })
                .collect(),
            items_cambio: cambio.into_iter().map(|(e, c)| PartidaCambio { codigo_ean: e, cantidad: c }).collect(),
            metodo_pago: None,
            observaciones: None,
        }
    }

    #[test]
    fn agrupa_por_ean_y_defecto() {
        let (r, metodo) = normalizar(req("reembolso", vec![(1, 1, false), (1, 2, false), (1, 1, true)], vec![])).unwrap();
        assert_eq!(r.resolucion, "REEMBOLSO");
        assert_eq!(metodo, "EFECTIVO");
        assert_eq!(r.items.len(), 2, "buenas y dañadas del mismo EAN van separadas");
        assert_eq!((r.items[0].cantidad, r.items[0].con_defecto), (3, false));
        assert_eq!((r.items[1].cantidad, r.items[1].con_defecto), (1, true));
    }

    #[test]
    fn reglas_de_reembolso_y_cambio() {
        assert!(normalizar(req("REEMBOLSO", vec![(1, 1, false)], vec![(2, 1)])).is_err());
        assert!(normalizar(req("CAMBIO", vec![(1, 1, false)], vec![])).is_err());
        assert!(normalizar(req("CAMBIO", vec![(1, 1, false)], vec![(2, 1)])).is_ok());
        assert!(normalizar(req("REGALO", vec![(1, 1, false)], vec![])).is_err());
        assert!(normalizar(req("REEMBOLSO", vec![], vec![])).is_err());
        assert!(normalizar(req("REEMBOLSO", vec![(1, 0, false)], vec![])).is_err());
    }

    #[test]
    fn valida_metodo_de_pago_y_observaciones() {
        let mut r = req("CAMBIO", vec![(1, 1, false)], vec![(2, 1)]);
        r.metodo_pago = Some("cheque".into());
        assert!(normalizar(r).is_err());
        let mut r = req("CAMBIO", vec![(1, 1, false)], vec![(2, 1)]);
        r.metodo_pago = Some(" tarjeta ".into());
        assert_eq!(normalizar(r).unwrap().1, "TARJETA");
        let mut r = req("REEMBOLSO", vec![(1, 1, false)], vec![]);
        r.observaciones = Some("x".repeat(201));
        assert!(normalizar(r).is_err());
    }
}
