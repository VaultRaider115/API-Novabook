use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    limites::{OBSERVACIONES_REQUISICION_MAX, validar_largo_opt},
    productos::{
        model::{
            FiltroRequisiciones, PartidaRequisicion, RecepcionRequest, RequisicionDetalle,
            RequisicionRequest, RequisicionResponse, RequisicionResumen, TipoMovimiento,
            TipoProducto, TrasladoRequest, TrasladoResponse,
        },
        repo::{MovimientoRepo, RecepcionRepo, UBICACION_BODEGA, UBICACION_TIENDA},
    },
};

pub struct MovimientoService;

/// Máximo de partidas por requisición (un documento impreso razonable).
const MAX_PARTIDAS_REQUISICION: usize = 100;

/// Valida las partidas y suma las de un mismo EAN en una sola.
fn normalizar_partidas(entrada: Vec<PartidaRequisicion>) -> Result<Vec<PartidaRequisicion>, AppError> {
    if entrada.is_empty() {
        return Err(AppError::BadRequest("La requisición debe tener al menos una partida".to_string()));
    }
    if entrada.len() > MAX_PARTIDAS_REQUISICION {
        return Err(AppError::BadRequest(format!(
            "La requisición no puede tener más de {} partidas", MAX_PARTIDAS_REQUISICION
        )));
    }
    let mut items: Vec<PartidaRequisicion> = Vec::new();
    for it in entrada {
        if it.codigo_ean <= 0 {
            return Err(AppError::BadRequest("Código EAN inválido".to_string()));
        }
        if it.cantidad <= 0 {
            return Err(AppError::BadRequest(format!(
                "La cantidad del EAN {} debe ser mayor a 0", it.codigo_ean
            )));
        }
        match items.iter_mut().find(|p| p.codigo_ean == it.codigo_ean) {
            Some(p) => p.cantidad += it.cantidad,
            None => items.push(it),
        }
    }
    Ok(items)
}

impl MovimientoService {
    pub async fn listar_requisiciones(
        pool: &DbPool,
        filtro: FiltroRequisiciones,
    ) -> Result<Vec<RequisicionResumen>, AppError> {
        RecepcionRepo::listar(pool, &filtro).await
    }

    pub async fn obtener_requisicion(pool: &DbPool, folio: &str) -> Result<RequisicionDetalle, AppError> {
        RecepcionRepo::obtener(pool, folio.trim()).await
    }

    /// Pasos 2 y 3: el Vendedor recibe la salida. Si lo recibido no coincide,
    /// la requisición queda CORREGIDA (stock y movimientos ajustados) y se
    /// reimprime con la respuesta; además registra el mueble de cada libro.
    pub async fn recibir_requisicion(
        pool: &DbPool,
        actor_id: i64,
        folio: &str,
        req: RecepcionRequest,
    ) -> Result<RequisicionDetalle, AppError> {
        let folio = folio.trim();
        validar_largo_opt("Las observaciones", req.observaciones.as_deref(), OBSERVACIONES_REQUISICION_MAX)?;
        let (diferencias, acomodos) = RecepcionRepo::recibir(pool, folio, actor_id, &req).await?;

        let mut detalle = if diferencias.is_empty() {
            format!("Requisición {} recibida en piso sin diferencias", folio)
        } else {
            format!("Requisición {} corregida al recibir: {}", folio, diferencias.join(", "))
        };
        if !acomodos.is_empty() {
            detalle.push_str(&format!(" · Mueble: {}", acomodos.join(", ")));
        }
        let _ = log_bitacora(pool, actor_id, "RECEPCIÓN DE REQUISICIÓN EN PISO", &detalle).await;

        RecepcionRepo::obtener(pool, folio).await
    }

    /// Requisición de varias partidas con folio. Bodega → Piso (salida) o
    /// Piso → Bodega (entrada); todo o nada.
    pub async fn registrar_requisicion(
        pool: &DbPool,
        actor_id: i64,
        tipo: TipoMovimiento,
        req: RequisicionRequest,
    ) -> Result<RequisicionResponse, AppError> {
        validar_largo_opt("Las observaciones", req.observaciones.as_deref(), OBSERVACIONES_REQUISICION_MAX)?;
        let items = normalizar_partidas(req.items)?;

        let (origen, destino, accion) = match tipo {
            TipoMovimiento::BodegaATienda => (UBICACION_BODEGA, UBICACION_TIENDA, "REQUISICIÓN DE SALIDA BODEGA A PISO"),
            TipoMovimiento::TiendaABodega => (UBICACION_TIENDA, UBICACION_BODEGA, "REQUISICIÓN DE ENTRADA PISO A BODEGA"),
        };
        let reg = MovimientoRepo::registrar_requisicion(
            pool, &items, origen, destino, actor_id, req.observaciones.as_deref(),
        )
        .await?;

        let total_piezas: i32 = reg.items.iter().map(|p| p.cantidad).sum();
        let _ = log_bitacora(
            pool,
            actor_id,
            accion,
            &format!(
                "Requisición {} {} ({}): {} unidades en {} partida(s)",
                reg.folio,
                if origen == UBICACION_BODEGA { "Bodega-Piso" } else { "Piso-Bodega" },
                reg.editorial,
                total_piezas,
                reg.items.len()
            ),
        )
        .await;

        Ok(RequisicionResponse {
            folio: reg.folio,
            fecha_hora: reg.fecha_hora,
            tipo_movimiento: tipo,
            total_piezas,
            editorial: reg.editorial,
            items: reg.items,
        })
    }

