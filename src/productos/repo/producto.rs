use sqlx::Row;

use crate::{
    db::{DbPool, row_get_f64, row_get_i32, row_get_i64, row_get_opt_i32, row_get_opt_i64},
    error::AppError,
    productos::{
        model::{
            ActualizarAutorRequest, ActualizarProveedorRequest, Autor, CompraResponse,
            CrearAutorRequest, CrearProveedorRequest, Devolucion, ExistenciaInventario,
            FiltroExistencias, Genero, HistorialDevolucionQuery, ItemDevolucionRequest, Libro,
            MovimientoDiarioItem, Proveedor, RegistrarCompraRequest, RegistrarLibroRequest,
            RegistrarRevistaRequest, RegistrarVentaRequest, Revista, TipoProducto, VentaResponse,
        },
        repo::{
            AutorRepo, BitacoraRepo, CompraRepo, DevolucionRepo, GeneroRepo, InventarioRepo,
            MovimientoRepo, ProveedorRepo, VentaRepo,
        },
    },
};

pub struct ProductoRepo;

impl ProductoRepo {
    // ========================================================================
    // Operaciones de Libros y Revistas (Producto)
    // ========================================================================

    // ------------------------------------------------------------------
    // Helpers internos (esquema unificado: productos + subtabla + existencias)
    // ------------------------------------------------------------------

    /// Stock total de un producto dentro de una ubicación (Piso/Bodega) y el mueble con más piezas.
    async fn stock_en_ubicacion(
        pool: &DbPool,
        codigo_ean: i64,
        id_ubicacion: i32,
    ) -> Result<(i32, i32), AppError> {
        let rows = sqlx::query(
            "SELECT e.id_mueble, e.cantidad FROM existencias e \
             JOIN mueble m ON m.id_mueble = e.id_mueble \
             WHERE e.codigo_ean = ? AND m.id_ubicacion = ? ORDER BY e.cantidad DESC",
        )
        .bind(codigo_ean)
        .bind(id_ubicacion)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        let id_mueble = rows.first().map(|r| row_get_i32(r, "id_mueble")).unwrap_or(0);
        let total: i32 = rows.iter().map(|r| row_get_i32(r, "cantidad")).sum();
        Ok((id_mueble, total))
    }

    async fn generos_de(pool: &DbPool, codigo_ean: i64) -> Result<(Vec<i32>, Option<String>), AppError> {
        let rows = sqlx::query(
            "SELECT pg.id_genero, g.genero_literario FROM productos_has_genero pg \
             JOIN genero g ON pg.id_genero = g.id_genero WHERE pg.codigo_ean = ?",
        )
        .bind(codigo_ean)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;
        let ids = rows.iter().map(|r| row_get_i32(r, "id_genero")).collect();
        let nombres: Vec<String> = rows.iter().map(|r| r.get("genero_literario")).collect();
        Ok((ids, if nombres.is_empty() { None } else { Some(nombres.join(", ")) }))
    }

