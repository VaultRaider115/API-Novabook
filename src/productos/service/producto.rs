use crate::{
    db::{DbPool, log_bitacora},
    error::AppError,
    limites::{PERIODICIDAD_MAX, TITULO_MAX, validar_largo, validar_largo_opt, validar_monto},
    productos::{
        model::{
            Libro, RegistrarLibroRequest, RegistrarRevistaRequest, Revista, TipoProducto,
            validar_patron_ean13, validar_sku,
        },
        repo::{ProductoRepo, UBICACION_BODEGA},
    },
};

pub struct ProductoService;

/// Diagrama, paso 1: el alta es solo para productos nuevos. Si el EAN ya
/// existe, las piezas entran por Compras a Proveedor (no se sobrescribe el
/// título, precio ni SKU del producto registrado).
async fn validar_ean_nuevo(pool: &DbPool, ean: i64) -> Result<(), AppError> {
    let fila = sqlx::query(
        "SELECT COALESCE(l.nombre_libro, r.nombre_revista, '') AS titulo FROM productos p          LEFT JOIN libros l ON l.codigo_ean = p.codigo_ean          LEFT JOIN revistas r ON r.codigo_ean = p.codigo_ean WHERE p.codigo_ean = ?",
    )
    .bind(ean)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;
    if let Some(f) = fila {
        let titulo: String = sqlx::Row::get(&f, "titulo");
        return Err(AppError::Conflict(format!(
            "El EAN {} ya está registrado como \"{}\"; para agregar piezas usa Compras a Proveedor",
            ean, titulo
        )));
    }
    Ok(())
}

/// SKU de 6 a 7 dígitos, obligatorio en el alta y sin repetirse entre productos.
async fn validar_sku_producto(pool: &DbPool, ean: i64, sku: Option<i64>) -> Result<(), AppError> {
    let Some(sku) = sku else {
        return Err(AppError::BadRequest(
            "El SKU es obligatorio para dar de alta un producto nuevo".to_string(),
        ));
    };
    validar_sku(sku).map_err(AppError::BadRequest)?;

    let otro = sqlx::query("SELECT codigo_ean FROM productos WHERE SKU = ? AND codigo_ean <> ? LIMIT 1")
        .bind(sku)
        .bind(ean)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?;
    if let Some(row) = otro {
        let ean_otro: i64 = sqlx::Row::get(&row, "codigo_ean");
        return Err(AppError::Conflict(format!(
            "El SKU {} ya está asignado al producto con EAN {}",
            sku, ean_otro
        )));
    }
    Ok(())
}

impl ProductoService {
    // ========================================================================
    // Registro de Libros y Revistas con Validación EAN (Bodega)
    // ========================================================================

    pub async fn registrar_libro(
        pool: &DbPool,
        actor_id: i64,
        req: RegistrarLibroRequest,
    ) -> Result<Libro, AppError> {
        // Validación obligatoria de patrón EAN-13 y dígito verificador
        let tipo_detectado =
            validar_patron_ean13(req.codigo_ean).map_err(AppError::BadRequest)?;

        if tipo_detectado != TipoProducto::Libro {
            return Err(AppError::BadRequest(format!(
                "El código EAN {} corresponde a una Revista (ISSN: prefijo 977), no a un Libro",
                req.codigo_ean
            )));
        }

        // Evitar duplicados cruzados en revistas
        if (ProductoRepo::buscar_revista(pool, req.codigo_ean, UBICACION_BODEGA).await?).is_some() {
            return Err(AppError::BadRequest(format!(
                "El código EAN {} ya está registrado en el sistema como una Revista",
                req.codigo_ean
            )));
        }
        validar_ean_nuevo(pool, req.codigo_ean).await?;
        validar_sku_producto(pool, req.codigo_ean, req.sku).await?;

        if req.cantidad < 0 {
            return Err(AppError::BadRequest(
                "La cantidad no puede ser negativa".to_string(),
            ));
        }
        validar_monto("El precio de venta", req.precio)?;
        validar_largo("El título", req.nombre_libro.trim(), TITULO_MAX)?;

        let ubicacion = req.id_ubicacion.unwrap_or(UBICACION_BODEGA);
        let libro = ProductoRepo::guardar_o_actualizar_libro(pool, &req, ubicacion).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "REGISTRO LIBRO BODEGA",
            &format!(
                "Alta de libro '{}' con EAN {} ({} pza(s) en bodega)",
                req.nombre_libro, req.codigo_ean, req.cantidad
            ),
        )
        .await;

        Ok(libro)
    }

    pub async fn registrar_revista(
        pool: &DbPool,
        actor_id: i64,
        req: RegistrarRevistaRequest,
    ) -> Result<Revista, AppError> {
        // Validación obligatoria de patrón EAN-13 y dígito verificador
        let tipo_detectado =
            validar_patron_ean13(req.codigo_ean).map_err(AppError::BadRequest)?;

        if tipo_detectado != TipoProducto::Revista {
            return Err(AppError::BadRequest(format!(
                "El código EAN {} corresponde a un Libro (ISBN: prefijo 978/979), no a una Revista",
                req.codigo_ean
            )));
        }

        // Evitar duplicados cruzados en libros
        if (ProductoRepo::buscar_libro(pool, req.codigo_ean, UBICACION_BODEGA).await?).is_some() {
            return Err(AppError::BadRequest(format!(
                "El código EAN {} ya está registrado en el sistema como un Libro",
                req.codigo_ean
            )));
        }
        validar_ean_nuevo(pool, req.codigo_ean).await?;
        validar_sku_producto(pool, req.codigo_ean, req.sku).await?;

        if req.cantidad < 0 {
            return Err(AppError::BadRequest(
                "La cantidad no puede ser negativa".to_string(),
            ));
        }
        validar_monto("El precio de venta", req.precio)?;
        validar_largo("El título", req.nombre_revista.trim(), TITULO_MAX)?;
        validar_largo_opt("La periodicidad", req.periodicidad.as_deref().map(str::trim), PERIODICIDAD_MAX)?;

        let ubicacion = req.id_ubicacion.unwrap_or(UBICACION_BODEGA);
        let revista = ProductoRepo::guardar_o_actualizar_revista(pool, &req, ubicacion).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "REGISTRO REVISTA BODEGA",
            &format!(
                "Alta de revista '{}'{} con EAN {} ({} pza(s) en bodega)",
                req.nombre_revista,
                req.numero_edicion.map(|n| format!(" Ed. {}", n)).unwrap_or_default(),
                req.codigo_ean,
                req.cantidad
            ),
        )
        .await;

        Ok(revista)
    }
}