    pub async fn traslado_bodega_a_tienda_libros(
        pool: &DbPool,
        actor_id: i64,
        req: TrasladoRequest,
    ) -> Result<TrasladoResponse, AppError> {
        validar_largo_opt("Las observaciones", req.observaciones.as_deref(), OBSERVACIONES_REQUISICION_MAX)?;
        if req.codigo_ean <= 0 {
            return Err(AppError::BadRequest("Código EAN inválido".to_string()));
        }
        if req.cantidad <= 0 {
            return Err(AppError::BadRequest(
                "La cantidad a trasladar debe ser mayor a 0".to_string(),
            ));
        }

        let (restante_bodega, nuevo_tienda) = MovimientoRepo::trasladar_stock_libro(
            pool,
            req.codigo_ean,
            req.cantidad,
            UBICACION_BODEGA,
            UBICACION_TIENDA,
            actor_id,
            req.observaciones.as_deref(),
        )
        .await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "TRASLADO BODEGA A TIENDA (LIBROS)",
            &format!(
                "Traslado Bodega-Piso {1} unidades de EAN {0} (quedan {2} en bodega, {3} en piso)",
                req.codigo_ean, req.cantidad, restante_bodega, nuevo_tienda
            ),
        )
        .await;

        Ok(TrasladoResponse {
            codigo_ean: req.codigo_ean,
            tipo_producto: TipoProducto::Libro,
            tipo_movimiento: TipoMovimiento::BodegaATienda,
            cantidad_trasladada: req.cantidad,
            stock_origen_restante: restante_bodega,
            stock_destino_nuevo: nuevo_tienda,
            mensaje: "Traslado de libros de bodega a tienda completado exitosamente".to_string(),
        })
    }

    pub async fn traslado_bodega_a_tienda_revistas(
        pool: &DbPool,
        actor_id: i64,
        req: TrasladoRequest,
    ) -> Result<TrasladoResponse, AppError> {
        validar_largo_opt("Las observaciones", req.observaciones.as_deref(), OBSERVACIONES_REQUISICION_MAX)?;
        if req.codigo_ean <= 0 {
            return Err(AppError::BadRequest(
                "Código EAN / ISSN inválido".to_string(),
            ));
        }
        if req.cantidad <= 0 {
            return Err(AppError::BadRequest(
                "La cantidad a trasladar debe ser mayor a 0".to_string(),
            ));
        }

        let (restante_bodega, nuevo_tienda) = MovimientoRepo::trasladar_stock_revista(
            pool,
            req.codigo_ean,
            req.cantidad,
            UBICACION_BODEGA,
            UBICACION_TIENDA,
            actor_id,
            req.observaciones.as_deref(),
        )
        .await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "TRASLADO BODEGA A TIENDA (REVISTAS)",
            &format!(
                "Traslado Bodega-Piso {1} unidades de EAN {0} (quedan {2} en bodega, {3} en piso)",
                req.codigo_ean, req.cantidad, restante_bodega, nuevo_tienda
            ),
        )
        .await;

        Ok(TrasladoResponse {
            codigo_ean: req.codigo_ean,
            tipo_producto: TipoProducto::Revista,
            tipo_movimiento: TipoMovimiento::BodegaATienda,
            cantidad_trasladada: req.cantidad,
            stock_origen_restante: restante_bodega,
            stock_destino_nuevo: nuevo_tienda,
            mensaje: "Traslado de revistas de bodega a tienda completado exitosamente".to_string(),
        })
    }

    pub async fn traslado_tienda_a_bodega_libros(
        pool: &DbPool,
        actor_id: i64,
        req: TrasladoRequest,
    ) -> Result<TrasladoResponse, AppError> {
        validar_largo_opt("Las observaciones", req.observaciones.as_deref(), OBSERVACIONES_REQUISICION_MAX)?;
        if req.codigo_ean <= 0 {
            return Err(AppError::BadRequest("Código EAN inválido".to_string()));
        }
        if req.cantidad <= 0 {
            return Err(AppError::BadRequest(
                "La cantidad a trasladar debe ser mayor a 0".to_string(),
            ));
        }

        let (restante_tienda, nuevo_bodega) = MovimientoRepo::trasladar_stock_libro(
            pool,
            req.codigo_ean,
            req.cantidad,
            UBICACION_TIENDA,
            UBICACION_BODEGA,
            actor_id,
            req.observaciones.as_deref(),
        )
        .await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "TRASLADO TIENDA A BODEGA (LIBROS)",
            &format!(
                "Traslado Piso-Bodega {1} unidades de EAN {0} (quedan {2} en piso, {3} en bodega)",
                req.codigo_ean, req.cantidad, restante_tienda, nuevo_bodega
            ),
        )
        .await;

        Ok(TrasladoResponse {
            codigo_ean: req.codigo_ean,
            tipo_producto: TipoProducto::Libro,
            tipo_movimiento: TipoMovimiento::TiendaABodega,
            cantidad_trasladada: req.cantidad,
            stock_origen_restante: restante_tienda,
            stock_destino_nuevo: nuevo_bodega,
            mensaje: "Traslado de libros de tienda a bodega completado exitosamente".to_string(),
        })
    }

    pub async fn traslado_tienda_a_bodega_revistas(
        pool: &DbPool,
        actor_id: i64,
        req: TrasladoRequest,
    ) -> Result<TrasladoResponse, AppError> {
        validar_largo_opt("Las observaciones", req.observaciones.as_deref(), OBSERVACIONES_REQUISICION_MAX)?;
        if req.codigo_ean <= 0 {
            return Err(AppError::BadRequest(
                "Código EAN / ISSN inválido".to_string(),
            ));
        }
        if req.cantidad <= 0 {
            return Err(AppError::BadRequest(
                "La cantidad a trasladar debe ser mayor a 0".to_string(),
            ));
        }

        let (restante_tienda, nuevo_bodega) = MovimientoRepo::trasladar_stock_revista(
            pool,
            req.codigo_ean,
            req.cantidad,
            UBICACION_TIENDA,
            UBICACION_BODEGA,
            actor_id,
            req.observaciones.as_deref(),
        )
        .await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "TRASLADO TIENDA A BODEGA (REVISTAS)",
            &format!(
                "Traslado Piso-Bodega {1} unidades de EAN {0} (quedan {2} en piso, {3} en bodega)",
                req.codigo_ean, req.cantidad, restante_tienda, nuevo_bodega
            ),
        )
        .await;

        Ok(TrasladoResponse {
            codigo_ean: req.codigo_ean,
            tipo_producto: TipoProducto::Revista,
            tipo_movimiento: TipoMovimiento::TiendaABodega,
            cantidad_trasladada: req.cantidad,
            stock_origen_restante: restante_tienda,
            stock_destino_nuevo: nuevo_bodega,
            mensaje: "Traslado de revistas de tienda a bodega completado exitosamente".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PARTIDAS_REQUISICION, normalizar_partidas};
    use crate::productos::model::PartidaRequisicion;

    fn p(ean: i64, cantidad: i32) -> PartidaRequisicion {
        PartidaRequisicion { codigo_ean: ean, cantidad }
    }

    #[test]
    fn suma_ean_repetidos_y_conserva_orden() {
        let r = normalizar_partidas(vec![p(9781, 2), p(9771, 5), p(9781, 3)]).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].codigo_ean, r[0].cantidad), (9781, 5));
        assert_eq!((r[1].codigo_ean, r[1].cantidad), (9771, 5));
    }

    #[test]
    fn rechaza_vacia_cantidades_y_ean_invalidos() {
        assert!(normalizar_partidas(vec![]).is_err());
        assert!(normalizar_partidas(vec![p(9781, 0)]).is_err());
        assert!(normalizar_partidas(vec![p(9781, -2)]).is_err());
        assert!(normalizar_partidas(vec![p(0, 1)]).is_err());
    }

    #[test]
    fn limita_el_numero_de_partidas() {
        let muchas: Vec<_> = (1..=MAX_PARTIDAS_REQUISICION as i64 + 1).map(|e| p(e, 1)).collect();
        assert!(normalizar_partidas(muchas).is_err());
        let justas: Vec<_> = (1..=MAX_PARTIDAS_REQUISICION as i64).map(|e| p(e, 1)).collect();
        assert!(normalizar_partidas(justas).is_ok());
    }
}