    /// Valida que el mueble exista, pertenezca a la ubicación y tenga espacio para `cantidad` piezas.
    async fn validar_mueble(
        tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
        id_mueble: i32,
        id_ubicacion: i32,
        cantidad: i32,
    ) -> Result<(), AppError> {
        let row = sqlx::query(
            "SELECT id_ubicacion, capacidad, \
             COALESCE((SELECT SUM(e.cantidad) FROM existencias e WHERE e.id_mueble = mueble.id_mueble), 0) AS ocupado \
             FROM mueble WHERE id_mueble = ? FOR UPDATE",
        )
        .bind(id_mueble)
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("El mueble {} no existe", id_mueble)))?;

        if row_get_i32(&row, "id_ubicacion") != id_ubicacion {
            return Err(AppError::BadRequest(format!(
                "El mueble {} no pertenece a la ubicación seleccionada",
                id_mueble
            )));
        }
        let capacidad = row_get_i32(&row, "capacidad");
        let ocupado = row_get_i32(&row, "ocupado");
        if capacidad > 0 && ocupado + cantidad > capacidad {
            return Err(AppError::Conflict(format!(
                "El mueble {} no tiene espacio suficiente (capacidad {}, ocupado {}, se intentan agregar {})",
                id_mueble, capacidad, ocupado, cantidad
            )));
        }
        Ok(())
    }

    async fn sumar_existencia(
        tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
        codigo_ean: i64,
        id_mueble: i32,
        cantidad: i32,
    ) -> Result<(), AppError> {
        if cantidad <= 0 {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO existencias (codigo_ean, id_mueble, cantidad) VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE cantidad = cantidad + VALUES(cantidad)",
        )
        .bind(codigo_ean)
        .bind(id_mueble)
        .bind(cantidad)
        .execute(&mut **tx)
        .await
        .map_err(AppError::Database)?;
        Ok(())
    }

    async fn reemplazar_generos(
        tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
        codigo_ean: i64,
        generos: &[i32],
    ) -> Result<(), AppError> {
        if generos.is_empty() {
            return Ok(());
        }
        sqlx::query("DELETE FROM productos_has_genero WHERE codigo_ean = ?")
            .bind(codigo_ean)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
        for &g in generos {
            sqlx::query("INSERT IGNORE INTO productos_has_genero (codigo_ean, id_genero) VALUES (?, ?)")
                .bind(codigo_ean)
                .bind(g)
                .execute(&mut **tx)
                .await
                .map_err(AppError::Database)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Libros
    // ------------------------------------------------------------------

    pub async fn buscar_libro(
        pool: &DbPool,
        codigo_ean: i64,
        id_ubicacion: i32,
    ) -> Result<Option<Libro>, AppError> {
        let row = sqlx::query(
            "SELECT p.codigo_ean, p.id_proveedor, p.precio, p.SKU, l.nombre_libro \
             FROM productos p JOIN libros l ON l.codigo_ean = p.codigo_ean \
             WHERE p.codigo_ean = ?",
        )
        .bind(codigo_ean)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?;

        let Some(r) = row else { return Ok(None) };

        let (generos, generos_nombres) = Self::generos_de(pool, codigo_ean).await?;
        let aut_rows = sqlx::query(
            "SELECT al.id_autores, CONCAT(a.nombre, ' ', a.apellidos) AS autor_nom \
             FROM autores_has_libros al JOIN autores a ON al.id_autores = a.id_autores \
             WHERE al.codigo_ean = ?",
        )
        .bind(codigo_ean)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;
        let autores: Vec<i32> = aut_rows.iter().map(|ar| row_get_i32(ar, "id_autores")).collect();
        let autores_nombres: Vec<String> = aut_rows.iter().map(|ar| ar.get("autor_nom")).collect();
        let (id_mueble, cantidad) = Self::stock_en_ubicacion(pool, codigo_ean, id_ubicacion).await?;

        Ok(Some(Libro {
            codigo_ean: row_get_i64(&r, "codigo_ean"),
            id_mueble,
            id_proveedor: row_get_i32(&r, "id_proveedor"),
            id_ubicacion,
            nombre_libro: r.get("nombre_libro"),
            precio: row_get_f64(&r, "precio"),
            cantidad,
            sku: row_get_opt_i64(&r, "SKU"),
            generos,
            autores,
            generos_nombres,
            autores_nombres: if autores_nombres.is_empty() { None } else { Some(autores_nombres.join(", ")) },
        }))
    }

    pub async fn guardar_o_actualizar_libro(
        pool: &DbPool,
        req: &RegistrarLibroRequest,
        ubicacion: i32,
    ) -> Result<Libro, AppError> {
        let mut final_generos = req.generos.clone();
        if final_generos.is_empty()
            && let Some(ig) = req.id_genero
        {
            final_generos.push(ig);
        }

        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        // Autores: por id, o (compatibilidad) por nombre -> buscar/crear
        let mut final_autores = req.autores.clone();
        if final_autores.is_empty()
            && let Some(ref nombre_autor) = req.autor
        {
            let clean = nombre_autor.trim();
            if !clean.is_empty() {
                let found = sqlx::query(
                    "SELECT id_autores FROM autores WHERE CONCAT(nombre, ' ', apellidos) = ? OR nombre = ? LIMIT 1",
                )
                .bind(clean)
                .bind(clean)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::Database)?;
                if let Some(ra) = found {
                    final_autores.push(row_get_i32(&ra, "id_autores"));
                } else {
                    let parts: Vec<&str> = clean.split_whitespace().collect();
                    let (nom, ape) = if parts.len() > 1 {
                        (parts[0].to_string(), parts[1..].join(" "))
                    } else {
                        (clean.to_string(), ".".to_string())
                    };
                    let ins = sqlx::query("INSERT INTO autores (nombre, apellidos, estado) VALUES (?, ?, TRUE)")
                        .bind(nom)
                        .bind(ape)
                        .execute(&mut *tx)
                        .await
                        .map_err(AppError::Database)?;
                    final_autores.push(ins.last_insert_id() as i32);
                }
            }
        }

        let existe = sqlx::query("SELECT 1 FROM productos WHERE codigo_ean = ? FOR UPDATE")
            .bind(req.codigo_ean)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?
            .is_some();

        if existe {
            sqlx::query("UPDATE productos SET precio = ?, id_proveedor = ?, SKU = COALESCE(?, SKU) WHERE codigo_ean = ?")
                .bind(req.precio)
                .bind(req.id_proveedor)
                .bind(req.sku)
                .bind(req.codigo_ean)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
            sqlx::query("UPDATE libros SET nombre_libro = ? WHERE codigo_ean = ?")
                .bind(&req.nombre_libro)
                .bind(req.codigo_ean)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
        } else {
            sqlx::query(
                "INSERT INTO productos (codigo_ean, id_proveedor, tipo_producto, SKU, precio) VALUES (?, ?, 'Libro', ?, ?)",
            )
            .bind(req.codigo_ean)
            .bind(req.id_proveedor)
            .bind(req.sku)
            .bind(req.precio)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            sqlx::query("INSERT INTO libros (codigo_ean, nombre_libro) VALUES (?, ?)")
                .bind(req.codigo_ean)
                .bind(&req.nombre_libro)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
        }

        Self::reemplazar_generos(&mut tx, req.codigo_ean, &final_generos).await?;
        if !final_autores.is_empty() {
            sqlx::query("DELETE FROM autores_has_libros WHERE codigo_ean = ?")
                .bind(req.codigo_ean)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
            for &a in &final_autores {
                sqlx::query("INSERT IGNORE INTO autores_has_libros (codigo_ean, id_autores) VALUES (?, ?)")
                    .bind(req.codigo_ean)
                    .bind(a)
                    .execute(&mut *tx)
                    .await
                    .map_err(AppError::Database)?;
            }
        }

        Self::validar_mueble(&mut tx, req.id_mueble, ubicacion, req.cantidad).await?;
        Self::sumar_existencia(&mut tx, req.codigo_ean, req.id_mueble, req.cantidad).await?;

        tx.commit().await.map_err(AppError::Database)?;

        Self::buscar_libro(pool, req.codigo_ean, ubicacion)
            .await?
            .ok_or_else(|| AppError::Internal("No se pudo recuperar el libro recién guardado".into()))
    }

    // ------------------------------------------------------------------
    // Revistas
    // ------------------------------------------------------------------

    pub async fn buscar_revista(
        pool: &DbPool,
        codigo_ean: i64,
        id_ubicacion: i32,
    ) -> Result<Option<Revista>, AppError> {
        let row = sqlx::query(
            "SELECT p.codigo_ean, p.id_proveedor, p.precio, p.SKU, \
                    r.id_editorial, r.nombre_revista, r.numero_edicion, r.periodicidad \
             FROM productos p JOIN revistas r ON r.codigo_ean = p.codigo_ean \
             WHERE p.codigo_ean = ?",
        )
        .bind(codigo_ean)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?;

        let Some(r) = row else { return Ok(None) };
        let (generos, generos_nombres) = Self::generos_de(pool, codigo_ean).await?;
        let (id_mueble, cantidad) = Self::stock_en_ubicacion(pool, codigo_ean, id_ubicacion).await?;

        Ok(Some(Revista {
            codigo_ean: row_get_i64(&r, "codigo_ean"),
            id_ubicacion,
            id_mueble,
            id_proveedor: row_get_i32(&r, "id_proveedor"),
            id_editorial: row_get_i32(&r, "id_editorial"),
            nombre_revista: r.get("nombre_revista"),
            numero_edicion: row_get_opt_i32(&r, "numero_edicion"),
            periodicidad: r.try_get::<Option<String>, _>("periodicidad").ok().flatten(),
            precio: row_get_f64(&r, "precio"),
            cantidad,
            sku: row_get_opt_i64(&r, "SKU"),
            generos,
            generos_nombres,
        }))
    }

    pub async fn guardar_o_actualizar_revista(
        pool: &DbPool,
        req: &RegistrarRevistaRequest,
        ubicacion: i32,
    ) -> Result<Revista, AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let existe = sqlx::query("SELECT id_editorial FROM revistas WHERE codigo_ean = ? FOR UPDATE")
            .bind(req.codigo_ean)
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::Database)?;

        // La editorial es obligatoria al crear; al actualizar se conserva si no se envía.
        let id_editorial = match (req.id_editorial, &existe) {
            (Some(e), _) => e,
            (None, Some(r)) => row_get_i32(r, "id_editorial"),
            (None, None) => {
                return Err(AppError::BadRequest(
                    "La editorial (id_editorial) es obligatoria para registrar una revista".to_string(),
                ));
            }
        };

        if existe.is_some() {
            sqlx::query("UPDATE productos SET precio = ?, id_proveedor = ?, SKU = COALESCE(?, SKU) WHERE codigo_ean = ?")
                .bind(req.precio)
                .bind(req.id_proveedor)
                .bind(req.sku)
                .bind(req.codigo_ean)
                .execute(&mut *tx)
                .await
                .map_err(AppError::Database)?;
            sqlx::query(
                "UPDATE revistas SET id_editorial = ?, nombre_revista = ?, numero_edicion = ?, periodicidad = ? WHERE codigo_ean = ?",
            )
            .bind(id_editorial)
            .bind(&req.nombre_revista)
            .bind(req.numero_edicion)
            .bind(&req.periodicidad)
            .bind(req.codigo_ean)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        } else {
            sqlx::query(
                "INSERT INTO productos (codigo_ean, id_proveedor, tipo_producto, SKU, precio) VALUES (?, ?, 'Revista', ?, ?)",
            )
            .bind(req.codigo_ean)
            .bind(req.id_proveedor)
            .bind(req.sku)
            .bind(req.precio)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
            sqlx::query(
                "INSERT INTO revistas (codigo_ean, id_editorial, nombre_revista, numero_edicion, periodicidad) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(req.codigo_ean)
            .bind(id_editorial)
            .bind(&req.nombre_revista)
            .bind(req.numero_edicion)
            .bind(&req.periodicidad)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }

        Self::reemplazar_generos(&mut tx, req.codigo_ean, &req.generos).await?;
        Self::validar_mueble(&mut tx, req.id_mueble, ubicacion, req.cantidad).await?;
        Self::sumar_existencia(&mut tx, req.codigo_ean, req.id_mueble, req.cantidad).await?;

        tx.commit().await.map_err(AppError::Database)?;

        Self::buscar_revista(pool, req.codigo_ean, ubicacion)
            .await?
            .ok_or_else(|| AppError::Internal("No se pudo recuperar la revista recién guardada".into()))
    }

    // ========================================================================
    // Delegaciones a repositorios especializados
    // ========================================================================

    pub async fn listar_generos(pool: &DbPool) -> Result<Vec<Genero>, AppError> {
        GeneroRepo::listar_generos(pool).await
    }

    pub async fn listar_autores(pool: &DbPool) -> Result<Vec<Autor>, AppError> {
        AutorRepo::listar_autores(pool).await
    }

    pub async fn buscar_autor(pool: &DbPool, id: i32) -> Result<Option<Autor>, AppError> {
        AutorRepo::buscar_autor(pool, id).await
    }

    pub async fn crear_autor(pool: &DbPool, req: &CrearAutorRequest) -> Result<Autor, AppError> {
        AutorRepo::crear_autor(pool, req).await
    }

    pub async fn actualizar_autor(
        pool: &DbPool,
        id: i32,
        req: &ActualizarAutorRequest,
    ) -> Result<Autor, AppError> {
        AutorRepo::actualizar_autor(pool, id, req).await
    }

    pub async fn eliminar_autor(pool: &DbPool, id: i32) -> Result<(), AppError> {
        AutorRepo::eliminar_autor(pool, id).await
    }

    pub async fn listar_proveedores(pool: &DbPool) -> Result<Vec<Proveedor>, AppError> {
        ProveedorRepo::listar_proveedores(pool).await
    }

    pub async fn buscar_proveedor(pool: &DbPool, id: i32) -> Result<Option<Proveedor>, AppError> {
        ProveedorRepo::buscar_proveedor(pool, id).await
    }

    pub async fn crear_proveedor(
        pool: &DbPool,
        req: &CrearProveedorRequest,
    ) -> Result<Proveedor, AppError> {
        ProveedorRepo::crear_proveedor(pool, req).await
    }

    pub async fn actualizar_proveedor(
        pool: &DbPool,
        id: i32,
        req: &ActualizarProveedorRequest,
    ) -> Result<Proveedor, AppError> {
        ProveedorRepo::actualizar_proveedor(pool, id, req).await
    }

    pub async fn eliminar_proveedor(pool: &DbPool, id: i32) -> Result<(), AppError> {
        ProveedorRepo::eliminar_proveedor(pool, id).await
    }

    pub async fn trasladar_stock_libro(
        pool: &DbPool,
        codigo_ean: i64,
        cantidad: i32,
        origen: i32,
        destino: i32,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<(i32, i32), AppError> {
        MovimientoRepo::trasladar_stock_libro(
            pool,
            codigo_ean,
            cantidad,
            origen,
            destino,
            actor_id,
            observaciones,
        )
        .await
    }

    pub async fn trasladar_stock_revista(
        pool: &DbPool,
        codigo_ean: i64,
        cantidad: i32,
        origen: i32,
        destino: i32,
        actor_id: i64,
        observaciones: Option<&str>,
    ) -> Result<(i32, i32), AppError> {
        MovimientoRepo::trasladar_stock_revista(
            pool,
            codigo_ean,
            cantidad,
            origen,
            destino,
            actor_id,
            observaciones,
        )
        .await
    }

    pub async fn registrar_venta_transaccional(
        pool: &DbPool,
        actor_id: i64,
        req: &RegistrarVentaRequest,
    ) -> Result<VentaResponse, AppError> {
        VentaRepo::registrar_venta_transaccional(pool, actor_id, req).await
    }

    pub async fn registrar_compra_transaccional(
        pool: &DbPool,
        actor_id: i64,
        req: &RegistrarCompraRequest,
    ) -> Result<CompraResponse, AppError> {
        CompraRepo::registrar_compra_transaccional(pool, actor_id, req).await
    }

    pub async fn listar_compras(pool: &DbPool) -> Result<Vec<CompraResponse>, AppError> {
        CompraRepo::listar_compras(pool).await
    }

    pub async fn consultar_existencias(
        pool: &DbPool,
        params: &FiltroExistencias,
    ) -> Result<Vec<ExistenciaInventario>, AppError> {
        InventarioRepo::consultar_existencias(pool, params).await
    }

    pub async fn registrar_devolucion(
        pool: &DbPool,
        id_usuario: i64,
        id_proveedor: i32,
        tipo_producto: TipoProducto,
        items: &[ItemDevolucionRequest],
    ) -> Result<i64, AppError> {
        DevolucionRepo::registrar_devolucion(
            pool,
            id_usuario,
            id_proveedor,
            tipo_producto,
            items,
        )
        .await
    }

    pub async fn buscar_devolucion(
        pool: &DbPool,
        id: i64,
    ) -> Result<Option<Devolucion>, AppError> {
        DevolucionRepo::buscar_devolucion(pool, id).await
    }

    pub async fn listar_historial_devoluciones(
        pool: &DbPool,
        params: &HistorialDevolucionQuery,
    ) -> Result<Vec<Devolucion>, AppError> {
        DevolucionRepo::listar_historial_devoluciones(pool, params).await
    }

    pub async fn actualizar_estado_devolucion(
        pool: &DbPool,
        id: i64,
        id_usuario_aprueba: i64,
        nuevo_estado: &str,
        aprobar: bool,
    ) -> Result<(), AppError> {
        DevolucionRepo::actualizar_estado_devolucion(
            pool,
            id,
            id_usuario_aprueba,
            nuevo_estado,
            aprobar,
        )
        .await
    }

    pub async fn consultar_movimiento_diario(
        pool: &DbPool,
        fecha: Option<&str>,
    ) -> Result<Vec<MovimientoDiarioItem>, AppError> {
        BitacoraRepo::consultar_movimiento_diario(pool, fecha).await
    }
}
