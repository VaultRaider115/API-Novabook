pub mod autor;
pub mod bitacora;
pub mod bodega;
pub mod compra;
pub mod devolucion;
pub mod editorial;
pub mod genero;
pub mod inventario;
pub mod inventario_mensual;
pub mod motivo;
pub mod movimiento;
pub mod mueble;
pub mod producto;
pub mod proveedor;
pub mod venta;

pub use autor::*;
pub use bitacora::*;
pub use compra::*;
pub use devolucion::*;
pub use editorial::*;
pub use genero::*;
pub use inventario::*;
pub use motivo::*;
pub use movimiento::*;
pub use mueble::*;
pub use producto::*;
pub use proveedor::*;
pub use venta::*;

use axum::{
    Router, middleware,
    routing::{get, post, put},
};

use crate::{
    db::AppState,
    middleware::{
        auth_middleware, require_bodega, require_gerente, require_jefe, require_jefe_o_bodega,
        require_vendedor,
    },
};

// ============================================================================
// Router Ensamblado para Productos, Catálogos, Bodega, Vendedor y Jefe
// ============================================================================

pub fn router(state: AppState) -> Router<AppState> {
    // Rutas de Catálogos. Permisos según docs/NovaBook-Diagrama-de-procesos-del-sistema.png
    // ("Procesos de soporte · Catálogos"):
    //   - Lectura: cualquier usuario autenticado.
    //   - Autores, géneros, muebles y motivos: solo el Jefe escribe.
    //   - Proveedores/editoriales: el Jefe tiene CRUD completo; Bodega solo puede
    //     dar de alta uno nuevo al recibir la compra (paso 1). El Gerente no
    //     administra catálogos (su proceso es Usuarios).
    let solo_jefe = middleware::from_fn(require_jefe);
    let alta_jefe_o_bodega = middleware::from_fn(require_jefe_o_bodega);
    let catalogos_routes = Router::new()
        .route(
            "/generos",
            get(genero::listar_generos_handler)
                .merge(post(genero::crear_genero_handler).layer(solo_jefe.clone())),
        )
        .route(
            "/generos/{id}",
            put(genero::actualizar_genero_handler)
                .delete(genero::eliminar_genero_handler)
                .layer(solo_jefe.clone()),
        )
        .route(
            "/motivos",
            get(motivo::listar_motivos_handler)
                .merge(post(motivo::crear_motivo_handler).layer(solo_jefe.clone())),
        )
        .route(
            "/motivos/{id}",
            put(motivo::actualizar_motivo_handler)
                .delete(motivo::eliminar_motivo_handler)
                .layer(solo_jefe.clone()),
        )
        .route(
            "/muebles",
            get(mueble::listar_muebles_handler)
                .merge(post(mueble::crear_mueble_handler).layer(solo_jefe.clone())),
        )
        .route(
            "/muebles/{id}",
            put(mueble::actualizar_mueble_handler)
                .delete(mueble::eliminar_mueble_handler)
                .layer(solo_jefe.clone()),
        )
        .route(
            "/autores",
            get(autor::listar_autores_handler)
                .merge(post(autor::crear_autor_handler).layer(solo_jefe.clone())),
        )
        .route(
            "/autores/{id}",
            get(autor::obtener_autor_handler).merge(
                put(autor::actualizar_autor_handler)
                    .delete(autor::eliminar_autor_handler)
                    .layer(solo_jefe.clone()),
            ),
        )
        .route(
            "/editoriales",
            get(editorial::listar_editoriales_handler)
                .merge(post(editorial::crear_editorial_handler).layer(alta_jefe_o_bodega.clone())),
        )
        .route(
            "/editoriales/{id}",
            put(editorial::actualizar_editorial_handler)
                .delete(editorial::eliminar_editorial_handler)
                .layer(solo_jefe.clone()),
        )
        .route(
            "/proveedores",
            get(proveedor::listar_proveedores_handler)
                .merge(post(proveedor::crear_proveedor_handler).layer(alta_jefe_o_bodega)),
        )
        .route(
            "/proveedores/{id}",
            get(proveedor::obtener_proveedor_handler).merge(
                put(proveedor::actualizar_proveedor_handler)
                    .delete(proveedor::eliminar_proveedor_handler)
                    .layer(solo_jefe),
            ),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let bodega_routes = Router::new()
        .route("/libros", post(producto::registrar_libro_bodega))
        .route("/revistas", post(producto::registrar_revista_bodega))
        .route("/movimientos/libros", post(movimiento::movimiento_bodega_tienda_libros))
        .route(
            "/movimientos/revistas",
            post(movimiento::movimiento_bodega_tienda_revistas),
        )
        .route("/requisiciones", post(movimiento::requisicion_salida_bodega))
        // Títulos y editorial para armar la requisición de salida.
        .route("/existencias", get(inventario::consultar_existencias_vendedor))
        .route("/compras", post(compra::registrar_compra_bodega))
        .route("/compras", get(compra::listar_compras_bodega))
        .layer(middleware::from_fn(require_bodega))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let vendedor_routes = Router::new()
        .route("/ventas", post(venta::registrar_venta_vendedor))
        .route("/existencias", get(inventario::consultar_existencias_vendedor))
        .route("/movimientos/libros", post(movimiento::movimiento_tienda_bodega_libros))
        .route(
            "/movimientos/revistas",
            post(movimiento::movimiento_tienda_bodega_revistas),
        )
        .route("/requisiciones", post(movimiento::requisicion_entrada_vendedor))
        .route("/tickets/{id}", get(devolucion::consultar_ticket_vendedor))
        .route(
            "/inventarios",
            post(inventario_mensual::abrir_inventario),
        )
        .route("/inventarios/actual", get(inventario_mensual::inventario_actual))
        .route("/inventarios/{id}/conteos", put(inventario_mensual::registrar_conteo))
        .route("/inventarios/{id}/cerrar", post(inventario_mensual::cerrar_inventario))
        .route("/devoluciones-cliente", post(devolucion::registrar_devolucion_cliente_vendedor))
        .route(
            "/devoluciones",
            post(devolucion::crear_devolucion_vendedor)
                .get(devolucion::consultar_mis_devoluciones_vendedor),
        )
        .route(
            "/devoluciones/{id}/pdf/libros",
            get(devolucion::generar_pdf_libros_vendedor),
        )
        .route(
            "/devoluciones/{id}/pdf/revistas",
            get(devolucion::generar_pdf_revistas_vendedor),
        )
        .layer(middleware::from_fn(require_vendedor))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let jefe_routes = Router::new()
        .route(
            "/devoluciones/historial",
            get(devolucion::consultar_historial_devoluciones_jefe),
        )
        .route(
            "/devoluciones/{id}/pdf/libros",
            get(devolucion::generar_pdf_libros_vendedor),
        )
        .route(
            "/devoluciones/{id}/pdf/revistas",
            get(devolucion::generar_pdf_revistas_vendedor),
        )
        .route(
            "/movimientos/diarios",
            get(bitacora::consultar_movimiento_diario_jefe),
        )
        .route("/bitacora", get(bitacora::consultar_bitacora_jefe))
        .route("/inventarios", get(inventario_mensual::listar_inventarios))
        .route("/inventarios/{id}", get(inventario_mensual::obtener_inventario))
        .route("/inventarios/{id}/revisar", put(inventario_mensual::revisar_inventario))
        .route(
            "/solicitudes-editorial",
            get(inventario_mensual::listar_solicitudes).post(inventario_mensual::crear_solicitud),
        )
        .route("/solicitudes-editorial/{id}/atender", put(inventario_mensual::atender_solicitud))
        .route("/devoluciones/{id}/aprobar", put(devolucion::aprobar_devolucion_jefe))
        .layer(middleware::from_fn(require_jefe))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Gerente: consulta el historial de devoluciones y descarga actas aprobadas.
    // La aprobación es solo del Jefe (diagrama de procesos, paso 9).
    let gerente_routes = Router::new()
        .route(
            "/devoluciones/historial",
            get(devolucion::consultar_historial_devoluciones_jefe),
        )
        .route(
            "/devoluciones/{id}/pdf/libros",
            get(devolucion::generar_pdf_libros_vendedor),
        )
        .route(
            "/devoluciones/{id}/pdf/revistas",
            get(devolucion::generar_pdf_revistas_vendedor),
        )
        .layer(middleware::from_fn(require_gerente))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/api", catalogos_routes)
        .nest("/api/gerente", gerente_routes)
        .nest("/api/bodega", bodega_routes)
        .nest("/api/vendedor", vendedor_routes)
        .nest("/api/jefe", jefe_routes)
}

