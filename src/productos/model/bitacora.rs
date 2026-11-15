use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct MovimientoDiarioItem {
    pub fecha_hora: String,
    pub usuario: String,
    pub rol: String,
    pub accion: String,
    pub detalle: String,
}

#[derive(Debug, Serialize)]
pub struct ResumenMovimientoDiario {
    pub fecha: String,
    pub total_eventos: usize,
    pub movimientos: Vec<MovimientoDiarioItem>,
}

#[derive(Debug, Deserialize)]
pub struct FechaQuery {
    pub fecha: Option<String>,
}

/// Filtros de la bitácora paginada (todas las fechas en formato YYYY-MM-DD).
#[derive(Debug, Deserialize, Default)]
pub struct BitacoraQuery {
    pub desde: Option<String>,
    pub hasta: Option<String>,
    /// ACCESO | VENTA | COMPRA | TRASLADO | DEVOLUCION | INVENTARIO | USUARIOS | CATALOGO | SISTEMA
    pub categoria: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct BitacoraItem {
    pub fecha_hora: String,
    pub usuario: String,
    pub rol: String,
    pub accion: String,
    pub detalle: String,
    pub categoria: String,
}

#[derive(Debug, Serialize)]
pub struct BitacoraPagina {
    pub items: Vec<BitacoraItem>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}
