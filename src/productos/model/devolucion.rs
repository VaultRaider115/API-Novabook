use super::producto::TipoProducto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDevolucion {
    pub codigo_ean: i64,
    pub id_motivo: i32,
    pub sku: Option<i64>,
    pub titulo: String,
    /// Segunda línea del acta (diseño 21): autores del libro o
    /// "Edición mensual · No. 452" de la revista.
    #[serde(default)]
    pub descripcion: Option<String>,
    /// Costo unitario de la última compra del EAN (None si nunca se compró).
    #[serde(default)]
    pub costo_unitario: Option<f64>,
    /// Precio de venta actual (productos.precio).
    #[serde(default)]
    pub precio_venta: Option<f64>,
    pub cantidad: i32,
    pub motivo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Devolucion {
    pub id_devolucion: i64,
    /// Folio visible, p. ej. "DEV-2026-7"
    pub folio: String,
    pub solicitante_rol: Option<String>,
    pub fecha: String,
    pub total_piezas: i32,
    pub estado: String,
    pub autorizado_por: Option<String>,
    pub id_usuarios: i64,
    pub vendedor_nombre: Option<String>,
    pub id_proveedor: i32,
    pub nombre_proveedor: Option<String>,
    pub proveedor_rfc: Option<String>,
    pub proveedor_telefono: Option<String>,
    pub proveedor_correo: Option<String>,
    pub proveedor_direccion: Option<String>,
    pub proveedor_contacto: Option<String>,
    pub tipo_producto: TipoProducto,
    pub items: Vec<ItemDevolucion>,
    /// Nota del Jefe al aprobar o rechazar (columna devolucion.observaciones).
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemDevolucionRequest {
    pub codigo_ean: i64,
    pub cantidad: i32,
    pub id_motivo: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidarPartidaQuery {
    pub id_proveedor: i32,
    pub codigo_ean: i64,
    /// Piezas de este EAN en la lista, contando las que ya se habían añadido.
    pub cantidad: i32,
}

/// Resultado de "Añadir" en la devolución a proveedor: el producto existe,
/// es de la editorial (proveedor) elegida y alcanza lo de bodega + apartados.
#[derive(Debug, Clone, Serialize)]
pub struct PartidaValidada {
    pub codigo_ean: i64,
    pub titulo: String,
    pub tipo_producto: String,
    pub id_proveedor: i32,
    pub proveedor: String,
    /// Bodega + apartados (lo que se puede devolver).
    pub disponible: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrearDevolucionRequest {
    pub id_proveedor: i32,
    #[serde(default = "default_tipo_libro")]
    pub tipo_producto: TipoProducto,
    pub items: Vec<ItemDevolucionRequest>,
}

fn default_tipo_libro() -> TipoProducto {
    TipoProducto::Libro
}

#[derive(Debug, Clone, Deserialize)]
pub struct AprobarDevolucionRequest {
    #[serde(default = "default_true")]
    pub aprobar: bool,
    pub notas: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HistorialDevolucionQuery {
    pub fecha: Option<String>,
    pub estado: Option<String>,
    pub id_proveedor: Option<i32>,
    /// Solo lo fija el servidor (lista "Mis devoluciones"); nunca viene de la URL.
    #[serde(skip)]
    pub id_usuarios: Option<i64>,
}
